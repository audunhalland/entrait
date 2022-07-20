//! # entrait_macros
//!
//! Procedural macros used by entrait.
//!

pub mod input;

mod analyze_generics;
mod signature;
mod for_trait;

use crate::util::generics;
use crate::util::opt::*;
use input::*;
use signature::EntraitSignature;

use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;
use quote::ToTokens;
use syn::parse_quote;

pub fn invoke(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
    opts_modifier: impl FnOnce(&mut Opts),
) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as Input);

    let (result, debug) = match input {
        Input::Fn(input_fn) => {
            let mut attr = syn::parse_macro_input!(attr as EntraitAttr);
            opts_modifier(&mut attr.opts);

            (output_tokens(&attr, input_fn), attr.debug_value())
        }
        Input::Trait(item_trait) => {
            let mut attr = syn::parse_macro_input!(attr as for_trait::DelegateImplAttr);
            opts_modifier(&mut attr.opts);
            let debug = attr.opts.debug.map(|opt| *opt.value()).unwrap_or(false);

            (for_trait::output_tokens(attr, item_trait), debug)
        }
    };

    let output = match result {
        Ok(token_stream) => token_stream,
        Err(err) => err.into_compile_error(),
    };

    if debug {
        println!("{}", output);
    }

    proc_macro::TokenStream::from(output)
}

fn output_tokens(attr: &EntraitAttr, input_fn: InputFn) -> syn::Result<proc_macro2::TokenStream> {
    let generics = analyze_generics::analyze_generics(&input_fn, attr)?;
    let entrait_sig = signature::SignatureConverter::new(attr, &input_fn, &generics.deps).convert();
    let trait_def = gen_trait_def(attr, &input_fn, &entrait_sig, &generics)?;
    let impl_blocks = gen_impl_blocks(attr, &input_fn, &entrait_sig, &generics)?;

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
        #impl_blocks
    })
}

fn gen_trait_def(
    attr: &EntraitAttr,
    input_fn: &InputFn,
    entrait_sig: &EntraitSignature,
    generics: &generics::Generics,
) -> syn::Result<proc_macro2::TokenStream> {
    let span = attr.trait_ident.span();

    let opt_unimock_attr = attr.opt_unimock_attribute(entrait_sig, &generics.deps);
    let opt_entrait_for_trait_attr = match &generics.deps {
        generics::Deps::Concrete(_) => {
            Some(quote! { #[::entrait::entrait(unimock = false, mockall = false)] })
        }
        _ => None
    };
    let opt_mockall_automock_attr = attr.opt_mockall_automock_attribute();
    let opt_async_trait_attr = input_fn.opt_async_trait_attribute(attr);

    let trait_visibility = &attr.trait_visibility;
    let trait_ident = &attr.trait_ident;
    let opt_associated_fut_decl = &entrait_sig.associated_fut_decl;
    let trait_fn_sig = &entrait_sig.sig;
    let where_clause = &generics.trait_generics.where_clause;
    let generics = &generics.trait_generics;

    Ok(
        quote_spanned! { span=>
            #opt_unimock_attr
            #opt_entrait_for_trait_attr
            #opt_mockall_automock_attr
            #opt_async_trait_attr
            #trait_visibility trait #trait_ident #generics #where_clause {
                #opt_associated_fut_decl
                #trait_fn_sig;
            }
        }
    )
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
fn gen_impl_blocks(
    attr: &EntraitAttr,
    input_fn: &InputFn,
    entrait_sig: &EntraitSignature,
    generics: &generics::Generics,
) -> syn::Result<proc_macro2::TokenStream> {
    let EntraitAttr { trait_ident, .. } = attr;
    let InputFn { fn_sig, .. } = input_fn;

    let span = trait_ident.span();

    let mut input_fn_ident = fn_sig.ident.clone();
    input_fn_ident.set_span(span);

    let async_trait_attribute = input_fn.opt_async_trait_attribute(attr);

    let args_gen = generics.arguments_generator();
    let mut where_clause_gen = generics.where_clause_generator();

    // Where bounds on the entire impl block,
    // TODO: Is it correct to always use `Sync` in here here?
    // It must be for Async at least?
    match &generics.deps {
        generics::Deps::Generic { trait_bounds, .. } => {
            if !trait_bounds.is_empty() {
                where_clause_gen.push_impl_predicate(parse_quote! {
                    ::entrait::Impl<EntraitT>: #(#trait_bounds)+*
                });
            }

            if input_fn.use_associated_future(attr) {
                // Deps must be 'static for zero-cost futures to work
                where_clause_gen.push_impl_predicate(parse_quote! {
                    EntraitT: Sync + 'static
                });
            } else {
                where_clause_gen.push_impl_predicate(parse_quote! {
                    EntraitT: Sync
                });
            }
        }
        generics::Deps::Concrete(_) => {
            where_clause_gen.push_impl_predicate(parse_quote! {
                EntraitT: #trait_ident #args_gen + Sync
            });
        }
        generics::Deps::NoDeps => {
            where_clause_gen.push_impl_predicate(parse_quote! {
                EntraitT: Sync
            });
        }
    };

    let associated_fut_impl = &entrait_sig.associated_fut_impl;

    Ok(match &generics.deps {
        generics::Deps::Concrete(path) => {
            // NOTE: the impl for Impl<T> is generated by invoking #[entrait] on the trait(!)

            let concrete_fn_def = gen_delegating_fn_item(
                span,
                input_fn,
                &input_fn_ident,
                entrait_sig,
                FnReceiverKind::SelfArg,
                &generics.deps,
            )?;

            let params_gen = generics.params_generator(generics::ImplementationGeneric(false));
            let where_clause = &generics.trait_generics.where_clause;

            quote_spanned! { span=>
                // Specific impl for the concrete type T:
                #async_trait_attribute
                impl #params_gen #trait_ident #args_gen for #path #where_clause {
                    #associated_fut_impl
                    #concrete_fn_def
                }
            }
        }
        _ => {
            let generic_fn_def = gen_delegating_fn_item(
                span,
                input_fn,
                &input_fn_ident,
                entrait_sig,
                match &generics.deps {
                    generics::Deps::Generic { .. } => FnReceiverKind::SelfArg,
                    generics::Deps::Concrete(_) => FnReceiverKind::SelfAsRefReceiver,
                    generics::Deps::NoDeps => FnReceiverKind::RefSelfArg,
                },
                &generics.deps,
            )?;
            let params_gen = generics.params_generator(generics::ImplementationGeneric(true));

            quote_spanned! { span=>
                #async_trait_attribute
                impl #params_gen #trait_ident #args_gen for ::entrait::Impl<EntraitT> #where_clause_gen {
                    #associated_fut_impl
                    #generic_fn_def
                }
            }
        }
    })
}

