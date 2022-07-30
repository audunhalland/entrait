//! # entrait_macros
//!
//! Procedural macros used by entrait.
//!

pub mod attr;

mod analyze_generics;
mod signature;

use crate::generics;
use crate::input::InputFn;
use crate::opt::*;
use crate::token_util::{EmptyToken, Punctuator};
use attr::*;
use signature::EntraitSignature;

use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;

pub fn output_tokens(attr: &EntraitFnAttr, input_fn: InputFn) -> syn::Result<TokenStream> {
    let generics = analyze_generics::analyze_generics(&input_fn, attr)?;
    let entrait_sig = signature::SignatureConverter::new(attr, &input_fn, &generics.deps).convert();
    let trait_def = gen_trait_def(attr, &input_fn, &entrait_sig, &generics)?;
    let impl_block = gen_impl_block(attr, &input_fn, &entrait_sig, &generics);

    let InputFn {
        fn_attrs,
        fn_vis,
        fn_sig,
        fn_body,
        ..
    } = input_fn;

    Ok(quote! {
        #(#fn_attrs)* #fn_vis #fn_sig #fn_body
        #trait_def
        #impl_block
    })
}

fn gen_trait_def(
    attr: &EntraitFnAttr,
    input_fn: &InputFn,
    entrait_sig: &EntraitSignature,
    generics: &generics::Generics,
) -> syn::Result<TokenStream> {
    let span = attr.trait_ident.span();

    let opt_unimock_attr = attr.opt_unimock_attribute(entrait_sig, &generics.deps);
    let opt_entrait_for_trait_attr = match &generics.deps {
        generics::Deps::Concrete(_) => {
            Some(quote! { #[::entrait::entrait(unimock = false, mockall = false)] })
        }
        _ => None,
    };
    let opt_mockall_automock_attr = attr.opt_mockall_automock_attribute();
    let opt_async_trait_attr = input_fn.opt_async_trait_attribute(attr);

    let trait_visibility = &attr.trait_visibility;
    let trait_ident = &attr.trait_ident;
    let opt_associated_fut_decl = &entrait_sig.associated_fut_decl;
    let trait_fn_sig = &entrait_sig.sig;
    let generics = &generics.trait_generics;
    let where_clause = &generics.where_clause;

    Ok(quote_spanned! { span=>
        #opt_unimock_attr
        #opt_entrait_for_trait_attr
        #opt_mockall_automock_attr
        #opt_async_trait_attr
        #trait_visibility trait #trait_ident #generics #where_clause {
            #opt_associated_fut_decl
            #trait_fn_sig;
        }
    })
}

///
/// Generate code like
///
/// ```no_compile
/// impl<__T: ::entrait::Impl + Deps> Trait for __T {
///     fn the_func(&self, args...) {
///         the_func(self, args)
///     }
/// }
/// ```
///
fn gen_impl_block(
    attr: &EntraitFnAttr,
    input_fn: &InputFn,
    entrait_sig: &EntraitSignature,
    generics: &generics::Generics,
) -> TokenStream {
    let span = attr.trait_ident.span();

    let async_trait_attribute = input_fn.opt_async_trait_attribute(attr);
    let params = generics.params_generator(generics::UseAssociatedFuture(
        input_fn.use_associated_future(attr),
    ));
    let trait_ident = &attr.trait_ident;
    let args = generics.arguments_generator();
    let self_ty = SelfTy(generics, span);
    let where_clause = ImplWhereClause { generics, span };
    let mut input_fn_ident = input_fn.fn_sig.ident.clone();
    input_fn_ident.set_span(span);
    let associated_fut_impl = &entrait_sig.associated_fut_impl;

    let fn_item =
        gen_delegating_fn_item(span, input_fn, &input_fn_ident, entrait_sig, &generics.deps);

    quote_spanned! { span=>
        #async_trait_attribute
        impl #params #trait_ident #args for #self_ty #where_clause {
            #associated_fut_impl
            #fn_item
        }
    }
}

struct SelfTy<'g>(&'g generics::Generics, Span);

impl<'g> quote::ToTokens for SelfTy<'g> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let span = self.1;
        match &self.0.deps {
            generics::Deps::Generic { idents, .. } => idents.impl_path(span).to_tokens(tokens),
            generics::Deps::NoDeps { idents, .. } => idents.impl_path(span).to_tokens(tokens),
            generics::Deps::Concrete(ty) => ty.to_tokens(tokens),
        }
    }
}

/// Join where clauses from the input function and the required ones for Impl<T>
pub struct ImplWhereClause<'g> {
    generics: &'g generics::Generics,
    span: Span,
}

