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
    let func = syn::parse_macro_input!(input as EntraitInputFn);

    let trait_def = gen_trait_def(&attr, &func);
    let impl_block = gen_impl_block(&attr, &func);

    let EntraitInputFn {
        fn_attrs,
        fn_vis,
        fn_sig,
        fn_body,
        ..
    } = func;

    let output = quote! {
        #(#fn_attrs)* #fn_vis #fn_sig #fn_body
        #trait_def
        #impl_block
    };

    if attr.debug.is_some() {
        println!("{}", output);
    }

    proc_macro::TokenStream::from(output)
}

fn gen_trait_def(attr: &EntraitAttr, func: &EntraitInputFn) -> proc_macro2::TokenStream {
    let span = attr.trait_ident.span();
    let trait_def = gen_trait_def_no_mock(attr, func);

    match (
        attr.opt_unimock_attribute(),
        attr.opt_mockall_automock_attribute(),
    ) {
        (None, None) => trait_def,
        (unimock, automock) => quote_spanned! { span=>
            #unimock
            #automock
            #trait_def
        },
    }
}

fn gen_trait_def_no_mock(attr: &EntraitAttr, func: &EntraitInputFn) -> proc_macro2::TokenStream {
    let EntraitInputFn {
        fn_sig,
        trait_fn_inputs,
        ..
    } = func;
    let trait_ident = &attr.trait_ident;
    let span = trait_ident.span();
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

    let opt_async_trait_attr = func.opt_async_trait_attribute(attr);
    let opt_async = func.opt_async(span);

    quote_spanned! { span=>
        #opt_async_trait_attr
        pub trait #trait_ident {
            #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output;
        }
    }
}

fn gen_impl_block(attr: &EntraitAttr, func: &EntraitInputFn) -> Option<proc_macro2::TokenStream> {
    let EntraitAttr {
        trait_ident,
        impl_target_type,
        ..
    } = attr;
    let EntraitInputFn {
        fn_sig,
        trait_fn_inputs,
        call_param_list,
        ..
    } = func;

    impl_target_type.as_ref().map(|impl_target_type| {
        let span = impl_target_type.span();

        let mut input_fn_ident = fn_sig.ident.clone();
        let fn_output = &fn_sig.output;

        input_fn_ident.set_span(span);

        let async_trait_attribute = func.opt_async_trait_attribute(attr);
        let opt_dot_await = func.opt_dot_await(span);
        let opt_async = func.opt_async(span);

        quote_spanned! { span=>
            #async_trait_attribute
            impl #trait_ident for #impl_target_type {
                #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output {
                    #input_fn_ident(#call_param_list) #opt_dot_await
                }
            }
        }
    })
}

impl EntraitAttr {
    pub fn opt_unimock_attribute(&self) -> Option<proc_macro2::TokenStream> {
        match self.unimock {
            Some((input::EnabledValue::Always, span)) => {
                Some(quote_spanned! { span=> #[::unimock::unimock] })
            }
            Some((input::EnabledValue::TestOnly, span)) => {
                Some(quote_spanned! { span=> #[cfg_attr(test, ::unimock::unimock)] })
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

impl EntraitInputFn {
    pub fn opt_async(&self, span: Span) -> Option<proc_macro2::TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote_spanned! { span=> async })
        } else {
            None
        }
    }

    pub fn opt_dot_await(&self, span: Span) -> Option<proc_macro2::TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote_spanned! { span=> .await })
        } else {
            None
        }
    }

    pub fn opt_async_trait_attribute(
        &self,
        attr: &EntraitAttr,
    ) -> Option<proc_macro2::TokenStream> {
        match (attr.async_trait, self.fn_sig.asyncness.is_some()) {
            (Some(span), true) => Some(quote_spanned! { span=> #[::async_trait::async_trait] }),
            _ => None,
        }
    }
}
