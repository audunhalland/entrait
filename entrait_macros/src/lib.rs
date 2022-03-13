//! # entrait_macros
//!
//! Procedural macros used by entrait.

#![forbid(unsafe_code)]

use quote::quote;

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
    let func = syn::parse_macro_input!(input as EntraitFn);

    let trait_def = gen_trait_def(&attr, &func);
    let impl_block = gen_impl_block(&attr, &func);

    let EntraitFn {
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

    if attr.debug {
        println!("{}", output);
    }

    proc_macro::TokenStream::from(output)
}

fn gen_trait_def(attr: &EntraitAttr, func: &EntraitFn) -> proc_macro2::TokenStream {
    let trait_def = gen_trait_def_no_mock(attr, func);

    match (
        attr.opt_unimock_attribute(),
        attr.opt_mockall_automock_attribute(),
    ) {
        (None, None) => trait_def,
        (unimock, automock) => quote! {
            #unimock
            #automock
            #trait_def
        },
    }
}

fn gen_trait_def_no_mock(attr: &EntraitAttr, func: &EntraitFn) -> proc_macro2::TokenStream {
    let EntraitFn {
        fn_sig,
        trait_fn_inputs,
        ..
    } = func;
    let trait_ident = &attr.trait_ident;
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

    let opt_async_trait_attr = func.opt_async_trait_attribute(attr);
    let opt_async = func.opt_async();

    quote! {
        #opt_async_trait_attr
        pub trait #trait_ident {
            #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output;
        }
    }
}

fn gen_impl_block(attr: &EntraitAttr, func: &EntraitFn) -> Option<proc_macro2::TokenStream> {
    let EntraitAttr {
        trait_ident,
        impl_target_type,
        ..
    } = attr;
    let EntraitFn {
        fn_sig,
        trait_fn_inputs,
        call_param_list,
        ..
    } = func;
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

    let async_trait_attribute = func.opt_async_trait_attribute(attr);
    let opt_async = func.opt_async();
    let opt_dot_await = func.opt_dot_await();

    impl_target_type.as_ref().map(|impl_target_type| {
        quote! {
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
            input::EnabledValue::Always => Some(quote! { #[::unimock::unimock] }),
            input::EnabledValue::TestOnly => Some(quote! { #[cfg_attr(test, ::unimock::unimock)] }),
            input::EnabledValue::Never => None,
        }
    }

    pub fn opt_mockall_automock_attribute(&self) -> Option<proc_macro2::TokenStream> {
        match self.mockall {
            input::EnabledValue::Always => Some(quote! { #[::mockall::automock] }),
            input::EnabledValue::TestOnly => {
                Some(quote! { #[cfg_attr(test, ::mockall::automock)] })
            }
            input::EnabledValue::Never => None,
        }
    }
}

impl EntraitFn {
    pub fn opt_async(&self) -> Option<proc_macro2::TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote! { async })
        } else {
            None
        }
    }

    pub fn opt_dot_await(&self) -> Option<proc_macro2::TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote! { .await })
        } else {
            None
        }
    }

    pub fn opt_async_trait_attribute(
        &self,
        attr: &EntraitAttr,
    ) -> Option<proc_macro2::TokenStream> {
        if attr.async_trait && self.fn_sig.asyncness.is_some() {
            Some(quote! { #[::async_trait::async_trait] })
        } else {
            None
        }
    }
}
