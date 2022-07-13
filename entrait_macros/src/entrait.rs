//! # entrait_macros
//!
//! Procedural macros used by entrait.

use proc_macro2::Span;
use proc_macro2::TokenStream;
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
    let deps = deps::analyze_deps(&input_fn, attr)?;
    let trait_def = gen_trait_def(attr, &input_fn, &deps)?;
    let impl_blocks = gen_impl_blocks(attr, &input_fn, &deps)?;

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
        #impl_blocks
    })
}

fn gen_trait_def(
    attr: &EntraitAttr,
    input_fn: &InputFn,
    deps: &deps::Deps,
) -> syn::Result<proc_macro2::TokenStream> {
    let span = attr.trait_ident.span();
    let trait_def = gen_trait_def_no_mock(attr, input_fn, deps)?;

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
    deps: &deps::Deps,
) -> syn::Result<proc_macro2::TokenStream> {
    let InputFn { fn_sig, .. } = input_fn;
    let trait_visibility = &attr.trait_visibility;
    let trait_ident = &attr.trait_ident;
    let span = trait_ident.span();
    let input_fn_ident = &fn_sig.ident;
    let return_type = &fn_sig.output;

    let opt_async = input_fn.opt_async();
    let trait_fn_inputs = input_fn.trait_fn_inputs(span, deps, attr);

    if let AsyncStrategy::AssociatedFuture = attr.get_async_strategy().0 {
        let output_ty = output_type_tokens(return_type);

        Ok(quote_spanned! { span=>
            #trait_visibility trait #trait_ident {
                type Fut<'entrait>: ::core::future::Future<Output = #output_ty> + Send
                where
                    Self: 'entrait;

                fn #input_fn_ident<'entrait>(#trait_fn_inputs) -> Self::Fut<'entrait>;
            }
        })
    } else {
        let opt_async_trait_attr = input_fn.opt_async_trait_attribute(attr);

        Ok(quote_spanned! { span=>
            #opt_async_trait_attr
            #trait_visibility trait #trait_ident {
                #opt_async fn #input_fn_ident(#trait_fn_inputs) #return_type;
            }
        })
    }
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
fn gen_impl_blocks(
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
    let return_type = &fn_sig.output;

    let async_trait_attribute = input_fn.opt_async_trait_attribute(attr);
    let trait_fn_inputs = input_fn.trait_fn_inputs(span, deps, attr);

    // Where bounds on the entire impl block,
    // TODO: Is it correct to always use `Sync` in here here?
    // It must be for Async at least?
    let impl_where_bounds = match deps {
        deps::Deps::Generic { trait_bounds } => {
            let impl_trait_bounds = if trait_bounds.is_empty() {
                None
            } else {
                Some(quote! {
                    ::entrait::Impl<EntraitT>: #(#trait_bounds)+*,
                })
            };

            let standard_bounds = if let AsyncStrategy::AssociatedFuture = attr.get_async_strategy().0 {
                // Deps must be 'static for zero-cost futures to work
                quote! { Sync + 'static }
            } else {
                quote! { Sync }
            };

            quote_spanned! { span=>
                where #impl_trait_bounds EntraitT: #standard_bounds
            }
        }
        deps::Deps::Concrete(_) => quote_spanned! { span=>
            where EntraitT: #trait_ident + Sync
        },
        deps::Deps::NoDeps => quote_spanned! { span=>
            where EntraitT: Sync
        },
    };

    // Future type for 'associated_future' feature
    let fut_type = if let AsyncStrategy::AssociatedFuture = attr.get_async_strategy().0 {
        let output_ty = output_type_tokens(return_type);
        Some(quote_spanned! { span=>
            type Fut<'entrait> = impl ::core::future::Future<Output = #output_ty>
            where
                Self: 'entrait;
        })
    } else {
        None
    };

    let generic_fn_def = gen_delegating_fn_item(
        span,
        input_fn,
        &input_fn_ident,
        &trait_fn_inputs,
        match deps {
            deps::Deps::Generic { trait_bounds: _ } => FnReceiverKind::SelfArg,
            deps::Deps::Concrete(_) => FnReceiverKind::SelfAsRefReceiver,
            deps::Deps::NoDeps => FnReceiverKind::RefSelfArg,
        },
        deps,
        attr,
    )?;

    let generic_impl_block = quote_spanned! { span=>
        #async_trait_attribute
        impl<EntraitT> #trait_ident for ::entrait::Impl<EntraitT> #impl_where_bounds {
            #fut_type
            #generic_fn_def
        }
    };

    Ok(match deps {
        deps::Deps::Concrete(path) => {
            let concrete_fn_def = gen_delegating_fn_item(
                span,
                input_fn,
                &input_fn_ident,
                &trait_fn_inputs,
                FnReceiverKind::SelfArg,
                deps,
                attr,
            )?;

            quote_spanned! { span=>
                #generic_impl_block

                // Specific impl for the concrete type:
                #async_trait_attribute
                impl #trait_ident for #path {
                    #fut_type
                    #concrete_fn_def
                }
            }
        }
        _ => generic_impl_block,
    })
}

