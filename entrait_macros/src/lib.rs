//! # entrait_macros
//!
//! Procedural macros used by entrait.

#![forbid(unsafe_code)]

use quote::quote;
use syn::parse::{Parse, ParseStream};

extern crate proc_macro;

mod bounds;
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
    let attrs = syn::parse_macro_input!(attr as EntraitAttrs);
    let func = syn::parse_macro_input!(input as EntraitFn);

    let trait_def = gen_trait_def(&attrs, &func);
    let multimock_macro = gen_multimock_macro(&attrs, &func);
    let mock_deps_def = gen_mock_deps(&attrs, &func);
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
        #multimock_macro
        #mock_deps_def
        #impl_block
    };

    if attrs.debug {
        println!("{}", output);
    }

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

        let attrs = trait_item.attrs;

        quote! {
            #(#attrs)*
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

fn gen_trait_def(attrs: &EntraitAttrs, func: &EntraitFn) -> proc_macro2::TokenStream {
    let EntraitFn {
        fn_sig,
        trait_fn_inputs,
        ..
    } = func;
    let trait_ident = &attrs.trait_ident;
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

    let opt_automock_attribute = attrs.opt_mockall_automock_attribute();
    let opt_async_trait_attr = func.opt_async_trait_attribute(attrs);
    let opt_async = func.opt_async();

    quote! {
        #opt_automock_attribute
        #opt_async_trait_attr
        pub trait #trait_ident {
            #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output;
        }
    }
}

fn gen_multimock_macro(attrs: &EntraitAttrs, func: &EntraitFn) -> Option<proc_macro2::TokenStream> {
    if !attrs.mockable {
        return None;
    }

    let EntraitFn {
        fn_sig,
        trait_fn_inputs,
        ..
    } = func;
    let trait_ident = &attrs.trait_ident;
    let macro_ident = quote::format_ident!("entrait_mock_{}", trait_ident);
    let opt_async_trait_attr = func.opt_async_trait_attribute(attrs);
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

    let opt_async = func.opt_async();

    Some(quote! {
        #[allow(unused_macros)]
        macro_rules! #macro_ident {
            ($target:tt, [$($rest_macros:ident),*] $($traits:item)*) => {
                entrait::expand_mock!(
                    $target,
                    [$($rest_macros),*]
                    $($traits)*
                    #opt_async_trait_attr
                    trait #trait_ident {
                        #opt_async fn #input_fn_ident(#trait_fn_inputs) #fn_output;
                    }
                );
            };
        }
    })
}

fn gen_mock_deps(attrs: &EntraitAttrs, func: &EntraitFn) -> Option<proc_macro2::TokenStream> {
    let mock_deps_as_ident = attrs.mock_deps_as.as_ref()?;

    match bounds::extract_first_arg_bounds(func) {
        Ok(path_bounds) => {
            let macros_to_call = path_bounds.into_iter().map(|path| {
                let type_ident = &path.segments.last().unwrap().ident;
                quote::format_ident!("entrait_mock_{}", type_ident)
            });

            Some(quote! {
                entrait::expand_mock!(#mock_deps_as_ident, [#(#macros_to_call),*]);
            })
        }
        Err(error) => Some(error.to_compile_error()),
    }
}

fn gen_impl_block(attrs: &EntraitAttrs, func: &EntraitFn) -> Option<proc_macro2::TokenStream> {
    let EntraitAttrs {
        trait_ident,
        impl_target_type,
        ..
    } = attrs;
    let EntraitFn {
        fn_sig,
        trait_fn_inputs,
        call_param_list,
        ..
    } = func;
    let input_fn_ident = &fn_sig.ident;
    let fn_output = &fn_sig.output;

    let async_trait_attribute = func.opt_async_trait_attribute(attrs);
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

impl Parse for EntraitMockInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mock_ident = input.parse()?;
        let mut trait_items: Vec<syn::ItemTrait> = Vec::new();

        while !input.is_empty() {
            trait_items.push(input.parse()?);
        }

        Ok(EntraitMockInput {
            mock_ident,
            trait_items,
        })
    }
}
