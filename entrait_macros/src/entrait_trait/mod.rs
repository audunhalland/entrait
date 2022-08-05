//! Implementation for invoking entrait on a trait!

pub mod input_attr;

use input_attr::EntraitTraitAttr;

use crate::generics;
use crate::idents::CrateIdents;
use crate::idents::GenericIdents;
use crate::opt::*;
use crate::token_util::*;

use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::{quote, quote_spanned};

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
    let entrait = &attr.crate_idents.entrait;

    let opt_unimock_attr = if attr.opts.unimock.map(|opt| *opt.value()).unwrap_or(false) {
        Some(quote! {
            #[::#entrait::__unimock::unimock(prefix=::#entrait::__unimock)]
        })
    } else {
        None
    };
    let opt_mockall_automock_attr = if attr.opts.mockall.map(|opt| *opt.value()).unwrap_or(false) {
        Some(quote! { #[::mockall::automock] })
    } else {
        None
    };

    let delegation_trait = gen_delegation_trait(&generic_idents, &item_trait, &attr);

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

    let params = generics.impl_params_from_idents(
        &generic_idents,
        &generics::ImplIndirection::None,
        generics::UseAssociatedFuture(false),
    );
    let args = generics.arguments(&generics::ImplIndirection::None);
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
            syn::TraitItem::Type(trait_item_type) => {
                Some(impl_assoc_type(trait_item_type, &attr.crate_idents))
            }
            _ => None,
        });

    let method_items = item_trait
        .items
        .iter()
        .filter_map(|trait_item| match trait_item {
            syn::TraitItem::Method(method) => Some(gen_method(method, &generic_idents, &attr)),
            _ => None,
        });

    let tokens = quote! {
        #opt_unimock_attr
        #opt_mockall_automock_attr
        #item_trait

        #delegation_trait

        #(#impl_attrs)*
        impl #params #trait_ident #args for #self_ty #where_clause {
            #(#impl_assoc_types)*
            #(#method_items)*
        }
    };

    Ok(tokens)
}

