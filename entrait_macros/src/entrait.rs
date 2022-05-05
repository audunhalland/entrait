//! # entrait_macros
//!
//! Procedural macros used by entrait.

use proc_macro2::Span;
use quote::quote;
use quote::quote_spanned;
use syn::spanned::Spanned;

use crate::deps;
use crate::input::*;

pub fn invoke(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
    attr_modifier: impl FnOnce(&mut EntraitAttr),
) -> proc_macro::TokenStream {
    let mut attr = syn::parse_macro_input!(attr as EntraitAttr);
    let input_fn = syn::parse_macro_input!(input as InputFn);

    attr_modifier(&mut attr);

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
    let deps = deps::analyze_deps(&input_fn)?;
    let trait_def = gen_trait_def(attr, &input_fn, &deps)?;
    let implementation_impl_block = gen_implementation_impl_block(attr, &input_fn, &deps)?;

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
        #implementation_impl_block
    })
}

fn gen_trait_def(
    attr: &EntraitAttr,
    input_fn: &InputFn,
    deps: &deps::Deps,
) -> syn::Result<proc_macro2::TokenStream> {
    let span = attr.trait_ident.span();
    let trait_def = gen_trait_def_no_mock(attr, input_fn)?;

    Ok(
        match (
            attr.opt_unimock_attribute(input_fn, deps),
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
    let trait_visibility = &attr.trait_visibility;
    let trait_ident = &attr.trait_ident;
    let span = trait_ident.span();
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

    let opt_async_trait_attr = input_fn.opt_async_trait_attribute(attr);
    let opt_async = input_fn.opt_async(span);
    let trait_fn_inputs = input_fn.trait_fn_inputs(span);

    Ok(quote_spanned! { span=>
        #opt_async_trait_attr
        #trait_visibility trait #trait_ident {
            #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output;
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
fn gen_implementation_impl_block(
    attr: &EntraitAttr,
    input_fn: &InputFn,
    deps: &deps::Deps,
) -> syn::Result<proc_macro2::TokenStream> {
    let EntraitAttr { trait_ident, .. } = attr;
    let InputFn { fn_sig, .. } = input_fn;

    let span = trait_ident.span();

    let mut input_fn_ident = fn_sig.ident.clone();
    input_fn_ident.set_span(span);

    // TODO: set span for output
    let fn_output = &fn_sig.output;

    let async_trait_attribute = input_fn.opt_async_trait_attribute(attr);
    let opt_dot_await = input_fn.opt_dot_await(span);
    let opt_async = input_fn.opt_async(span);
    let trait_fn_inputs = input_fn.trait_fn_inputs(span);

    let output = match deps {
        deps::Deps::GenericOrAbsent { trait_bounds } => {
            let call_param_list = input_fn.call_param_list(span, SelfImplParam::OuterImplT)?;

            let impl_trait_bounds = if trait_bounds.is_empty() {
                None
            } else {
                Some(quote! {
                    ::implementation::Impl<EntraitT>: #(#trait_bounds)+*,
                })
            };

            quote_spanned! { span=>
                #async_trait_attribute
                impl<EntraitT> #trait_ident for ::implementation::Impl<EntraitT>
                    // TODO: Is it correct to always use Sync here?
                    // It must be for Async at least?
                    where #impl_trait_bounds EntraitT: Sync
                {
                    #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output {
                        #input_fn_ident(#call_param_list) #opt_dot_await
                    }
                }
            }
        }
        deps::Deps::Concrete(path) => {
            let call_param_list = input_fn.call_param_list(span, SelfImplParam::InnerRefT)?;

            quote_spanned! { span=>
                #async_trait_attribute
                impl #trait_ident for ::implementation::Impl<#path> {
                    #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output {
                        #input_fn_ident(#call_param_list) #opt_dot_await
                    }
                }
            }
        }
    };

    Ok(output)
}

impl EntraitAttr {
    pub fn opt_unimock_attribute(
        &self,
        input_fn: &InputFn,
        deps: &deps::Deps,
    ) -> Option<proc_macro2::TokenStream> {
        match self.unimock {
            Some((ref enabled_value, span)) => {
                let fn_ident = &input_fn.fn_sig.ident;

                let unmocked = match deps {
                    deps::Deps::GenericOrAbsent { trait_bounds: _ } => quote! { #fn_ident },
                    deps::Deps::Concrete(_) => quote! { _ },
                };

                let unimock_attr = quote_spanned! {span=>
                    ::unimock::unimock(mod=#fn_ident, as=[Fn], unmocked=[#unmocked])
                };
                Some(match enabled_value {
                    EnabledValue::Always => quote_spanned! {span=>
                        #[#unimock_attr]
                    },
                    EnabledValue::TestOnly => quote_spanned! {span=>
                        #[cfg_attr(test, #unimock_attr)]
                    },
                })
            }
            None => None,
        }
    }

    pub fn opt_mockall_automock_attribute(&self) -> Option<proc_macro2::TokenStream> {
        match self.mockall {
            Some((EnabledValue::Always, span)) => {
                Some(quote_spanned! { span=> #[::mockall::automock] })
            }
            Some((EnabledValue::TestOnly, span)) => {
                Some(quote_spanned! { span=> #[cfg_attr(test, ::mockall::automock)] })
            }
            None => None,
        }
    }
}

enum SelfImplParam {
    OuterImplT,
    InnerRefT,
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

    fn trait_fn_inputs(&self, span: Span) -> proc_macro2::TokenStream {
        let mut inputs = self.fn_sig.inputs.clone();

        if inputs.is_empty() {
            return quote! {};
        }

        let first_mut = inputs.first_mut().unwrap();
        *first_mut = syn::parse_quote_spanned! { span=> &self };

        quote! {
            #inputs
        }
    }

    fn call_param_list(
        &self,
        span: Span,
        self_impl_param: SelfImplParam,
    ) -> syn::Result<proc_macro2::TokenStream> {
        let params = self
            .fn_sig
            .inputs
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                if index == 0 {
                    Ok(match self_impl_param {
                        SelfImplParam::OuterImplT => quote_spanned! { span=> self },
                        SelfImplParam::InnerRefT => quote_spanned! { span=> &self },
                    })
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
