#![forbid(unsafe_code)]

use quote::{quote, spanned::Spanned};
use syn::parse::{Parse, ParseStream};

extern crate proc_macro;

use proc_macro::TokenStream;

struct EntraitAttrs {
    trait_ident: syn::Ident,
    impl_ident: Option<syn::Ident>,
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

struct EntraitBody {
    input_fn: syn::ItemFn,
    trait_fn_inputs: proc_macro2::TokenStream,
    call_param_list: proc_macro2::TokenStream,
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

#[proc_macro_attribute]
pub fn entrait(attr: TokenStream, input: TokenStream) -> TokenStream {
    let EntraitAttrs {
        trait_ident,
        impl_ident,
    } = syn::parse_macro_input!(attr as EntraitAttrs);
    let EntraitBody {
        input_fn,
        trait_fn_inputs: trait_fn_signature,
        call_param_list,
    } = syn::parse_macro_input!(input as EntraitBody);

    let input_fn_ident = &input_fn.sig.ident;
    let fn_output = &input_fn.sig.output;

    let impl_block = impl_ident.map(|impl_ident|
        quote! {
            impl #trait_ident for #impl_ident {
                fn #input_fn_ident(#trait_fn_signature) #fn_output {
                    #input_fn_ident(#call_param_list)
                }
            }
        }
    );

    let output = quote! {
        #input_fn
        pub trait #trait_ident {
            fn #input_fn_ident(#trait_fn_signature) #fn_output;
        }
        #impl_block
    };

    // println!("{}", output);

    TokenStream::from(output)
}
