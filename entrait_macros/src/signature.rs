use super::deps::Deps;
use super::input::{EntraitAttr, InputFn};

use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;
use std::collections::HashSet;
use syn::visit_mut::VisitMut;

pub struct EntraitSignature {
    pub sig: syn::Signature,
    pub associated_fut_decl: Option<proc_macro2::TokenStream>,
    pub associated_fut_impl: Option<proc_macro2::TokenStream>,
    pub lifetimes: Vec<EntraitLifetime>,
}

/// Only used for associated future:
pub struct EntraitLifetime {
    pub lifetime: syn::Lifetime,
    pub source: SigComponent,
    pub user_provided: UserProvidedLifetime,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SigComponent {
    Receiver,
    Param(usize),
    Output,
}

pub struct UserProvidedLifetime(bool);

pub struct SignatureConverter<'a> {
    attr: &'a EntraitAttr,
    input_fn: &'a InputFn,
    deps: &'a Deps<'a>,
}

#[derive(Clone, Copy)]
enum ReceiverGeneration {
    Insert,
    Rewrite,
    None,
}

impl<'a> SignatureConverter<'a> {
    pub fn new(
        attr: &'a EntraitAttr,
        input_fn: &'a InputFn,
        deps: &'a Deps,
    ) -> SignatureConverter<'a> {
        Self {
            attr,
            input_fn,
            deps,
        }
    }

    pub fn convert(&self) -> EntraitSignature {
        let mut entrait_sig = EntraitSignature {
            sig: self.input_fn.fn_sig.clone(),
            associated_fut_decl: None,
            associated_fut_impl: None,
            lifetimes: vec![],
        };

        // strip away attributes
        for fn_arg in entrait_sig.sig.inputs.iter_mut() {
            match fn_arg {
                syn::FnArg::Receiver(receiver) => {
                    receiver.attrs = vec![];
                }
                syn::FnArg::Typed(pat_type) => {
                    pat_type.attrs = vec![];
                }
            }
        }

        let receiver_generation = self.detect_receiver_generation(&entrait_sig.sig);
        self.generate_receiver(&mut entrait_sig.sig, receiver_generation);

        if self.input_fn.use_associated_future(self.attr) {
            self.convert_to_associated_future(&mut entrait_sig, receiver_generation);
        }

        self.remove_deps_generic_param(&mut entrait_sig.sig);
        tidy_generics(&mut entrait_sig.sig.generics);

        entrait_sig
    }

    fn detect_receiver_generation(&self, sig: &syn::Signature) -> ReceiverGeneration {
        match self.deps {
            Deps::NoDeps => ReceiverGeneration::Insert,
            _ => {
                if sig.inputs.is_empty() {
                    if self.input_fn.use_associated_future(self.attr) {
                        ReceiverGeneration::Insert
                    } else {
                        ReceiverGeneration::None // bug?
                    }
                } else {
                    ReceiverGeneration::Rewrite
                }
            }
        }
    }

    fn generate_receiver(&self, sig: &mut syn::Signature, receiver_generation: ReceiverGeneration) {
        let span = self.attr.trait_ident.span();
        match receiver_generation {
            ReceiverGeneration::Insert => {
                sig.inputs
                    .insert(0, syn::parse_quote_spanned! { span=> &self });
            }
            ReceiverGeneration::Rewrite => {
                let input = sig.inputs.first_mut().unwrap();
                match input {
                    syn::FnArg::Typed(pat_type) => match pat_type.ty.as_ref() {
                        syn::Type::Reference(type_reference) => {
                            let and_token = type_reference.and_token.clone();
                            let lifetime = type_reference.lifetime.clone();

                            *input = syn::FnArg::Receiver(syn::Receiver {
                                attrs: vec![],
                                reference: Some((and_token, lifetime)),
                                mutability: None,
                                self_token: syn::parse_quote_spanned! { span=> self },
                            });
                        }
                        _ => {
                            let first_mut = sig.inputs.first_mut().unwrap();
                            *first_mut = syn::parse_quote_spanned! { span=> &self };
                        }
                    },
                    syn::FnArg::Receiver(_) => panic!(),
                }
            }
            ReceiverGeneration::None => {}
        }
    }

    fn convert_to_associated_future(
        &self,
        entrait_sig: &mut EntraitSignature,
        receiver_generation: ReceiverGeneration,
    ) {
        let span = self.attr.trait_ident.span();

        expand_lifetimes(entrait_sig, receiver_generation);

        let output_ty = output_type_tokens(&entrait_sig.sig.output);

        let fut_lifetimes = entrait_sig
            .lifetimes
            .iter()
            .map(|ft| &ft.lifetime)
            .collect::<Vec<_>>();
        let self_lifetimes = entrait_sig
            .lifetimes
            .iter()
            .filter(|ft| ft.source == SigComponent::Receiver)
            .map(|ft| &ft.lifetime)
            .collect::<Vec<_>>();

        // make the function generic if it wasn't already
        let sig = &mut entrait_sig.sig;
        sig.asyncness = None;
        let generics = &mut sig.generics;
        generics.lt_token.get_or_insert(syn::parse_quote! { < });
        generics.gt_token.get_or_insert(syn::parse_quote! { > });

        // insert generated/non-user-provided lifetimes
        for fut_lifetime in entrait_sig
            .lifetimes
            .iter()
            .filter(|lt| !lt.user_provided.0)
        {
            generics
                .params
                .push(syn::GenericParam::Lifetime(syn::LifetimeDef {
                    attrs: vec![],
                    lifetime: fut_lifetime.lifetime.clone(),
                    colon_token: None,
                    bounds: syn::punctuated::Punctuated::new(),
                }));
        }

        entrait_sig.sig.output = syn::parse_quote_spanned! {span =>
            -> Self::Fut<#(#fut_lifetimes),*>
        };

        entrait_sig.associated_fut_decl = Some(quote_spanned! { span=>
            type Fut<#(#fut_lifetimes),*>: ::core::future::Future<Output = #output_ty> + Send
            where
                Self: #(#self_lifetimes)+*;
        });

        entrait_sig.associated_fut_impl = Some(quote_spanned! { span=>
            type Fut<#(#fut_lifetimes),*> = impl ::core::future::Future<Output = #output_ty>
            where
                Self: #(#self_lifetimes)+*;
        });
    }

    fn remove_deps_generic_param(&self, sig: &mut syn::Signature) {
        match self.deps {
            Deps::Generic {
                generic_param: Some(ident),
                ..
            } => {
                let generics = &mut sig.generics;
                let mut params = syn::punctuated::Punctuated::new();
                std::mem::swap(&mut params, &mut generics.params);

                for param in params.into_iter() {
                    match &param {
                        syn::GenericParam::Type(type_param) => {
                            if type_param.ident != *ident {
                                generics.params.push(param);
                            }
                        }
                        _ => {
                            generics.params.push(param);
                        }
                    }
                }

                if let Some(where_clause) = &mut generics.where_clause {
                    let mut predicates = syn::punctuated::Punctuated::new();
                    std::mem::swap(&mut predicates, &mut where_clause.predicates);

                    for predicate in predicates.into_iter() {
                        match &predicate {
                            syn::WherePredicate::Type(pred) => {
                                if !is_type_eq_ident(&pred.bounded_ty, &ident) {
                                    where_clause.predicates.push(predicate);
                                }
                            }
                            _ => {
                                where_clause.predicates.push(predicate);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn is_type_eq_ident(ty: &syn::Type, ident: &syn::Ident) -> bool {
    match ty {
        syn::Type::Path(type_path) if type_path.path.segments.len() == 1 => {
            type_path.path.segments.first().unwrap().ident == *ident
        }
        _ => false,
    }
}

fn output_type_tokens(return_type: &syn::ReturnType) -> TokenStream {
    match return_type {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    }
}

fn tidy_generics(generics: &mut syn::Generics) {
    if generics
        .where_clause
        .as_ref()
        .map(|cl| cl.predicates.is_empty())
        .unwrap_or(false)
    {
        generics.where_clause = None;
    }

    if generics.params.is_empty() {
        generics.lt_token = None;
        generics.gt_token = None;
    }
}

fn expand_lifetimes(entrait_sig: &mut EntraitSignature, receiver_generation: ReceiverGeneration) {
    let mut elision_detector = ElisionDetector::new(receiver_generation);
    elision_detector.detect(&mut entrait_sig.sig);

    let mut expander = LifetimeExpander::new(elision_detector.elided_params);

    match receiver_generation {
        ReceiverGeneration::None => {
            for (index, arg) in entrait_sig.sig.inputs.iter_mut().enumerate() {
                expander.expand_param(index, arg);
            }
        }
        ReceiverGeneration::Rewrite | ReceiverGeneration::Insert => {
            expander.expand_receiver(entrait_sig.sig.inputs.first_mut().unwrap());

            for (index, arg) in entrait_sig.sig.inputs.iter_mut().skip(1).enumerate() {
                expander.expand_param(index, arg);
            }
        }
    }

    expander.expand_output(&mut entrait_sig.sig.output);

    entrait_sig.lifetimes.append(&mut expander.lifetimes);
}

/// Looks at elided lifetimes and makes them explicit.
/// Also collects all lifetimes into `lifetimes`.
struct LifetimeExpander {
    current_component: SigComponent,
    elided_params: HashSet<usize>,
    lifetimes: Vec<EntraitLifetime>,
}

impl LifetimeExpander {
    fn new(elided_params: HashSet<usize>) -> Self {
        Self {
            current_component: SigComponent::Receiver,
            elided_params,
            lifetimes: vec![],
        }
    }

    fn expand_receiver(&mut self, arg: &mut syn::FnArg) {
        self.current_component = SigComponent::Receiver;
        self.visit_fn_arg_mut(arg);
    }

    fn expand_param(&mut self, index: usize, arg: &mut syn::FnArg) {
        self.current_component = SigComponent::Param(index);
        self.visit_fn_arg_mut(arg);
    }

    fn expand_output(&mut self, output: &mut syn::ReturnType) {
        self.current_component = SigComponent::Output;
        self.visit_return_type_mut(output);
    }

    fn make_lifetime_explicit(&mut self, lifetime: Option<syn::Lifetime>) -> syn::Lifetime {
        match self.current_component {
            SigComponent::Receiver | SigComponent::Param(_) => match lifetime {
                Some(lifetime) => self.register_user_lifetime(lifetime),
                None => self.register_new_entrait_lifetime(),
            },
            // Do not register user-provided output lifetimes, should already be registered from inputs:
            SigComponent::Output => lifetime
                // If lifetime was elided, try to find it:
                .or_else(|| self.find_output_lifetime())
                // If not, there must be some kind of compile error somewhere else
                .unwrap_or_else(|| self.broken_lifetime()),
        }
    }

    fn find_output_lifetime(&self) -> Option<syn::Lifetime> {
        let from_component = match self.only_elided_input() {
            // If only one input was elided, use that input:
            Some(elided_input) => SigComponent::Param(elided_input),
            // If not, use the receiver lifetime:
            None => SigComponent::Receiver,
        };

        self.lifetimes
            .iter()
            .find(|lt| lt.source == from_component)
            .map(|lt| lt.lifetime.clone())
    }

    fn only_elided_input(&self) -> Option<usize> {
        if self.elided_params.len() == 1 {
            self.elided_params.iter().next().map(|index| *index)
        } else {
            None
        }
    }

    fn register_user_lifetime(&mut self, lifetime: syn::Lifetime) -> syn::Lifetime {
        self.register_lifetime(EntraitLifetime {
            lifetime,
            source: self.current_component,
            user_provided: UserProvidedLifetime(true),
        })
    }

    fn register_new_entrait_lifetime(&mut self) -> syn::Lifetime {
        let index = self.lifetimes.len();
        self.register_lifetime(EntraitLifetime {
            lifetime: syn::Lifetime::new(
                &format!("'entrait{}", index),
                proc_macro2::Span::call_site(),
            ),
            source: self.current_component,
            user_provided: UserProvidedLifetime(false),
        })
    }

    fn register_lifetime(&mut self, entrait_lifetime: EntraitLifetime) -> syn::Lifetime {
        let lifetime = entrait_lifetime.lifetime.clone();
        self.lifetimes.push(entrait_lifetime);
        lifetime
    }

    fn broken_lifetime(&self) -> syn::Lifetime {
        syn::Lifetime::new("'entrait_broken", proc_macro2::Span::call_site())
    }
}

impl<'s> syn::visit_mut::VisitMut for LifetimeExpander {
    fn visit_receiver_mut(&mut self, receiver: &mut syn::Receiver) {
        if let Some((_, lifetime)) = &mut receiver.reference {
            *lifetime = Some(self.make_lifetime_explicit(lifetime.clone()));
        }
        syn::visit_mut::visit_receiver_mut(self, receiver);
    }

    fn visit_type_reference_mut(&mut self, reference: &mut syn::TypeReference) {
        reference.lifetime = Some(self.make_lifetime_explicit(reference.lifetime.clone()));
        syn::visit_mut::visit_type_reference_mut(self, reference);
    }

    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident == "_" {
            *lifetime = self.make_lifetime_explicit(Some(lifetime.clone()));
        }
    }
}

struct ElisionDetector {
    receiver_generation: ReceiverGeneration,
    current_input: usize,
    elided_params: HashSet<usize>,
}

impl ElisionDetector {
    fn new(receiver_generation: ReceiverGeneration) -> Self {
        Self {
            receiver_generation,
            current_input: 0,
            elided_params: Default::default(),
        }
    }

    fn detect(&mut self, sig: &mut syn::Signature) {
        for (index, input) in sig.inputs.iter_mut().enumerate() {
            match self.receiver_generation {
                ReceiverGeneration::None => {
                    self.current_input = index;
                    self.visit_fn_arg_mut(input);
                }
                _ => {
                    if index > 1 {
                        self.current_input = index - 1;
                        self.visit_fn_arg_mut(input);
                    }
                }
            }
        }
    }
}

impl syn::visit_mut::VisitMut for ElisionDetector {
    fn visit_type_reference_mut(&mut self, reference: &mut syn::TypeReference) {
        if reference.lifetime.is_none() {
            self.elided_params.insert(self.current_input);
        }
        syn::visit_mut::visit_type_reference_mut(self, reference);
    }

    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident == "_" {
            self.elided_params.insert(self.current_input);
        }
    }
}
