//! Implementation for invoking entrait on a trait!

pub mod input_attr;

use input_attr::EntraitTraitAttr;

use crate::generics;
use crate::idents::GenericIdents;
use crate::opt::*;
use crate::token_util::*;

use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;

pub fn output_tokens(
    attr: EntraitTraitAttr,
    item_trait: syn::ItemTrait,
) -> syn::Result<proc_macro2::TokenStream> {
    let generics = generics::TraitGenerics {
        params: item_trait.generics.params.clone(),
        where_predicates: item_trait
            .generics
            .where_clause
            .as_ref()
            .map(|where_clause| where_clause.predicates.clone())
            .unwrap_or_default(),
    };
    let generic_idents = GenericIdents::new(&attr.crate_idents, item_trait.ident.span());
    let trait_ident = &item_trait.ident;

    // NOTE: all of the trait _input attributes_ are outputted, unchanged

    let contains_async = item_trait.items.iter().any(|item| match item {
        syn::TraitItem::Method(method) => method.sig.asyncness.is_some(),
        _ => false,
    });

    let opt_unimock_attr = if attr.opts.unimock.map(|opt| *opt.value()).unwrap_or(false) {
        Some(quote! {
            #[::entrait::__unimock::unimock(prefix=::entrait::__unimock)]
        })
    } else {
        None
    };
    let opt_mockall_automock_attr = if attr.opts.mockall.map(|opt| *opt.value()).unwrap_or(false) {
        Some(quote! { #[::mockall::automock] })
    } else {
        None
    };

    let impl_attrs = item_trait
        .attrs
        .iter()
        .filter(|attr| {
            matches!(
                attr.path.segments.last(),
                Some(last_segment) if last_segment.ident == "async_trait"
            )
        })
        .collect::<Vec<_>>();

    let params =
        generics.impl_params_from_idents(&generic_idents, generics::UseAssociatedFuture(false));
    let args = generics.arguments();
    let self_ty = generic_idents.impl_path(item_trait.ident.span());
    let where_clause = ImplWhereClause {
        item_trait: &item_trait,
        contains_async,
        trait_generics: &generics,
        generic_idents: &generic_idents,
        attr: &attr,
        span: item_trait.ident.span(),
    };

    let impl_assoc_types = item_trait
        .items
        .iter()
        .filter_map(|trait_item| match trait_item {
            syn::TraitItem::Type(trait_item_type) => Some(impl_assoc_type(trait_item_type)),
            _ => None,
        });

    let method_items = item_trait
        .items
        .iter()
        .filter_map(|trait_item| match trait_item {
            syn::TraitItem::Method(method) => Some(gen_method(method, &attr)),
            _ => None,
        });

    let tokens = quote! {
        #opt_unimock_attr
        #opt_mockall_automock_attr
        #item_trait

        #(#impl_attrs)*
        impl #params #trait_ident #args for #self_ty #where_clause {
            #(#impl_assoc_types)*
            #(#method_items)*
        }
    };

    Ok(tokens)
}

fn gen_method(method: &syn::TraitItemMethod, attr: &EntraitTraitAttr) -> TokenStream {
    let fn_sig = &method.sig;
    let fn_ident = &fn_sig.ident;
    let arguments = fn_sig.inputs.iter().filter_map(|arg| match arg {
        syn::FnArg::Receiver(_) => None,
        syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
            syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_token_stream()),
            _ => panic!("Found a non-ident pattern, this should be handled in signature.rs"),
        },
    });
    let opt_dot_await = fn_sig.asyncness.map(|_| quote! { .await });

    match &attr.delegation_kind {
        Some(SpanOpt(DelegationKind::ByBorrow, _)) => {
            quote! {
                #fn_sig {
                    self.as_ref().borrow().#fn_ident(#(#arguments),*) #opt_dot_await
                }
            }
        }
        _ => {
            quote! {
                #fn_sig {
                    self.as_ref().#fn_ident(#(#arguments),*) #opt_dot_await
                }
            }
        }
    }
}

fn impl_assoc_type(assoc_type: &syn::TraitItemType) -> TokenStream {
    if let Some(future_arguments) = assoc_type.bounds.iter().find_map(find_future_arguments) {
        let ident = &assoc_type.ident;
        let generics = &assoc_type.generics;
        let where_clause = &generics.where_clause;

        quote! {
            type #ident #generics = impl ::core::future::Future #future_arguments #where_clause;
        }
    } else {
        // Probably a compile error
        quote! { #assoc_type }
    }
}

fn find_future_arguments(bound: &syn::TypeParamBound) -> Option<&syn::PathArguments> {
    match bound {
        syn::TypeParamBound::Trait(trait_bound) => match trait_bound.path.segments.last() {
            Some(segment) if segment.ident == "Future" => Some(&segment.arguments),
            _ => None,
        },
        _ => None,
    }
}

struct ImplWhereClause<'g, 'c> {
    item_trait: &'g syn::ItemTrait,
    contains_async: bool,
    trait_generics: &'g generics::TraitGenerics,
    generic_idents: &'g GenericIdents<'c>,
    attr: &'g EntraitTraitAttr,
    span: proc_macro2::Span,
}

impl<'g, 'c> ImplWhereClause<'g, 'c> {
    fn impl_t_bounds(&self, stream: &mut TokenStream) {
        push_tokens!(
            stream,
            self.generic_idents.impl_t,
            syn::token::Colon(self.span)
        );

        match &self.attr.delegation_kind {
            Some(SpanOpt(DelegationKind::ByBorrow, _)) => {
                use syn::token::*;

                use syn::Ident;

                push_tokens!(
                    stream,
                    Colon2(self.span),
                    Ident::new("core", self.span),
                    Colon2(self.span),
                    syn::Ident::new("borrow", self.span),
                    Colon2(self.span),
                    syn::Ident::new("Borrow", self.span),
                    // Generic arguments:
                    Lt(self.span),
                    Dyn(self.span),
                    self.trait_with_arguments(),
                    Gt(self.span)
                );

                if self.contains_async {
                    push_tokens!(stream, self.plus_send(), self.plus_sync());
                }
                push_tokens!(stream, self.plus_static());
            }
            _ => {
                push_tokens!(stream, self.trait_with_arguments(), self.plus_sync());
            }
        }
    }

    fn trait_with_arguments(&self) -> TokenPair<impl ToTokens + '_, impl ToTokens + '_> {
        TokenPair(&self.item_trait.ident, self.trait_generics.arguments())
    }

    fn plus_static(&self) -> TokenPair<impl ToTokens, impl ToTokens> {
        TokenPair(
            syn::token::Add(self.span),
            syn::Lifetime::new("'static", self.span),
        )
    }

    fn plus_send(&self) -> TokenPair<impl ToTokens, impl ToTokens> {
        TokenPair(
            syn::token::Add(self.span),
            syn::Ident::new("Send", self.span),
        )
    }

    fn plus_sync(&self) -> TokenPair<impl ToTokens, impl ToTokens> {
        TokenPair(
            syn::token::Add(self.span),
            syn::Ident::new("Sync", self.span),
        )
    }
}

impl<'g, 'c> quote::ToTokens for ImplWhereClause<'g, 'c> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Where(self.span),
            syn::token::Comma(self.span),
            EmptyToken,
        );

        // Bounds on the `T` in `Impl<T>`:
        punctuator.push_fn(|stream| {
            self.impl_t_bounds(stream);
        });

        for predicate in &self.trait_generics.where_predicates {
            punctuator.push(predicate);
        }
    }
}
