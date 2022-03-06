//! # entrait_macros
//!
//! Procedural macros used by entrait.

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

    let fn_sig_macro = gen_fn_sig_macro(&func);
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
        #fn_sig_macro
        #trait_def
        #impl_block
    };

    //println!("{}", output);

    proc_macro::TokenStream::from(output)
}

#[proc_macro]
pub fn generate_mock(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mock_input = syn::parse_macro_input!(input as EntraitMockInput);

    let mock_ident = mock_input.mock_ident;

    let impls = mock_input.trait_items.into_iter().map(|trait_item| {
        let trait_ident = trait_item.ident;

        let items = trait_item.items.into_iter().map(|item| match item {
            syn::TraitItem::Method(mut trait_item_method) => {
                trait_item_method.sig.inputs = trait_item_method
                    .sig
                    .inputs
                    .into_iter()
                    .map(|fn_arg| match fn_arg {
                        syn::FnArg::Receiver(receiver) => syn::FnArg::Receiver(syn::Receiver {
                            attrs: receiver.attrs,
                            reference: receiver.reference,
                            mutability: receiver.mutability,
                            // All this for fixing this hygiene issue:
                            self_token: syn::parse_quote! { self },
                        }),
                        _ => fn_arg,
                    })
                    .collect();

                quote! { #trait_item_method }
            }
            _ => quote! {},
        });

        quote! {
            impl #trait_ident for #mock_ident {
                #(#items);*
            }
        }
    });

    let output = quote! {
        mockall::mock! {
            #mock_ident {}
            #(#impls)*
        }
    };

    proc_macro::TokenStream::from(output)
}

struct EntraitAttrs {
    trait_ident: syn::Ident,
    impl_target_type: Option<syn::Type>,
}

struct EntraitFn {
    fn_attrs: Vec<syn::Attribute>,
    fn_vis: syn::Visibility,
    fn_sig: syn::Signature,
    // don't try to parse fn_body, just pass through the tokens:
    fn_body: proc_macro2::TokenStream,

    sig_macro_ident: syn::Ident,
    trait_fn_inputs: proc_macro2::TokenStream,
    call_param_list: proc_macro2::TokenStream,
}

struct EntraitMockInput {
    mock_ident: syn::Ident,
    trait_items: Vec<syn::ItemTrait>,
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

    // FIXME: Must have explicit parameter to generate this:
    fn opt_async_trait_attribute(&self) -> Option<proc_macro2::TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote! { #[async_trait::async_trait] })
        } else {
            None
        }
    }
}

fn gen_fn_sig_macro(body: &EntraitFn) -> proc_macro2::TokenStream {
    let EntraitFn {
        fn_sig,
        trait_fn_inputs,
        sig_macro_ident,
        ..
    } = body;
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

    let opt_async = body.opt_async();

    return quote! {
        macro_rules! #sig_macro_ident {
            () => {
                #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output;
            };
        }
    };
}

fn gen_trait_def(
    EntraitAttrs { trait_ident, .. }: &EntraitAttrs,
    body: &EntraitFn,
) -> proc_macro2::TokenStream {
    let EntraitFn {
        fn_sig,
        trait_fn_inputs,
        ..
    } = body;
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

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
    body: &EntraitFn,
) -> Option<proc_macro2::TokenStream> {
    let EntraitFn {
        fn_sig,
        trait_fn_inputs,
        call_param_list,
        ..
    } = body;
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

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

impl Parse for EntraitFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fn_attrs = input.call(syn::Attribute::parse_outer)?;
        let fn_vis = input.parse()?;
        let fn_sig: syn::Signature = input.parse()?;
        let fn_body = input.parse()?;

        let sig_macro_ident = quote::format_ident!("__entrait_{}", fn_sig.ident);
        let trait_fn_inputs = extract_trait_fn_inputs(&fn_sig)?;
        let call_param_list = extract_call_param_list(&fn_sig)?;

        Ok(EntraitFn {
            fn_attrs,
            fn_vis,
            fn_sig,
            fn_body,
            sig_macro_ident,
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

impl Parse for EntraitMockInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mock_ident = input.parse()?;
        let mut trait_items: Vec<syn::ItemTrait> = Vec::new();

        while !input.is_empty() {
            trait_items.push(input.parse()?);
        }

        println!("parsed traits: {trait_items:?}");

        Ok(EntraitMockInput {
            mock_ident,
            trait_items,
        })
    }
}
