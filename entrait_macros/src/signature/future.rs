use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;

use crate::generics::TraitIndirection;
use crate::idents::CrateIdents;
use crate::token_util::EmptyToken;
use crate::token_util::Punctuator;

use super::AssociatedFut;
use super::EntraitSignature;

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