impl<'g> quote::ToTokens for ImplWhereClause<'g> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut punctuator = Punctuator::new(
            tokens,
            syn::token::Where(self.span),
            syn::token::Comma(self.span),
            EmptyToken,
        );

        // The where clause looks quite different depending on what kind of Deps is used in the function.
        match &self.generics.deps {
            generics::Deps::Generic { trait_bounds, .. } => {
                // Self bounds
                if !trait_bounds.is_empty() {
                    punctuator.push_fn(|tokens| {
                        syn::token::SelfType(self.span).to_tokens(tokens);
                        syn::token::Colon(self.span).to_tokens(tokens);

                        let n_bounds = trait_bounds.len();
                        for (index, bound) in trait_bounds.iter().enumerate() {
                            bound.to_tokens(tokens);
                            if index < n_bounds - 1 {
                                syn::token::Add(self.span).to_tokens(tokens);
                            }
                        }
                    });
                }
            }
            generics::Deps::NoDeps { .. } => {
                // Bounds for T are inline in params
            }
            generics::Deps::Concrete(_) => {
                // NOTE: the impl for Impl<T> is generated by invoking #[entrait] on the trait(!),
                // So we need only one impl here: for the path (the `T` in `Impl<T>`).
            }
        };

        if let Some(where_clause) = &self.generics.trait_generics.where_clause {
            for predicate in where_clause.predicates.iter() {
                punctuator.push(predicate);
            }
        }
    }
}

/// Generate the fn (in the impl block) that calls the entraited fn
fn gen_delegating_fn_item(
    span: Span,
    input_fn: &InputFn,
    fn_ident: &syn::Ident,
    entrait_sig: &EntraitSignature,
    deps: &generics::Deps,
) -> TokenStream {
    let trait_fn_sig = &entrait_sig.sig;

    struct SelfComma(Span);

    impl quote::ToTokens for SelfComma {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            syn::token::SelfValue(self.0).to_tokens(tokens);
            syn::token::Comma(self.0).to_tokens(tokens);
        }
    }

    let opt_self_comma = match (deps, entrait_sig.sig.inputs.first()) {
        (generics::Deps::NoDeps { .. }, _) | (_, None) => None,
        (_, Some(_)) => Some(SelfComma(span)),
    };

    let arguments = entrait_sig
        .sig
        .inputs
        .iter()
        .filter_map(|fn_arg| match fn_arg {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                syn::Pat::Ident(pat_ident) => Some(&pat_ident.ident),
                _ => panic!("Found a non-ident pattern, this should be handled in signature.rs"),
            },
        });

    let mut opt_dot_await = input_fn.opt_dot_await(span);
    if entrait_sig.associated_fut_decl.is_some() {
        opt_dot_await = None;
    }

    quote_spanned! { span=>
        #trait_fn_sig {
            #fn_ident(#opt_self_comma #(#arguments),*) #opt_dot_await
        }
    }
}

impl EntraitFnAttr {
    pub fn opt_unimock_attribute(
        &self,
        entrait_sig: &EntraitSignature,
        deps: &generics::Deps,
    ) -> Option<TokenStream> {
        match self.default_option(self.opts.unimock, false) {
            SpanOpt(true, span) => {
                let fn_ident = &entrait_sig.sig.ident;

                let unmocked = match deps {
                    generics::Deps::Generic { .. } => quote! { #fn_ident },
                    generics::Deps::Concrete(_) => quote! { _ },
                    generics::Deps::NoDeps { .. } => {
                        let arguments =
                            entrait_sig
                                .sig
                                .inputs
                                .iter()
                                .filter_map(|fn_arg| match fn_arg {
                                    syn::FnArg::Receiver(_) => None,
                                    syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                                        syn::Pat::Ident(pat_ident) => Some(&pat_ident.ident),
                                        _ => None,
                                    },
                                });

                        quote! { #fn_ident(#(#arguments),*) }
                    }
                };

                Some(self.gated_mock_attr(span, quote_spanned! {span=>
                    ::entrait::__unimock::unimock(prefix=::entrait::__unimock, mod=#fn_ident, as=[Fn], unmocked=[#unmocked])
                }))
            }
            _ => None,
        }
    }

    pub fn opt_mockall_automock_attribute(&self) -> Option<TokenStream> {
        match self.default_option(self.opts.mockall, false) {
            SpanOpt(true, span) => {
                Some(self.gated_mock_attr(span, quote_spanned! { span=> ::mockall::automock }))
            }
            _ => None,
        }
    }

    fn gated_mock_attr(&self, span: Span, attr: TokenStream) -> TokenStream {
        match self.export_value() {
            true => quote_spanned! {span=>
                #[#attr]
            },
            false => quote_spanned! {span=>
                #[cfg_attr(test, #attr)]
            },
        }
    }
}

impl InputFn {
    fn opt_dot_await(&self, span: Span) -> Option<TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote_spanned! { span=> .await })
        } else {
            None
        }
    }

    pub fn use_associated_future(&self, attr: &EntraitFnAttr) -> bool {
        matches!(
            (attr.async_strategy(), self.fn_sig.asyncness),
            (SpanOpt(AsyncStrategy::AssociatedFuture, _), Some(_async))
        )
    }

    fn opt_async_trait_attribute(&self, attr: &EntraitFnAttr) -> Option<TokenStream> {
        match (attr.async_strategy(), self.fn_sig.asyncness) {
            (SpanOpt(AsyncStrategy::AsyncTrait, span), Some(_async)) => {
                Some(quote_spanned! { span=> #[::entrait::__async_trait::async_trait] })
            }
            _ => None,
        }
    }
}