fn gen_delegating_fn_item(
    span: Span,
    input_fn: &InputFn,
    fn_ident: &syn::Ident,
    entrait_sig: &EntraitSignature,
    receiver_kind: FnReceiverKind,
    deps: &generics::Deps,
) -> syn::Result<proc_macro2::TokenStream> {
    let mut opt_dot_await = input_fn.opt_dot_await(span);
    let trait_fn_sig = &entrait_sig.sig;

    let arguments = entrait_sig.sig.inputs.iter().filter_map(|arg| match arg {
        syn::FnArg::Receiver(_) => match deps {
            generics::Deps::NoDeps => None,
            _ => match receiver_kind {
                FnReceiverKind::SelfArg => Some(quote_spanned! { span=> self }),
                FnReceiverKind::RefSelfArg => Some(quote_spanned! { span=> &self }),
                FnReceiverKind::SelfAsRefReceiver => None,
            },
        },
        syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
            syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_token_stream()),
            _ => panic!("Found a non-ident pattern, this should be handled in signature.rs"),
        },
    });

    let function_call = match receiver_kind {
        FnReceiverKind::SelfAsRefReceiver => quote_spanned! { span=>
            self.as_ref().#fn_ident(#(#arguments),*)
        },
        _ => quote_spanned! { span=>
            #fn_ident(#(#arguments),*)
        },
    };

    if entrait_sig.associated_fut_decl.is_some() {
        opt_dot_await = None;
    }

    Ok(quote_spanned! { span=>
        #trait_fn_sig {
            #function_call #opt_dot_await
        }
    })
}

impl EntraitAttr {
    pub fn opt_unimock_attribute(
        &self,
        entrait_sig: &EntraitSignature,
        deps: &generics::Deps,
    ) -> Option<proc_macro2::TokenStream> {
        match self.default_option(self.opts.unimock, false) {
            SpanOpt(true, span) => {
                let fn_ident = &entrait_sig.sig.ident;

                let unmocked = match deps {
                    generics::Deps::Generic { .. } => quote! { #fn_ident },
                    generics::Deps::Concrete(_) => quote! { _ },
                    generics::Deps::NoDeps => {
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

    pub fn opt_mockall_automock_attribute(&self) -> Option<proc_macro2::TokenStream> {
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

enum FnReceiverKind {
    /// f(self, ..)
    SelfArg,
    /// f(&self, ..)
    RefSelfArg,
    /// self.as_ref().f(..)
    SelfAsRefReceiver,
}

impl InputFn {
    fn opt_dot_await(&self, span: Span) -> Option<proc_macro2::TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote_spanned! { span=> .await })
        } else {
            None
        }
    }

    pub fn use_associated_future(&self, attr: &EntraitAttr) -> bool {
        matches!(
            (attr.async_strategy(), self.fn_sig.asyncness),
            (SpanOpt(AsyncStrategy::AssociatedFuture, _), Some(_async))
        )
    }

    fn opt_async_trait_attribute(&self, attr: &EntraitAttr) -> Option<proc_macro2::TokenStream> {
        match (attr.async_strategy(), self.fn_sig.asyncness) {
            (SpanOpt(AsyncStrategy::AsyncTrait, span), Some(_async)) => {
                Some(quote_spanned! { span=> #[::entrait::__async_trait::async_trait] })
            }
            _ => None,
        }
    }
}
