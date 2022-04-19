//! # entrait_macros
//!
//! Procedural macros used by entrait.

#![forbid(unsafe_code)]

use proc_macro2::Span;
use quote::quote;
use quote::quote_spanned;
use syn::spanned::Spanned;

extern crate proc_macro;

mod input;

use input::*;

///
/// Generate a trait definition from a regular function.
///
#[proc_macro_attribute]
pub fn entrait(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attr = syn::parse_macro_input!(attr as EntraitAttr);
    let input_fn = syn::parse_macro_input!(input as InputFn);

    let output = match output_tokens(&attr, input_fn) {
        Ok(token_stream) => token_stream,
        Err(err) => err.into_compile_error(),
    };

    if attr.debug.is_some() {
        println!("{}", output);
    }

    proc_macro::TokenStream::from(output)
}

fn output_tokens(attr: &EntraitAttr, input_fn: InputFn) -> syn::Result<proc_macro2::TokenStream> {
    let trait_def = gen_trait_def(attr, &input_fn)?;
    let impl_block = match attr.impl_target_type.as_ref() {
        Some(impl_target_type) => Some(gen_impl_block(impl_target_type, attr, &input_fn)?),
        None => None,
    };

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

fn gen_trait_def(attr: &EntraitAttr, input_fn: &InputFn) -> syn::Result<proc_macro2::TokenStream> {
    let span = attr.trait_ident.span();
    let trait_def = gen_trait_def_no_mock(attr, input_fn)?;

    Ok(
        match (
            attr.opt_unimock_attribute(input_fn),
            attr.opt_mockall_automock_attribute(),
        ) {
            (None, None) => trait_def,
            (unimock, automock) => quote_spanned! { span=>
                #unimock
                #automock
                #trait_def
            },
        },
    )
}

fn gen_trait_def_no_mock(
    attr: &EntraitAttr,
    input_fn: &InputFn,
) -> syn::Result<proc_macro2::TokenStream> {
    let InputFn { fn_sig, .. } = input_fn;
    let trait_ident = &attr.trait_ident;
    let span = trait_ident.span();
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

    let opt_async_trait_attr = input_fn.opt_async_trait_attribute(attr);
    let opt_async = input_fn.opt_async(span);
    let trait_fn_inputs = input_fn.trait_fn_inputs(span)?;

    Ok(quote_spanned! { span=>
        #opt_async_trait_attr
        pub trait #trait_ident {
            #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output;
        }
    })
}

fn gen_impl_block(
    impl_target_type: &syn::Type,
    attr: &EntraitAttr,
    input_fn: &InputFn,
) -> syn::Result<proc_macro2::TokenStream> {
    let EntraitAttr { trait_ident, .. } = attr;
    let InputFn { fn_sig, .. } = input_fn;

    let span = impl_target_type.span();

    let mut input_fn_ident = fn_sig.ident.clone();
    input_fn_ident.set_span(span);

    // TODO: set span for output
    let fn_output = &fn_sig.output;

    let async_trait_attribute = input_fn.opt_async_trait_attribute(attr);
    let opt_dot_await = input_fn.opt_dot_await(span);
    let opt_async = input_fn.opt_async(span);
    let trait_fn_inputs = input_fn.trait_fn_inputs(span)?;
    let call_param_list = input_fn.call_param_list(span)?;

    Ok(quote_spanned! { span=>
        #async_trait_attribute
        impl #trait_ident for #impl_target_type {
            #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output {
                #input_fn_ident(#call_param_list) #opt_dot_await
            }
        }
    })
}

impl EntraitAttr {
    pub fn opt_unimock_attribute(&self, input_fn: &InputFn) -> Option<proc_macro2::TokenStream> {
        match self.unimock {
            Some((ref enabled_value, span)) => {
                let fn_ident = &input_fn.fn_sig.ident;

                let unmocked = if self.disable_unmock.is_some() {
                    quote! { _ }
                } else {
                    quote! { #fn_ident }
                };

                let unimock_attr = quote_spanned! {span=>
                    ::unimock::unimock(mod=#fn_ident, as=[Fn], unmocked=[#unmocked])
                };
                Some(match enabled_value {
                    input::EnabledValue::Always => quote_spanned! {span=>
                        #[#unimock_attr]
                    },
                    input::EnabledValue::TestOnly => quote_spanned! {span=>
                        #[cfg_attr(test, #unimock_attr)]
                    },
                })
            }
            None => None,
        }
    }

    pub fn opt_mockall_automock_attribute(&self) -> Option<proc_macro2::TokenStream> {
        match self.mockall {
            Some((input::EnabledValue::Always, span)) => {
                Some(quote_spanned! { span=> #[::mockall::automock] })
            }
            Some((input::EnabledValue::TestOnly, span)) => {
                Some(quote_spanned! { span=> #[cfg_attr(test, ::mockall::automock)] })
            }
            None => None,
        }
    }
}

impl InputFn {
    fn opt_async(&self, span: Span) -> Option<proc_macro2::TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote_spanned! { span=> async })
        } else {
            None
        }
    }

    fn opt_dot_await(&self, span: Span) -> Option<proc_macro2::TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote_spanned! { span=> .await })
        } else {
            None
        }
    }

    fn opt_async_trait_attribute(&self, attr: &EntraitAttr) -> Option<proc_macro2::TokenStream> {
        match (attr.async_trait, self.fn_sig.asyncness.is_some()) {
            (Some(span), true) => Some(quote_spanned! { span=> #[::async_trait::async_trait] }),
            _ => None,
        }
    }

    fn trait_fn_inputs(&self, span: Span) -> syn::Result<proc_macro2::TokenStream> {
        let mut inputs = self.fn_sig.inputs.clone();

        if inputs.is_empty() {
            return Err(syn::Error::new(
                self.fn_sig.span(),
                "Function must take at least one parameter",
            ));
        }

        let first_mut = inputs.first_mut().unwrap();
        *first_mut = syn::parse_quote_spanned! { span=> &self };

        Ok(quote! {
            #inputs
        })
    }

    fn call_param_list(&self, span: Span) -> syn::Result<proc_macro2::TokenStream> {
        let params = self
            .fn_sig
            .inputs
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                if index == 0 {
                    Ok(quote_spanned! { span=> self })
                } else {
                    match arg {
                        syn::FnArg::Receiver(_) => {
                            Err(syn::Error::new(arg.span(), "Unexpected receiver arg"))
                        }
                        syn::FnArg::Typed(pat_typed) => match pat_typed.pat.as_ref() {
                            syn::Pat::Ident(pat_ident) => {
                                let ident = &pat_ident.ident;
                                Ok(quote_spanned! { span=> #ident })
                            }
                            _ => Err(syn::Error::new(
                                arg.span(),
                                "Expected ident for function argument",
                            )),
                        },
                    }
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(quote_spanned! { span=>
            #(#params),*
        })
    }
}