fn gen_delegation_trait(
    generic_idents: &GenericIdents,
    input_trait: &syn::ItemTrait,
    attr: &EntraitTraitAttr,
) -> Option<TokenStream> {
    let entrait = &generic_idents.crate_idents.entrait;

    match &attr.delegation_kind {
        Some(SpanOpt(DelegationKind::ByTraitStatic(trait_ident), _)) => {
            let span = trait_ident.span();
            let impl_t = &generic_idents.impl_t;
            Some(quote_spanned! { span=>
                pub trait #trait_ident<#impl_t> {
                    type By: ::#entrait::BorrowImpl<#impl_t>;
                }
            })
        }
        Some(SpanOpt(DelegationKind::ByTraitDyn(trait_ident), _)) => {
            let mut trait_copy = input_trait.clone();

            trait_copy.ident = trait_ident.clone();
            trait_copy.supertraits.push(syn::parse_quote! { 'static });
            trait_copy.generics.params.insert(
                0,
                syn::parse_quote! {
                    EntraitT
                },
            );

            for item in trait_copy.items.iter_mut() {
                if let syn::TraitItem::Method(method) = item {
                    if !matches!(method.sig.inputs.first(), Some(syn::FnArg::Receiver(_))) {
                        continue;
                    }

                    method.sig.inputs.insert(
                        1,
                        syn::parse_quote! {
                            __impl: &::#entrait::Impl<EntraitT>
                        },
                    );
                }
            }

            Some(quote! { #trait_copy })
        }
        _ => None,
    }
}

fn gen_method(
    method: &syn::TraitItemMethod,
    generic_idents: &GenericIdents,
    attr: &EntraitTraitAttr,
) -> TokenStream {
    let fn_sig = &method.sig;
    let fn_ident = &fn_sig.ident;
    let impl_t = &generic_idents.impl_t;
    let entrait = &attr.crate_idents.entrait;

    let arguments = fn_sig.inputs.iter().filter_map(|arg| match arg {
        syn::FnArg::Receiver(_) => None,
        syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
            syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_token_stream()),
            _ => panic!("Found a non-ident pattern, this should be handled in signature.rs"),
        },
    });
    let opt_dot_await = fn_sig.asyncness.map(|_| quote! { .await });
    let core = &generic_idents.crate_idents.core;

    match &attr.delegation_kind {
        Some(SpanOpt(DelegationKind::ByTraitStatic(_), _)) => {
            quote! {
                #fn_sig {
                    <#impl_t::By as ::#entrait::BorrowImplRef<#impl_t>>::Ref::from(self).#fn_ident(#(#arguments),*) #opt_dot_await
                }
            }
        }
        Some(SpanOpt(DelegationKind::ByTraitDyn(trait_ident), _)) => {
            quote! {
                #fn_sig {
                    <#impl_t as ::#core::borrow::Borrow<dyn #trait_ident<#impl_t>>>::borrow(&*self)
                        .#fn_ident(self, #(#arguments),*) #opt_dot_await
                }
            }
        }
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

fn impl_assoc_type(assoc_type: &syn::TraitItemType, crate_idents: &CrateIdents) -> TokenStream {
    if let Some(future_arguments) = assoc_type.bounds.iter().find_map(find_future_arguments) {
        let ident = &assoc_type.ident;
        let generics = &assoc_type.generics;
        let where_clause = &generics.where_clause;
        let core = &crate_idents.core;

        quote! {
            type #ident #generics = impl ::#core::future::Future #future_arguments #where_clause;
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
    fn push_impl_t_bounds(&self, stream: &mut TokenStream) {
        use syn::token::*;

        push_tokens!(stream, self.generic_idents.impl_t, Colon(self.span));

        match &self.attr.delegation_kind {
            Some(SpanOpt(DelegationKind::ByTraitStatic(trait_ident), _)) => {
                push_tokens!(
                    stream,
                    trait_ident,
                    Lt(self.span),
                    self.generic_idents.impl_t,
                    Gt(self.span)
                );
            }
            Some(SpanOpt(DelegationKind::ByTraitDyn(trait_ident), _)) => {
                self.push_core_borrow_borrow(stream);
                push_tokens!(
                    stream,
                    // Generic arguments:
                    Lt(self.span),
                    Dyn(self.span),
                    trait_ident,
                    Lt(self.span),
                    self.generic_idents.impl_t,
                    Gt(self.span),
                    Gt(self.span)
                );

                if self.contains_async {
                    push_tokens!(stream, self.plus_send(), self.plus_sync());
                }
                push_tokens!(stream, self.plus_static());
            }
            Some(SpanOpt(DelegationKind::ByBorrow, _)) => {
                self.push_core_borrow_borrow(stream);
                push_tokens!(
                    stream,
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

    fn push_delegate_borrow_impl_ref_bounds(&self, stream: &mut TokenStream) {
        use syn::token::*;
        use syn::Ident;

        let span = self.span;

        let impl_lifetime = syn::Lifetime::new("'impl_life", self.span);

        push_tokens!(
            stream,
            // for<'i>
            For(span),
            Lt(span),
            impl_lifetime,
            Gt(span),
            // <T::By as ::entrait::BorrowImplRef<'i, T>>
            Lt(span),
            self.generic_idents.impl_t,
            Colon2(span),
            Ident::new("By", span),
            As(span),
            Colon2(span),
            self.attr.crate_idents.entrait,
            Colon2(span),
            Ident::new("BorrowImplRef", span),
            Lt(span),
            impl_lifetime,
            Comma(span),
            self.generic_idents.impl_t,
            Gt(span),
            Gt(span),
            // ::Ref: #trait_ident
            Colon2(span),
            Ident::new("Ref", span),
            Colon(span),
            self.item_trait.ident
        );
    }

    fn push_core_borrow_borrow(&self, stream: &mut TokenStream) {
        use syn::token::*;
        push_tokens!(
            stream,
            Colon2(self.span),
            self.generic_idents.crate_idents.core,
            Colon2(self.span),
            syn::Ident::new("borrow", self.span),
            Colon2(self.span),
            syn::Ident::new("Borrow", self.span)
        );
    }

    fn trait_with_arguments(&self) -> TokenPair<impl ToTokens + '_, impl ToTokens + '_> {
        TokenPair(
            &self.item_trait.ident,
            self.trait_generics
                .arguments(&generics::ImplIndirection::None),
        )
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
            self.push_impl_t_bounds(stream);
        });

        // Trait delegation bounds:
        if let Some(SpanOpt(DelegationKind::ByTraitStatic(_), _)) = &self.attr.delegation_kind {
            punctuator.push_fn(|stream| {
                self.push_delegate_borrow_impl_ref_bounds(stream);
            });
        }

        for predicate in &self.trait_generics.where_predicates {
            punctuator.push(predicate);
        }
    }
}
