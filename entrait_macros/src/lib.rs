//! # entrait_macros
//!
//! Procedural macros used by entrait.

#![forbid(unsafe_code)]

use quote::quote;

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
    let attr = syn::parse_macro_input!(attr as EntraitAttr);
    let func = syn::parse_macro_input!(input as EntraitFn);

    let trait_def = gen_trait_def(&attr, &func);
    let multimock_macro = gen_multimock_macro(&attr, &func);
    let mock_deps_def = gen_mock_deps(&attr, &func);
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
        #multimock_macro
        #mock_deps_def
        #impl_block
    };

    if attr.debug {
        println!("{}", output);
    }

    proc_macro::TokenStream::from(output)
}

#[proc_macro]
pub fn generate_mock(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mock_input = syn::parse_macro_input!(input as EntraitGenerateMockInput);

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

fn gen_trait_def(attr: &EntraitAttr, func: &EntraitFn) -> proc_macro2::TokenStream {
    let trait_def = gen_trait_def_no_mock(attr, func);

    match attr.opt_mockall_automock_attribute() {
        Some(automock_attribute) => quote! {
            #automock_attribute
            #trait_def
        },
        None => trait_def,
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

fn gen_multimock_macro(attr: &EntraitAttr, func: &EntraitFn) -> Option<proc_macro2::TokenStream> {
    if !attr.mockable {
        return None;
    }

    let trait_ident = &attr.trait_ident;
    let macro_ident = quote::format_ident!("entrait_mock_{}", trait_ident);
    let trait_def = gen_trait_def_no_mock(attr, func);

    Some(quote! {
        #[allow(unused_macros)]
        macro_rules! #macro_ident {
            ($target:tt, [$($rest_macros:ident),*] $($traits:item)*) => {
                entrait::expand_mock!(
                    $target,
                    [$($rest_macros),*]
                    $($traits)*
                    #trait_def
                );
            };
        }
    })
}

fn gen_mock_deps(attr: &EntraitAttr, func: &EntraitFn) -> Option<proc_macro2::TokenStream> {
    let mock_deps_as_ident = attr.mock_deps_as.as_ref()?;

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

impl EntraitAttr {
    pub fn opt_mockall_automock_attribute(&self) -> Option<proc_macro2::TokenStream> {
        if self.mockable {
            Some(quote! { #[mockall::automock] })
        } else {
            None
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
            Some(quote! { #[async_trait::async_trait] })
        } else {
            None
        }
    }
}
