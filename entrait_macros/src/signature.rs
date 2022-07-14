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
    pub fut_lifetimes: Vec<FutureLifetime>,
}

/// Only used for associated future:
pub struct FutureLifetime {
    pub lifetime: syn::Lifetime,
    pub kind: LifetimeKind,
}

#[derive(Eq, PartialEq)]
pub enum LifetimeKind {
    Receiver,
    Other,
}

pub struct SignatureConverter<'a> {
    attr: &'a EntraitAttr,
    input_fn: &'a InputFn,
    deps: &'a Deps<'a>,
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
            fut_lifetimes: vec![],
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

        if self.input_fn.use_associated_future(self.attr) {
            self.convert_to_associated_future(&mut entrait_sig);
        } else {
            self.make_self_parameter(&mut entrait_sig.sig);
        }

        self.remove_deps_generic_param(&mut entrait_sig.sig);
        tidy_generics(&mut entrait_sig.sig.generics);

        entrait_sig
    }

    fn make_self_parameter(&self, sig: &mut syn::Signature) -> bool {
        let span = self.attr.trait_ident.span();
        match self.deps {
            Deps::NoDeps => {
                sig.inputs
                    .insert(0, syn::parse_quote_spanned! { span=> &self });
                true
            }
            _ => {
                if sig.inputs.is_empty() {
                    if self.input_fn.use_associated_future(self.attr) {
                        sig.inputs
                            .insert(0, syn::parse_quote_spanned! { span=> &self });
                        true
                    } else {
                        false
                    }
                } else {
                    let first_mut = sig.inputs.first_mut().unwrap();
                    *first_mut = syn::parse_quote_spanned! { span=> &self };
                    true
                }
            }
        }
    }

    fn convert_to_associated_future(&self, entrait_sig: &mut EntraitSignature) {
        let span = self.attr.trait_ident.span();

        let mut elision_detector: ElisionDetector = Default::default();
        elision_detector.detect(&mut entrait_sig.sig);

        // make the self parameter after the elision detector has run, since it indexes parameters without a receiver
        let has_receiver = self.make_self_parameter(&mut entrait_sig.sig);

        make_all_lifetimes_explicit(entrait_sig, has_receiver, elision_detector.elided_inputs);

        let output_ty = output_type_tokens(&entrait_sig.sig.output);

        let fut_lifetimes = entrait_sig
            .fut_lifetimes
            .iter()
            .map(|ft| &ft.lifetime)
            .collect::<Vec<_>>();
        let self_lifetimes = entrait_sig
            .fut_lifetimes
            .iter()
            .filter(|ft| ft.kind == LifetimeKind::Receiver)
            .map(|ft| &ft.lifetime)
            .collect::<Vec<_>>();

        // make the function generic if it wasn't already
        let sig = &mut entrait_sig.sig;
        sig.asyncness = None;
        let generics = &mut sig.generics;
        generics.lt_token.get_or_insert(syn::parse_quote! { < });
        generics.gt_token.get_or_insert(syn::parse_quote! { > });

        // insert lifetime params at the front
        for (index, fut_lifetime) in fut_lifetimes.iter().enumerate() {
            generics.params.insert(
                index,
                syn::GenericParam::Lifetime(syn::LifetimeDef {
                    attrs: vec![],
                    lifetime: (*fut_lifetime).clone(),
                    colon_token: None,
                    bounds: syn::punctuated::Punctuated::new(),
                }),
            );
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

pub fn make_all_lifetimes_explicit(
    entrait_sig: &mut EntraitSignature,
    has_receiver: bool,
    elided_inputs: HashSet<usize>,
) {
    let mut explicitor = Explicator {
        current: SigComponent::Receiver,
        elided_inputs,
        lifetimes: vec![],
    };

    if has_receiver {
        explicitor.explicate_receiver(entrait_sig.sig.inputs.first_mut().unwrap());

        for (index, arg) in entrait_sig.sig.inputs.iter_mut().skip(1).enumerate() {
            explicitor.explicate_arg(index, arg);
        }
    } else {
        for (index, arg) in entrait_sig.sig.inputs.iter_mut().enumerate() {
            explicitor.explicate_arg(index, arg);
        }
    }

    explicitor.explicate_output(&mut entrait_sig.sig.output);

    for generated in explicitor.lifetimes.into_iter() {
        entrait_sig.fut_lifetimes.push(FutureLifetime {
            lifetime: generated.lifetime,
            kind: match generated.source {
                SigComponent::Receiver => LifetimeKind::Receiver,
                _ => LifetimeKind::Other,
            },
        });
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum SigComponent {
    Receiver,
    Indexed(usize),
    Output,
}

struct GeneratedLifetime {
    source: SigComponent,
    lifetime: syn::Lifetime,
}

struct Explicator {
    current: SigComponent,
    elided_inputs: HashSet<usize>,
    lifetimes: Vec<GeneratedLifetime>,
}

impl Explicator {
    fn explicate_receiver(&mut self, arg: &mut syn::FnArg) {
        self.current = SigComponent::Receiver;
        self.visit_fn_arg_mut(arg);
    }

    fn explicate_arg(&mut self, index: usize, arg: &mut syn::FnArg) {
        self.current = SigComponent::Indexed(index);
        self.visit_fn_arg_mut(arg);
    }

    fn explicate_output(&mut self, output: &mut syn::ReturnType) {
        self.current = SigComponent::Output;
        self.visit_return_type_mut(output);
    }

    fn make_lifetime_explicit(&mut self, lifetime: Option<syn::Lifetime>) -> syn::Lifetime {
        let lifetime = lifetime.unwrap_or_else(|| self.generate_or_find());
        self.register_lifetime(lifetime)
    }

    fn generate_or_find(&mut self) -> syn::Lifetime {
        match self.current {
            SigComponent::Output => self
                .find_lifetime_for_output()
                .unwrap_or_else(|| self.new_lifetime()),
            _ => self.new_lifetime(),
        }
    }

    fn find_lifetime_for_output(&self) -> Option<syn::Lifetime> {
        let from_component = match self.only_elided_input() {
            // If only one input was elided, use that input:
            Some(elided_input) => SigComponent::Indexed(elided_input),
            // If not, use the receiver lifetime:
            None => SigComponent::Receiver,
        };

        self.lifetimes
            .iter()
            .find(|generated| generated.source == from_component)
            .map(|generated| generated.lifetime.clone())
    }

    fn only_elided_input(&self) -> Option<usize> {
        if self.elided_inputs.len() == 1 {
            self.elided_inputs.iter().next().map(|index| *index)
        } else {
            None
        }
    }

    fn new_lifetime(&self) -> syn::Lifetime {
        let index = self.lifetimes.len();
        syn::Lifetime::new(
            &format!("'entrait{}", index),
            proc_macro2::Span::call_site(),
        )
    }

    fn register_lifetime(&mut self, lifetime: syn::Lifetime) -> syn::Lifetime {
        self.lifetimes.push(GeneratedLifetime {
            source: self.current,
            lifetime: lifetime.clone(),
        });
        lifetime
    }
}

impl<'s> syn::visit_mut::VisitMut for Explicator {
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

    fn visit_lifetime_mut(&mut self, i: &mut syn::Lifetime) {
        if i.ident == "_" {
            *i = self.make_lifetime_explicit(Some(i.clone()));
        }
    }
}

#[derive(Default)]
struct ElisionDetector {
    current_input: usize,
    elided_inputs: HashSet<usize>,
}

impl ElisionDetector {
    fn detect(&mut self, sig: &mut syn::Signature) {
        for (index, input) in sig.inputs.iter_mut().enumerate() {
            self.current_input = index;
            self.visit_fn_arg_mut(input);
        }
    }
}

impl syn::visit_mut::VisitMut for ElisionDetector {
    fn visit_type_reference_mut(&mut self, reference: &mut syn::TypeReference) {
        if reference.lifetime.is_none() {
            self.elided_inputs.insert(self.current_input);
        }
        syn::visit_mut::visit_type_reference_mut(self, reference);
    }

    fn visit_lifetime_mut(&mut self, i: &mut syn::Lifetime) {
        if i.ident == "_" {
            self.elided_inputs.insert(self.current_input);
        }
    }
}
