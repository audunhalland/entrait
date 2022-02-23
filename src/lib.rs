#![forbid(unsafe_code)]

use quote::{quote, spanned::Spanned};
use syn::parse::{Parse, ParseStream};

extern crate proc_macro;

use proc_macro::TokenStream;

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
    impl_ident: Option<syn::Ident>,
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

fn gen_trait_def(EntraitAttrs { trait_ident, .. }: &EntraitAttrs, body: &EntraitBody) -> proc_macro2::TokenStream {
    let EntraitBody { input_fn, trait_fn_inputs, .. } = body;
    let input_fn_ident = &input_fn.sig.ident;
    let fn_output = &input_fn.sig.output;

    let opt_async_trait_attr = body.opt_async_trait_attribute();
    let opt_async = body.opt_async();

    return quote! {
        #opt_async_trait_attr
        pub trait #trait_ident {
            #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output;
        }
    }
}

fn gen_impl_block(EntraitAttrs { trait_ident, impl_ident }: &EntraitAttrs, body: &EntraitBody) -> Option<proc_macro2::TokenStream> {
    let EntraitBody { input_fn, trait_fn_inputs, call_param_list } = body;
    let input_fn_ident = &input_fn.sig.ident;
    let fn_output = &input_fn.sig.output;

    let async_trait_attribute = body.opt_async_trait_attribute();
    let opt_async = body.opt_async();
    let opt_dot_await = body.opt_dot_await();

    impl_ident.as_ref().map(|impl_ident|
        quote! {
            #async_trait_attribute
            impl #trait_ident for #impl_ident {
                #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output {
                    #input_fn_ident(#call_param_list) #opt_dot_await
                }
            }
        }
    )
}

impl Parse for EntraitAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let trait_ident = input.parse()?;

        let impl_ident = if input.peek(syn::token::For) {
            input.parse::<syn::token::For>()?;
            Some(input.parse()?)
        } else { None };

        Ok(EntraitAttrs {
            trait_ident,
            impl_ident,
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
