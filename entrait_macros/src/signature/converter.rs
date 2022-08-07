use super::{fn_params, ReceiverGeneration};
use super::{EntraitSignature, FnIndex, ImplReceiverKind, InputSig};
use crate::{generics::FnDeps, idents::CrateIdents, opt::Opts};

use proc_macro2::Span;
use syn::spanned::Spanned;

pub struct SignatureConverter<'a> {
    pub crate_idents: &'a CrateIdents,
    pub trait_span: Span,
    pub opts: &'a Opts,
    pub input_sig: InputSig<'a>,
    pub deps: &'a FnDeps,
    pub impl_receiver_kind: ImplReceiverKind,
    pub fn_index: FnIndex,
}

impl<'a> SignatureConverter<'a> {
    /// Convert from an standalone `fn` signature to a trait `fn` signature.
    pub fn convert_fn_to_trait_fn(&self) -> EntraitSignature {
        let mut entrait_sig = EntraitSignature::new(self.input_sig.sig.clone());

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
        self.generate_params(&mut entrait_sig.sig, receiver_generation);

        if self.input_sig.use_associated_future(self.opts) {
            entrait_sig.convert_to_associated_future(
                self.fn_index,
                receiver_generation,
                self.trait_span,
                &self.crate_idents,
            );
        }

        self.remove_generic_type_params(&mut entrait_sig.sig);
        tidy_generics(&mut entrait_sig.sig.generics);

        fn_params::fix_fn_param_idents(&mut entrait_sig.sig);

        entrait_sig
    }

    fn detect_receiver_generation(&self, sig: &syn::Signature) -> ReceiverGeneration {
        match self.deps {
            FnDeps::NoDeps { .. } => ReceiverGeneration::Insert,
            _ => {
                if sig.inputs.is_empty() {
                    if self.input_sig.use_associated_future(self.opts) {
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

    fn generate_params(&self, sig: &mut syn::Signature, receiver_generation: ReceiverGeneration) {
        let span = self.trait_span;
        match receiver_generation {
            ReceiverGeneration::Insert => {
                sig.inputs.insert(
                    0,
                    self.gen_first_receiver(
                        Span::call_site(),
                        Some((syn::token::And::default(), None)),
                    ),
                );
            }
            ReceiverGeneration::Rewrite => {
                let input = sig.inputs.first_mut().unwrap();
                let input_span = input.span();
                match input {
                    syn::FnArg::Typed(pat_type) => match pat_type.ty.as_ref() {
                        syn::Type::Reference(type_reference) => {
                            let and_token = type_reference.and_token;
                            let lifetime = type_reference.lifetime.clone();

                            *input = self
                                .gen_first_receiver(pat_type.span(), Some((and_token, lifetime)));
                        }
                        _ => {
                            let first_mut = sig.inputs.first_mut().unwrap();
                            *first_mut = self.gen_first_receiver(input_span, None);
                        }
                    },
                    syn::FnArg::Receiver(_) => panic!(),
                }
            }
            ReceiverGeneration::None => {}
        }

        if matches!(self.impl_receiver_kind, ImplReceiverKind::DynamicImpl) {
            sig.inputs
                .insert(1, self.gen_impl_receiver(Span::call_site()));
        }
    }

    fn gen_first_receiver(
        &self,
        span: Span,
        reference: Option<(syn::token::And, Option<syn::Lifetime>)>,
    ) -> syn::FnArg {
        match &self.impl_receiver_kind {
            ImplReceiverKind::SelfRef | ImplReceiverKind::DynamicImpl => {
                self.gen_self_receiver(span, reference)
            }
            ImplReceiverKind::StaticImpl => self.gen_impl_receiver(span),
        }
    }

    fn gen_self_receiver(
        &self,
        span: Span,
        reference: Option<(syn::token::And, Option<syn::Lifetime>)>,
    ) -> syn::FnArg {
        syn::FnArg::Receiver(syn::Receiver {
            attrs: vec![],
            reference: reference,
            mutability: None,
            self_token: syn::token::SelfValue(span),
        })
    }

    fn gen_impl_receiver(&self, span: Span) -> syn::FnArg {
        let entrait = &self.crate_idents.entrait;
        syn::parse_quote! {
            __impl: &::#entrait::Impl<EntraitT>
        }
    }

    fn remove_generic_type_params(&self, sig: &mut syn::Signature) {
        let deps_ident = match &self.deps {
            FnDeps::Generic { generic_param, .. } => generic_param.as_ref(),
            _ => None,
        };

        let generics = &mut sig.generics;
        let mut params = syn::punctuated::Punctuated::new();
        std::mem::swap(&mut params, &mut generics.params);

        for param in params.into_iter() {
            match &param {
                syn::GenericParam::Type(_) => {}
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
                        if let Some(deps_ident) = &deps_ident {
                            if !is_type_eq_ident(&pred.bounded_ty, deps_ident) {
                                where_clause.predicates.push(predicate);
                            }
                        } else {
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
}

fn is_type_eq_ident(ty: &syn::Type, ident: &syn::Ident) -> bool {
    match ty {
        syn::Type::Path(type_path) if type_path.path.segments.len() == 1 => {
            type_path.path.segments.first().unwrap().ident == *ident
        }
        _ => false,
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
