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

///
/// Generate a trait definition from a regular function.
///
#[proc_macro_attribute]
pub fn entrait(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attrs = syn::parse_macro_input!(attr as EntraitAttrs);
    let func = syn::parse_macro_input!(input as EntraitFn);

    let trait_def = gen_trait_def(&attrs, &func);
    let impl_block = gen_impl_block(&attrs, &func);

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

    proc_macro::TokenStream::from(output)
}

#[proc_macro_derive(Lol)]
pub fn lol(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    input
}

struct EntraitAttrs {
    trait_ident: syn::Ident,
    impl_target_type: Option<syn::Type>,
    use_async_trait: bool,
    use_mockall: bool,
}

enum Extension {
    AsyncTrait,
    Mockall,
}

struct EntraitFn {
    fn_attrs: Vec<syn::Attribute>,
    fn_vis: syn::Visibility,
    fn_sig: syn::Signature,
    // don't try to parse fn_body, just pass through the tokens:
    fn_body: proc_macro2::TokenStream,

    trait_fn_inputs: proc_macro2::TokenStream,
    call_param_list: proc_macro2::TokenStream,
}

impl EntraitFn {
    fn opt_async(&self) -> Option<proc_macro2::TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote! { async })
        } else {
            None
        }
    }

    fn opt_dot_await(&self) -> Option<proc_macro2::TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote! { .await })
        } else {
            None
        }
    }

    fn opt_async_trait_attribute(&self, attrs: &EntraitAttrs) -> Option<proc_macro2::TokenStream> {
        if attrs.use_async_trait && self.fn_sig.asyncness.is_some() {
            Some(quote! { #[async_trait::async_trait] })
        } else {
            None
        }
    }
}

fn gen_trait_def(attrs: &EntraitAttrs, body: &EntraitFn) -> proc_macro2::TokenStream {
    let EntraitFn {
        fn_sig,
        trait_fn_inputs,
        ..
    } = body;
    let trait_ident = &attrs.trait_ident;
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

    let opt_async_trait_attr = body.opt_async_trait_attribute(attrs);
    let opt_async = body.opt_async();

    return quote! {
        #opt_async_trait_attr
        pub trait #trait_ident {
            #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output;
        }
    };
}

fn gen_impl_block(attrs: &EntraitAttrs, body: &EntraitFn) -> Option<proc_macro2::TokenStream> {
    let impl_target_type = attrs.impl_target_type.as_ref()?;
    let trait_ident = &attrs.trait_ident;

    let EntraitFn {
        fn_sig,
        trait_fn_inputs,
        call_param_list,
        ..
    } = body;
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

    let async_trait_attribute = body.opt_async_trait_attribute(attrs);
    let opt_async = body.opt_async();
    let opt_dot_await = body.opt_dot_await();

    Some(quote! {
        #async_trait_attribute
        impl #trait_ident for #impl_target_type {
            #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output {
                #input_fn_ident(#call_param_list) #opt_dot_await
            }
        }
    })
}

impl Parse for EntraitAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut use_async_trait = false;
        let mut use_mockall = false;

        input.peek(syn::token::Macro);

        while input.peek(syn::token::Pound) {
            input.parse::<syn::token::Pound>()?;

            println!("parsed pound {input}");

            let custom_attr;
            syn::bracketed!(custom_attr in input);
            let extension: Extension = custom_attr.parse()?;

            match extension {
                Extension::AsyncTrait => use_async_trait = true,
                Extension::Mockall => use_mockall = true,
            };
        }

        let trait_ident = input.parse()?;

        let impl_target_type = if input.peek(syn::token::For) {
            input.parse::<syn::token::For>()?;
            Some(input.parse()?)
        } else {
            None
        };

        if input.peek(syn::token::Comma) {
            input.parse::<syn::token::Comma>()?;
            input.parse::<syn::token::Use>()?;

            loop {
                let extension: Extension = input.parse()?;

                match extension {
                    Extension::AsyncTrait => use_async_trait = true,
                    Extension::Mockall => use_mockall = true,
                };

                if input.peek(syn::token::Add) {
                    input.parse::<syn::token::Add>()?;
                } else {
                    break;
                }
            }
        }

        println!("use_async_trait: {use_async_trait}");

        Ok(EntraitAttrs {
            trait_ident,
            impl_target_type,
            use_async_trait,
            use_mockall,
        })
    }
}

impl Parse for Extension {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        let span = ident.span();
        let ident_string = ident.to_string();

        match ident_string.as_str() {
            "async_trait" => Ok(Extension::AsyncTrait),
            "mockall" => Ok(Extension::Mockall),
            _ => Err(syn::Error::new(
                span,
                format!("Unkonwn entrait extension \"{ident_string}\""),
            )),
        }
    }
}

impl Parse for EntraitFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fn_attrs = input.call(syn::Attribute::parse_outer)?;
        let fn_vis = input.parse()?;
        let fn_sig = input.parse()?;
        let fn_body = input.parse()?;

        let trait_fn_inputs = extract_trait_fn_inputs(&fn_sig)?;
        let call_param_list = extract_call_param_list(&fn_sig)?;

        Ok(EntraitFn {
            fn_attrs,
            fn_vis,
            fn_sig,
            fn_body,
            trait_fn_inputs,
            call_param_list,
        })
    }
}

fn extract_trait_fn_inputs(sig: &syn::Signature) -> syn::Result<proc_macro2::TokenStream> {
    let mut inputs = sig.inputs.clone();

    if inputs.is_empty() {
        return Err(syn::Error::new(
            sig.__span(),
            "Function must take at least one parameter",
        ));
    }

    let first_mut = inputs.first_mut().unwrap();
    *first_mut = syn::parse_quote! { &self };

    Ok(quote! {
        #inputs
    })
}

fn extract_call_param_list(sig: &syn::Signature) -> syn::Result<proc_macro2::TokenStream> {
    let params = sig
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
