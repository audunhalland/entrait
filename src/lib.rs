//! # entrait
//!
//! Experimental proc macro to ease development using _Inversion of Control_ patterns in Rust.
//!
//! `entrait` is used to generate a trait from the definition of a regular function.
//! The main use case for this is that other functions may depend upon the trait
//! instead of the concrete implementation, enabling better test isolation.
//!
//! The macro looks like this:
//!
//! ```rust
//! # use entrait::*;
//! #[entrait(MyFunction)]
//! fn my_function<A>(a: &A) {
//! }
//! ```
//!
//! which generates the trait `MyFunction`:
//!
//! ```rust
//! trait MyFunction {
//!     fn my_function(&self);
//! }
//! ```
//!
//! `my_function`'s first and only parameter is `a` which is generic over some unknown type `A`. This would correspond to the `self` parameter in the trait. But what is this type supposed to be? We can generate an implementation in the same go, using `for Type`:
//!
//! ```rust
//! struct App;
//!
//! #[entrait::entrait(MyFunction for App)]
//! fn my_function<A>(app: &A) {
//! }
//!
//! // Generated:
//! // trait MyFunction {
//! //     fn my_function(&self);
//! // }
//! //
//! // impl MyFunction for App {
//! //     fn my_function(&self) {
//! //         my_function(self)
//! //     }
//! // }
//!
//! fn main() {
//!     let app = App;
//!     app.my_function();
//! }
//! ```
//!
//! The advantage of this pattern comes into play when a function declares its dependencies, as _trait bounds_:
//!
//!
//! ```rust
//! # use entrait::*;
//! # struct App;
//! #[entrait(Foo for App)]
//! fn foo<A>(a: &A)
//! where
//!     A: Bar
//! {
//!     a.bar();
//! }
//!
//! #[entrait(Bar for App)]
//! fn bar<A>(a: &A) {
//! }
//! ```
//!
//! The functions may take any number of parameters, but the first one is always considered specially as the "dependency parameter".
//!
//! Functions may also be non-generic, depending directly on the App:
//!
//! ```rust
//! # use entrait::*;
//! # struct App { some_thing: SomeType };
//! # type SomeType = u32;
//! #[entrait(ExtractSomething for App)]
//! fn extract_something(app: &App) -> SomeType {
//!     app.some_thing
//! }
//! ```
//!
//! These kinds of functions may be considered "leaves" of a dependency tree.

#![forbid(unsafe_code)]

use quote::{quote, spanned::Spanned};
use syn::parse::{Parse, ParseStream};

extern crate proc_macro;

use proc_macro::TokenStream;

///
/// Generate a trait definition from a regular function.
///
#[proc_macro_attribute]
pub fn entrait(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attrs = syn::parse_macro_input!(attr as EntraitAttrs);
    let body = syn::parse_macro_input!(input as EntraitBody);

    let input_fn = &body.input_fn;
    let trait_def = gen_trait_def(&attrs, &body);
    let impl_block = gen_impl_block(&attrs, &body);

    let output = quote! {
        #input_fn
        #trait_def
        #impl_block
    };

    TokenStream::from(output)
}

struct EntraitAttrs {
    trait_ident: syn::Ident,
    impl_target_type: Option<syn::Type>,
}

struct EntraitBody {
    input_fn: syn::ItemFn,
    trait_fn_inputs: proc_macro2::TokenStream,
    call_param_list: proc_macro2::TokenStream,
}

impl EntraitBody {
    fn opt_async(&self) -> Option<proc_macro2::TokenStream> {
        if self.input_fn.sig.asyncness.is_some() {
            Some(quote! { async })
        } else {
            None
        }
    }

    fn opt_dot_await(&self) -> Option<proc_macro2::TokenStream> {
        if self.input_fn.sig.asyncness.is_some() {
            Some(quote! { .await })
        } else {
            None
        }
    }

    fn opt_async_trait_attribute(&self) -> Option<proc_macro2::TokenStream> {
        if cfg!(feature = "async_trait") && self.input_fn.sig.asyncness.is_some() {
            Some(quote! { #[async_trait::async_trait] })
        } else {
            None
        }
    }
}

fn gen_trait_def(
    EntraitAttrs { trait_ident, .. }: &EntraitAttrs,
    body: &EntraitBody,
) -> proc_macro2::TokenStream {
    let EntraitBody {
        input_fn,
        trait_fn_inputs,
        ..
    } = body;
    let input_fn_ident = &input_fn.sig.ident;
    let fn_output = &input_fn.sig.output;

    let opt_async_trait_attr = body.opt_async_trait_attribute();
    let opt_async = body.opt_async();

    return quote! {
        #opt_async_trait_attr
        pub trait #trait_ident {
            #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output;
        }
    };
}

fn gen_impl_block(
    EntraitAttrs {
        trait_ident,
        impl_target_type,
    }: &EntraitAttrs,
    body: &EntraitBody,
) -> Option<proc_macro2::TokenStream> {
    let EntraitBody {
        input_fn,
        trait_fn_inputs,
        call_param_list,
    } = body;
    let input_fn_ident = &input_fn.sig.ident;
    let fn_output = &input_fn.sig.output;

    let async_trait_attribute = body.opt_async_trait_attribute();
    let opt_async = body.opt_async();
    let opt_dot_await = body.opt_dot_await();

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

impl Parse for EntraitAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let trait_ident = input.parse()?;

        let impl_target_type = if input.peek(syn::token::For) {
            input.parse::<syn::token::For>()?;
            Some(input.parse()?)
        } else {
            None
        };

        Ok(EntraitAttrs {
            trait_ident,
            impl_target_type,
        })
    }
}

impl Parse for EntraitBody {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let input_fn = input.parse()?;

        let trait_fn_inputs = extract_trait_fn_inputs(&input_fn)?;
        let call_param_list = extract_call_param_list(&input_fn)?;

        Ok(EntraitBody {
            input_fn,
            trait_fn_inputs,
            call_param_list,
        })
    }
}

fn extract_trait_fn_inputs(input_fn: &syn::ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    let sig = &input_fn.sig;
    let mut inputs = sig.inputs.clone();

    if inputs.is_empty() {
        return Err(syn::Error::new(
            input_fn.sig.__span(),
            "Function must take at least one parameter",
        ));
    }

    let first_mut = inputs.first_mut().unwrap();
    *first_mut = syn::parse_quote! { &self };

    Ok(quote! {
        #inputs
    })
}

fn extract_call_param_list(input_fn: &syn::ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    let params = input_fn
        .sig
        .inputs
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            if index == 0 {
                Ok(quote! { self })
            } else {
                match arg {
                    syn::FnArg::Receiver(_) => {
                        Err(syn::Error::new(arg.__span(), "Unexpected receiver arg"))
                    }
                    syn::FnArg::Typed(pat_typed) => match pat_typed.pat.as_ref() {
                        syn::Pat::Ident(pat_ident) => {
                            let ident = &pat_ident.ident;
                            Ok(quote! { #ident })
                        }
                        _ => Err(syn::Error::new(
                            arg.__span(),
                            "Expected ident for function argument",
                        )),
                    },
                }
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(quote! {
        #(#params),*
    })
}
