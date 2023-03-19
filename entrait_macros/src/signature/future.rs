use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;

use crate::generics::TraitIndirection;
use crate::idents::CrateIdents;
use crate::token_util::EmptyToken;
use crate::token_util::Punctuator;

use super::lifetimes;
use super::AssociatedFut;
use super::EntraitSignature;
use super::ReceiverGeneration;
use super::SigComponent;
use super::UsedInOutput;
use super::UserProvidedLifetime;

impl EntraitSignature {
    pub fn convert_to_associated_future(
        &mut self,
        receiver_generation: ReceiverGeneration,
        trait_span: Span,
    ) {
        lifetimes::de_elide_lifetimes(self, receiver_generation);

        let base_lifetime = syn::Lifetime::new("'entrait_future", Span::call_site());
        self.et_lifetimes.push(super::EntraitLifetime {
            lifetime: base_lifetime.clone(),
            source: SigComponent::Base,
            user_provided: UserProvidedLifetime(false),
            used_in_output: UsedInOutput(false),
        });

        let output = clone_output_type(&self.sig.output);

        // make the function generic if it wasn't already
        let sig = &mut self.sig;
        sig.asyncness = None;
        let generics = &mut sig.generics;
        generics.lt_token.get_or_insert(syn::parse_quote! { < });
        generics.gt_token.get_or_insert(syn::parse_quote! { > });

        // insert generated/non-user-provided lifetimes
        for fut_lifetime in self.et_lifetimes.iter().filter(|lt| !lt.user_provided.0) {
            generics
                .params
                .push(syn::GenericParam::Lifetime(syn::LifetimeParam {
                    attrs: vec![],
                    lifetime: fut_lifetime.lifetime.clone(),
                    colon_token: None,
                    bounds: syn::punctuated::Punctuated::new(),
                }));
        }

        let fut_ident = quote::format_ident!("Fut__{}", sig.ident);

        let fut_lifetimes = self
            .et_lifetimes_in_assoc_future()
            .map(|et| &et.lifetime)
            .collect::<Vec<_>>();

        self.sig.output = syn::parse_quote_spanned! { trait_span =>
            -> Self::#fut_ident<#(#fut_lifetimes),*>
        };

        let sig_where_clause = self.sig.generics.make_where_clause();
        for lifetime in &self.et_lifetimes {
            if !matches!(lifetime.source, SigComponent::Base) {
                let lt = &lifetime.lifetime;

                sig_where_clause.predicates.push(syn::parse_quote! {
                    #lt: #base_lifetime
                });
            }
        }
        sig_where_clause.predicates.push(syn::parse_quote! {
            Self: #base_lifetime
        });

        self.associated_fut = Some(AssociatedFut {
            ident: fut_ident,
            output,
            base_lifetime,
        });
    }
}

fn clone_output_type(return_type: &syn::ReturnType) -> syn::Type {
    match return_type {
        syn::ReturnType::Default => syn::parse_quote! { () },
        syn::ReturnType::Type(_, ty) => ty.as_ref().clone(),
    }
}

pub struct FutDecl<'s> {
    pub signature: &'s EntraitSignature,
    pub associated_fut: &'s AssociatedFut,
    pub trait_indirection: TraitIndirection,
    pub crate_idents: &'s CrateIdents,
}

impl<'s> ToTokens for FutDecl<'s> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let ident = &self.associated_fut.ident;
        let core = &self.crate_idents.core;
        let output = &self.associated_fut.output;
        let base_lifetime = &self.associated_fut.base_lifetime;

        let params = FutParams {
            signature: self.signature,
        };
        let where_clause = FutWhereClause {
            signature: self.signature,
            trait_indirection: self.trait_indirection,
            associated_fut: self.associated_fut,
        };

        let tokens = quote! {
            #[allow(non_camel_case_types)]
            type #ident #params: ::#core::future::Future<Output = #output> + Send + #base_lifetime #where_clause;
        };

        tokens.to_tokens(stream);
    }
}

pub struct FutImpl<'s> {
    pub signature: &'s EntraitSignature,
    pub associated_fut: &'s AssociatedFut,
    pub trait_indirection: TraitIndirection,
    pub crate_idents: &'s CrateIdents,
}

impl<'s> ToTokens for FutImpl<'s> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let ident = &self.associated_fut.ident;
        let core = &self.crate_idents.core;
        let output = &self.associated_fut.output;

        let params = FutParams {
            signature: self.signature,
        };
        let fut_bounds = FutImplBounds {
            associated_fut: self.associated_fut,
        };
        let where_clause = FutWhereClause {
            signature: self.signature,
            trait_indirection: self.trait_indirection,
            associated_fut: self.associated_fut,
        };

        let tokens = quote! {
            #[allow(non_camel_case_types)]
            type #ident #params = impl ::#core::future::Future<Output = #output> #fut_bounds #where_clause;
        };
        tokens.to_tokens(stream);
    }
}

struct FutParams<'s> {
    signature: &'s EntraitSignature,
}

impl<'s> ToTokens for FutParams<'s> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Lt::default(),
            syn::token::Comma::default(),
            syn::token::Gt::default(),
        );

        for lt in self.signature.et_lifetimes_in_assoc_future() {
            punctuator.push(&lt.lifetime);
        }
    }
}

struct FutWhereClause<'s> {
    signature: &'s EntraitSignature,
    trait_indirection: TraitIndirection,
    associated_fut: &'s AssociatedFut,
}

impl<'s> ToTokens for FutWhereClause<'s> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let base_lifetime = &self.associated_fut.base_lifetime;
        let mut punctuator = Punctuator::new(
            stream,
            quote! { where },
            syn::token::Comma::default(),
            EmptyToken,
        );

        for et_lifetime in self.signature.et_lifetimes_in_assoc_future_except_base() {
            let lt = &et_lifetime.lifetime;

            punctuator.push(quote! {
                #lt: #base_lifetime
            });
        }

        punctuator.push_fn(|stream| {
            let bound_target = match self.trait_indirection {
                TraitIndirection::StaticImpl | TraitIndirection::DynamicImpl => quote! { EntraitT },
                TraitIndirection::Plain | TraitIndirection::Trait => quote! { Self },
            };

            let outlives = self
                .signature
                .et_lifetimes_in_assoc_future()
                .map(|et| &et.lifetime);

            stream.extend(quote! {
                #bound_target: #(#outlives)+*
            });
        });
    }
}

struct FutImplBounds<'s> {
    associated_fut: &'s AssociatedFut,
}

impl<'s> ToTokens for FutImplBounds<'s> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Plus::default(),
            syn::token::Plus::default(),
            EmptyToken,
        );

        punctuator.push(syn::Ident::new("Send", proc_macro2::Span::call_site()));
        punctuator.push(&self.associated_fut.base_lifetime);
    }
}
