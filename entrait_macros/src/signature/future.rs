use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote_spanned;

use crate::idents::CrateIdents;

use super::lifetimes;
use super::EntraitSignature;
use super::FnIndex;
use super::ReceiverGeneration;
use super::SigComponent;

impl EntraitSignature {
    pub fn convert_to_associated_future(
        &mut self,
        fn_index: FnIndex,
        receiver_generation: ReceiverGeneration,
        span: Span,
        crate_idents: &CrateIdents,
    ) {
        lifetimes::de_elide_lifetimes(self, receiver_generation);

        let output_ty = output_type_tokens(&self.sig.output);

        let fut_lifetimes = self
            .lifetimes
            .iter()
            .map(|ft| &ft.lifetime)
            .collect::<Vec<_>>();
        let self_lifetimes = self
            .lifetimes
            .iter()
            .filter(|ft| ft.source == SigComponent::Receiver)
            .map(|ft| &ft.lifetime)
            .collect::<Vec<_>>();

        // make the function generic if it wasn't already
        let sig = &mut self.sig;
        sig.asyncness = None;
        let generics = &mut sig.generics;
        generics.lt_token.get_or_insert(syn::parse_quote! { < });
        generics.gt_token.get_or_insert(syn::parse_quote! { > });

        // insert generated/non-user-provided lifetimes
        for fut_lifetime in self.lifetimes.iter().filter(|lt| !lt.user_provided.0) {
            generics
                .params
                .push(syn::GenericParam::Lifetime(syn::LifetimeDef {
                    attrs: vec![],
                    lifetime: fut_lifetime.lifetime.clone(),
                    colon_token: None,
                    bounds: syn::punctuated::Punctuated::new(),
                }));
        }

        let fut = quote::format_ident!("Fut{}", fn_index.0);

        self.sig.output = syn::parse_quote_spanned! {span =>
            -> Self::#fut<#(#fut_lifetimes),*>
        };

        let core = &crate_idents.core;

        self.associated_fut_decl = Some(quote_spanned! { span=>
            type #fut<#(#fut_lifetimes),*>: ::#core::future::Future<Output = #output_ty> + Send
            where
                Self: #(#self_lifetimes)+*;
        });

        self.associated_fut_impl = Some(quote_spanned! { span=>
            type #fut<#(#fut_lifetimes),*> = impl ::#core::future::Future<Output = #output_ty>
            where
                Self: #(#self_lifetimes)+*;
        });
    }
}

fn output_type_tokens(return_type: &syn::ReturnType) -> TokenStream {
    use quote::quote;

    match return_type {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    }
}
