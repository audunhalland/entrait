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
use crate::token_util::{push_tokens, EmptyToken, Punctuator, TokenPair};
use attr::*;

use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;

struct OutputFn<'i> {
    source: &'i InputFn,
    generics: generics::Generics,
    entrait_sig: signature::EntraitSignature,
}

impl<'i> OutputFn<'i> {
    fn analyze(source: &'i InputFn, attr: &EntraitFnAttr) -> syn::Result<Self> {
        let generics = analyze_generics::analyze_generics(&source, attr)?;
        let entrait_sig =
            signature::SignatureConverter::new(attr, &source, &generics.deps).convert();
        Ok(Self {
            source,
            generics,
            entrait_sig,
        })
    }

    fn sig(&self) -> &syn::Signature {
        &self.entrait_sig.sig
    }
}

pub fn gen_single_fn(attr: &EntraitFnAttr, input_fn: InputFn) -> syn::Result<TokenStream> {
    let output_fn = OutputFn::analyze(&input_fn, attr)?;
    let trait_def = gen_trait_def(attr, &output_fn)?;
    let impl_block = gen_impl_block(attr, &output_fn);

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

fn gen_trait_def(attr: &EntraitFnAttr, output_fn: &OutputFn) -> syn::Result<TokenStream> {
    let span = attr.trait_ident.span();

    let opt_unimock_attr = attr.opt_unimock_attribute(output_fn);
    let opt_entrait_for_trait_attr = match &output_fn.generics.deps {
        generics::Deps::Concrete(_) => {
            Some(quote! { #[::entrait::entrait(unimock = false, mockall = false)] })
        }
        _ => None,
    };
    let opt_mockall_automock_attr = attr.opt_mockall_automock_attribute();
    let opt_async_trait_attr = output_fn.source.opt_async_trait_attribute(attr);

    let trait_visibility = &attr.trait_visibility;
    let trait_ident = &attr.trait_ident;
    let opt_associated_fut_decl = &output_fn.entrait_sig.associated_fut_decl;
    let trait_fn_sig = &output_fn.sig();
    let generics = &output_fn.generics.trait_generics;
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
fn gen_impl_block(attr: &EntraitFnAttr, output_fn: &OutputFn) -> TokenStream {
    let span = attr.trait_ident.span();
    let generics = &output_fn.generics;

    let async_trait_attribute = output_fn.source.opt_async_trait_attribute(attr);
    let params = generics.params_generator(generics::UseAssociatedFuture(
        output_fn.source.use_associated_future(attr),
    ));
    let trait_ident = &attr.trait_ident;
    let args = generics.arguments_generator();
    let self_ty = SelfTy(generics, span);
    let where_clause = ImplWhereClause { generics, span };
    let associated_fut_impl = &output_fn.entrait_sig.associated_fut_impl;

    let fn_item = gen_delegating_fn_item(output_fn, span);

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
    fn to_tokens(&self, stream: &mut TokenStream) {
        let span = self.1;
        match &self.0.deps {
            generics::Deps::Generic { idents, .. } => push_tokens!(stream, idents.impl_path(span)),
            generics::Deps::NoDeps { idents, .. } => push_tokens!(stream, idents.impl_path(span)),
            generics::Deps::Concrete(ty) => push_tokens!(stream, ty),
        }
    }
}

/// Join where clauses from the input function and the required ones for Impl<T>
pub struct ImplWhereClause<'g> {
    generics: &'g generics::Generics,
    span: Span,
}

impl<'g> quote::ToTokens for ImplWhereClause<'g> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Where(self.span),
            syn::token::Comma(self.span),
            EmptyToken,
        );

        // The where clause looks quite different depending on what kind of Deps is used in the function.
        match &self.generics.deps {
            generics::Deps::Generic { trait_bounds, .. } => {
                // Self bounds
                if !trait_bounds.is_empty() {
                    punctuator.push_fn(|stream| {
                        push_tokens!(
                            stream,
                            syn::token::SelfType(self.span),
                            syn::token::Colon(self.span)
                        );

                        let n_bounds = trait_bounds.len();
                        for (index, bound) in trait_bounds.iter().enumerate() {
                            push_tokens!(stream, bound);
                            if index < n_bounds - 1 {
                                push_tokens!(stream, syn::token::Add(self.span));
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
fn gen_delegating_fn_item(output_fn: &OutputFn, span: Span) -> TokenStream {
    let entrait_sig = &output_fn.entrait_sig;
    let trait_fn_sig = &output_fn.sig();
    let deps = &output_fn.generics.deps;

    let mut fn_ident = output_fn.source.fn_sig.ident.clone();
    fn_ident.set_span(span);

    let opt_self_comma = match (deps, entrait_sig.sig.inputs.first()) {
        (generics::Deps::NoDeps { .. }, _) | (_, None) => None,
        (_, Some(_)) => Some(TokenPair(
            syn::token::SelfValue(span),
            syn::token::Comma(span),
        )),
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

    let mut opt_dot_await = output_fn.source.opt_dot_await(span);
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
    fn opt_unimock_attribute(&self, output_fn: &OutputFn) -> Option<TokenStream> {
        match self.default_option(self.opts.unimock, false) {
            SpanOpt(true, span) => {
                let fn_ident = &output_fn.sig().ident;

                let unmocked = match &output_fn.generics.deps {
                    generics::Deps::Generic { .. } => quote! { #fn_ident },
                    generics::Deps::Concrete(_) => quote! { _ },
                    generics::Deps::NoDeps { .. } => {
                        let arguments =
                            output_fn
                                .sig()
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