fn gen_delegating_fn_item(
    span: Span,
    input_fn: &InputFn,
    fn_ident: &syn::Ident,
    trait_fn_inputs: &TokenStream,
    receiver_kind: FnReceiverKind,
    deps: &deps::Deps,
    attr: &EntraitAttr,
) -> syn::Result<proc_macro2::TokenStream> {
    let opt_async = input_fn.opt_async();
    let opt_dot_await = input_fn.opt_dot_await(span);
    // TODO: set span for output
    let return_type = &input_fn.fn_sig.output;

    let params = input_fn
        .fn_sig
        .inputs
        .iter()
        .enumerate()
        .filter_map(|(index, arg)| {
            if deps.is_deps_param(index) {
                match receiver_kind {
                    FnReceiverKind::SelfArg => Some(Ok(quote_spanned! { span=> self })),
                    FnReceiverKind::RefSelfArg => Some(Ok(quote_spanned! { span=> &self })),
                    FnReceiverKind::SelfAsRefReceiver => None,
                }
            } else {
                Some(match arg {
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
                })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let function_call = match receiver_kind {
        FnReceiverKind::SelfAsRefReceiver => quote_spanned! { span=>
            self.as_ref().#fn_ident(#(#params),*)
        },
        _ => quote_spanned! { span=>
            #fn_ident(#(#params),*)
        },
    };

    Ok(if let AsyncStrategy::AssociatedFuture = attr.get_async_strategy().0 {
        quote_spanned! { span=>
            fn #fn_ident<'entrait>(#trait_fn_inputs) -> Self::Fut<'entrait> {
                #function_call
            }
        }
    } else {
        quote_spanned! { span=>
            #opt_async fn #fn_ident(#trait_fn_inputs) #return_type {
                #function_call #opt_dot_await
            }
        }
    })
}

impl EntraitAttr {
    pub fn opt_unimock_attribute(
        &self,
        input_fn: &InputFn,
        deps: &deps::Deps,
    ) -> Option<proc_macro2::TokenStream> {
        match self.default_option(self.unimock, false) {
            SpanOpt(true, span) => {
                let fn_ident = &input_fn.fn_sig.ident;

                let unmocked = match deps {
                    deps::Deps::Generic { trait_bounds: _ } => quote! { #fn_ident },
                    deps::Deps::Concrete(_) => quote! { _ },
                    deps::Deps::NoDeps => {
                        let inputs =
                            input_fn
                                .fn_sig
                                .inputs
                                .iter()
                                .filter_map(|fn_arg| match fn_arg {
                                    syn::FnArg::Receiver(_) => None,
                                    syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                                        syn::Pat::Ident(pat_ident) => Some(&pat_ident.ident),
                                        _ => None,
                                    },
                                });

                        quote! { #fn_ident(#(#inputs),*) }
                    }
                };

                Some(self.gated_mock_attr(span, quote_spanned! {span=>
                    ::entrait::__unimock::unimock(prefix=::entrait::__unimock, mod=#fn_ident, as=[Fn], unmocked=[#unmocked])
                }))
            }
            _ => None,
        }
    }

    pub fn opt_mockall_automock_attribute(&self) -> Option<proc_macro2::TokenStream> {
        match self.default_option(self.mockall, false) {
            SpanOpt(true, span) => {
                Some(self.gated_mock_attr(span, quote_spanned! { span=> ::mockall::automock }))
            }
            _ => None,
        }
    }

    fn gated_mock_attr(&self, span: Span, attr: TokenStream) -> TokenStream {
        match self.default_option(self.export, false).0 {
            true => quote_spanned! {span=>
                #[#attr]
            },
            false => quote_spanned! {span=>
                #[cfg_attr(test, #attr)]
            },
        }
    }
}

enum FnReceiverKind {
    /// f(self, ..)
    SelfArg,
    /// f(&self, ..)
    RefSelfArg,
    /// self.as_ref().f(..)
    SelfAsRefReceiver,
}

impl InputFn {
    fn opt_async(&self) -> Option<proc_macro2::TokenStream> {
        if let Some(async_) = self.fn_sig.asyncness {
            Some(quote! { #async_ })
        } else {
            None
        }
    }

    fn opt_dot_await(&self, span: Span) -> Option<proc_macro2::TokenStream> {
        if let Some(_) = self.fn_sig.asyncness {
            Some(quote_spanned! { span=> .await })
        } else {
            None
        }
    }

    fn opt_async_trait_attribute(&self, attr: &EntraitAttr) -> Option<proc_macro2::TokenStream> {
        match (
            attr.get_async_strategy(),
            self.fn_sig.asyncness,
        ) {
            (SpanOpt(AsyncStrategy::AsyncTrait, span), Some(_async)) => {
                Some(quote_spanned! { span=> #[::entrait::__async_trait::async_trait] })
            }
            _ => None,
        }
    }

    fn trait_fn_inputs(
        &self,
        span: Span,
        deps: &deps::Deps,
        attr: &EntraitAttr,
    ) -> proc_macro2::TokenStream {
        let mut input_args = self.fn_sig.inputs.clone();

        // strip away attributes
        for fn_arg in input_args.iter_mut() {
            match fn_arg {
                syn::FnArg::Receiver(receiver) => {
                    receiver.attrs = vec![];
                }
                syn::FnArg::Typed(pat_type) => {
                    pat_type.attrs = vec![];
                }
            }
        }

        let self_lifetime = if let AsyncStrategy::AssociatedFuture = attr.get_async_strategy().0 {
            Some(quote! { 'entrait })
        } else {
            None
        };

        match deps {
            deps::Deps::NoDeps => {
                input_args.insert(
                    0,
                    syn::parse_quote_spanned! { span=> & #self_lifetime self },
                );
            }
            _ => {
                if input_args.is_empty() {
                    return if let AsyncStrategy::AssociatedFuture = attr.get_async_strategy().0 {
                        quote! { & #self_lifetime self }
                    } else {
                        quote! {}
                    };
                }

                let first_mut = input_args.first_mut().unwrap();
                *first_mut = syn::parse_quote_spanned! { span=> & #self_lifetime self };
            }
        }

        quote! {
            #input_args
        }
    }
}

fn output_type_tokens(return_type: &syn::ReturnType) -> TokenStream {
    match return_type {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    }
}
