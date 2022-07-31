//! Implementation for invoking entrait on a trait!

use crate::generics;
use crate::opt::*;
use crate::token_util::*;

use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream};

pub struct EntraitTraitAttr {
    pub opts: Opts,
    pub delegation_kind: Option<SpanOpt<DelegationKind>>,
}

impl Parse for EntraitTraitAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut debug = None;
        let mut unimock = None;
        let mut mockall = None;
        let mut delegation_kind = None;

        if !input.is_empty() {
            loop {
                match input.parse::<EntraitOpt>()? {
                    EntraitOpt::Unimock(opt) => unimock = Some(opt),
                    EntraitOpt::Mockall(opt) => mockall = Some(opt),
                    EntraitOpt::Debug(opt) => debug = Some(opt),
                    EntraitOpt::DelegateBy(kind) => delegation_kind = Some(kind),
                    entrait_opt => {
                        return Err(syn::Error::new(entrait_opt.span(), "Unsupported option"))
                    }
                };

                if input.peek(syn::token::Comma) {
                    input.parse::<syn::token::Comma>()?;
                } else {
                    break;
                }
            }
        }

        Ok(Self {
            opts: Opts {
                no_deps: None,
                debug,
                async_strategy: None,
                export: None,
                unimock,
                mockall,
            },
            delegation_kind,
        })
    }
}

pub fn output_tokens(
    attr: EntraitTraitAttr,
    item_trait: syn::ItemTrait,
) -> syn::Result<proc_macro2::TokenStream> {
    let generics = generics::Generics::new(
        generics::FnDeps::NoDeps {
            idents: generics::GenericIdents::new(item_trait.ident.span()),
        },
        item_trait.generics.clone(),
    );
    let generic_idents = match &generics.deps {
        generics::FnDeps::NoDeps { idents } => idents,
        _ => panic!(),
    };
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

    let params = generics.params_generator(generics::UseAssociatedFuture(false));
    let args = generics.arguments_generator();
    let self_ty = generic_idents.impl_path(item_trait.ident.span());
    let where_clause = ImplWhereClause {
        item_trait: &item_trait,
        contains_async,
        generics: &generics,
        generic_idents,
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

struct ImplWhereClause<'g> {
    item_trait: &'g syn::ItemTrait,
    contains_async: bool,
    generics: &'g generics::Generics,
    generic_idents: &'g generics::GenericIdents,
    attr: &'g EntraitTraitAttr,
    span: proc_macro2::Span,
}

impl<'g> ImplWhereClause<'g> {
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
        TokenPair(&self.item_trait.ident, self.generics.arguments_generator())
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

impl<'g> quote::ToTokens for ImplWhereClause<'g> {
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

        if let Some(where_clause) = &self.generics.trait_generics.where_clause {
            for predicate in &where_clause.predicates {
                punctuator.push(predicate);
            }
        }
    }
}
