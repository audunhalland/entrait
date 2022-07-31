//! # entrait_macros
//!
//! Procedural macros used by entrait.
//!

pub mod attr;

mod analyze_generics;
mod signature;

use crate::generics::{self, TraitDependencyMode};
use crate::input::InputFn;
use crate::opt::*;
use crate::token_util::{push_tokens, TokenPair};
use analyze_generics::GenericsAnalyzer;
use attr::*;

use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;

use self::analyze_generics::detect_trait_dependency_mode;

pub struct OutputFn<'i> {
    source: &'i InputFn,
    pub deps: generics::FnDeps,
    entrait_sig: signature::EntraitSignature,
}

impl<'i> OutputFn<'i> {
    fn analyze(
        source: &'i InputFn,
        analyzer: &mut GenericsAnalyzer,
        attr: &EntraitFnAttr,
    ) -> syn::Result<Self> {
        let deps = analyzer.analyze_fn_deps(&source, attr)?;
        let entrait_sig = signature::SignatureConverter::new(attr, &source, &deps).convert();
        Ok(Self {
            source,
            deps,
            entrait_sig,
        })
    }

    fn sig(&self) -> &syn::Signature {
        &self.entrait_sig.sig
    }
}

pub fn entrait_for_single_fn(attr: &EntraitFnAttr, input_fn: InputFn) -> syn::Result<TokenStream> {
    let mut generics_analyzer = analyze_generics::GenericsAnalyzer::new();
    let output_fn = OutputFn::analyze(&input_fn, &mut generics_analyzer, attr)?;
    let trait_generics = generics_analyzer.into_trait_generics();

    let output_fns = [output_fn];

    let trait_dependency_mode = detect_trait_dependency_mode(&output_fns, attr.trait_ident.span());
    let use_associated_future = detect_use_associated_future(attr, [&input_fn].into_iter());

    let trait_def = gen_trait_def(
        attr,
        &trait_generics,
        &trait_dependency_mode,
        &output_fns,
        Some(&input_fn.fn_sig.ident),
    )?;
    let impl_block = gen_impl_block(
        attr,
        &trait_generics,
        &output_fns,
        &trait_dependency_mode,
        use_associated_future,
    );

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
        #impl_block
    })
}

fn gen_trait_def(
    attr: &EntraitFnAttr,
    trait_generics: &generics::TraitGenerics,
    trait_dependency_mode: &TraitDependencyMode,
    output_fns: &[OutputFn],
    single_fn_ident: Option<&syn::Ident>,
) -> syn::Result<TokenStream> {
    let span = attr.trait_ident.span();

    let opt_unimock_attr = attr.opt_unimock_attribute(output_fns, single_fn_ident);
    let opt_entrait_for_trait_attr = match trait_dependency_mode {
        TraitDependencyMode::Concrete(_) => {
            Some(quote! { #[::entrait::entrait(unimock = false, mockall = false)] })
        }
        _ => None,
    };
    let opt_mockall_automock_attr = attr.opt_mockall_automock_attribute();
    let opt_async_trait_attr = opt_async_trait_attribute(attr, output_fns.iter());

    let trait_visibility = &attr.trait_visibility;
    let trait_ident = &attr.trait_ident;

    let fn_defs = output_fns.iter().map(|output_fn| {
        let opt_associated_fut_decl = &output_fn.entrait_sig.associated_fut_decl;
        let trait_fn_sig = output_fn.sig();

        quote! {
            #opt_associated_fut_decl
            #trait_fn_sig;
        }
    });

    let params = trait_generics.trait_params();
    let where_clause = trait_generics.trait_where_clause();

    Ok(quote_spanned! { span=>
        #opt_unimock_attr
        #opt_entrait_for_trait_attr
        #opt_mockall_automock_attr
        #opt_async_trait_attr
        #trait_visibility trait #trait_ident #params #where_clause {
            #(#fn_defs)*
        }
    })
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
fn gen_impl_block(
    attr: &EntraitFnAttr,
    trait_generics: &generics::TraitGenerics,
    output_fns: &[OutputFn],
    trait_dependency_mode: &TraitDependencyMode,
    use_associated_future: generics::UseAssociatedFuture,
) -> TokenStream {
    let span = attr.trait_ident.span();

    let async_trait_attribute = opt_async_trait_attribute(attr, output_fns.iter());
    let params = trait_generics.impl_params(trait_dependency_mode, use_associated_future);
    let trait_ident = &attr.trait_ident;
    let args = trait_generics.arguments();
    let self_ty = SelfTy(trait_dependency_mode, span);
    let where_clause = trait_generics.impl_where_clause(output_fns, trait_dependency_mode, span);

    let items = output_fns.iter().map(|output_fn| {
        let associated_fut_impl = &output_fn.entrait_sig.associated_fut_impl;

        let fn_item = gen_delegating_fn_item(output_fn, span);

        quote! {
            #associated_fut_impl
            #fn_item
        }
    });

    quote_spanned! { span=>
        #async_trait_attribute
        impl #params #trait_ident #args for #self_ty #where_clause {
            #(#items)*
        }
    }
}

struct SelfTy<'g>(&'g TraitDependencyMode<'g>, Span);

impl<'g> quote::ToTokens for SelfTy<'g> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let span = self.1;
        match &self.0 {
            TraitDependencyMode::Generic(idents) => {
                push_tokens!(stream, idents.impl_path(span))
            }
            TraitDependencyMode::Concrete(ty) => {
                push_tokens!(stream, ty)
            }
        }
    }
}

/// Generate the fn (in the impl block) that calls the entraited fn
fn gen_delegating_fn_item(output_fn: &OutputFn, span: Span) -> TokenStream {
    let entrait_sig = &output_fn.entrait_sig;
    let trait_fn_sig = &output_fn.sig();
    let deps = &output_fn.deps;

    let mut fn_ident = output_fn.source.fn_sig.ident.clone();
    fn_ident.set_span(span);

    let opt_self_comma = match (deps, entrait_sig.sig.inputs.first()) {
        (generics::FnDeps::NoDeps { .. }, _) | (_, None) => None,
        (_, Some(_)) => Some(TokenPair(
            syn::token::SelfValue(span),
            syn::token::Comma(span),
        )),
    };

    let arguments = entrait_sig
        .sig
        .inputs
        .iter()
        .filter_map(|fn_arg| match fn_arg {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                syn::Pat::Ident(pat_ident) => Some(&pat_ident.ident),
                _ => panic!("Found a non-ident pattern, this should be handled in signature.rs"),
            },
        });

    let mut opt_dot_await = output_fn.source.opt_dot_await(span);
    if entrait_sig.associated_fut_decl.is_some() {
        opt_dot_await = None;
    }

    quote_spanned! { span=>
        #trait_fn_sig {
            #fn_ident(#opt_self_comma #(#arguments),*) #opt_dot_await
        }
    }
}

impl EntraitFnAttr {
    fn opt_unimock_attribute(
        &self,
        output_fns: &[OutputFn],
        single_fn_ident: Option<&syn::Ident>,
    ) -> Option<TokenStream> {
        match self.default_option(self.opts.unimock, false) {
            SpanOpt(true, span) => {
                let unmocked =
                    output_fns.iter().map(|output_fn| {
                        let fn_ident = &output_fn.sig().ident;

                        match &output_fn.deps {
                            generics::FnDeps::Generic { .. } => quote! { #fn_ident },
                            generics::FnDeps::Concrete(_) => quote! { _ },
                            generics::FnDeps::NoDeps { .. } => {
                                let arguments = output_fn.sig().inputs.iter().filter_map(
                                    |fn_arg| match fn_arg {
                                        syn::FnArg::Receiver(_) => None,
                                        syn::FnArg::Typed(pat_type) => {
                                            match pat_type.pat.as_ref() {
                                                syn::Pat::Ident(pat_ident) => {
                                                    Some(&pat_ident.ident)
                                                }
                                                _ => None,
                                            }
                                        }
                                    },
                                );

                                quote! { #fn_ident(#(#arguments),*) }
                            }
                        }
                    });

                let opt_mock_mod = if let Some(fn_ident) = single_fn_ident {
                    Some(quote! { mod=#fn_ident, as=[Fn], })
                } else {
                    None
                };

                Some(self.gated_mock_attr(span, quote_spanned! {span=>
                    ::entrait::__unimock::unimock(prefix=::entrait::__unimock, #opt_mock_mod unmocked=[#(#unmocked),*])
                }))
            }
            _ => None,
        }
    }

    pub fn opt_mockall_automock_attribute(&self) -> Option<TokenStream> {
        match self.default_option(self.opts.mockall, false) {
            SpanOpt(true, span) => {
                Some(self.gated_mock_attr(span, quote_spanned! { span=> ::mockall::automock }))
            }
            _ => None,
        }
    }

    fn gated_mock_attr(&self, span: Span, attr: TokenStream) -> TokenStream {
        match self.export_value() {
            true => quote_spanned! {span=>
                #[#attr]
            },
            false => quote_spanned! {span=>
                #[cfg_attr(test, #attr)]
            },
        }
    }
}

impl InputFn {
    fn opt_dot_await(&self, span: Span) -> Option<TokenStream> {
        if self.fn_sig.asyncness.is_some() {
            Some(quote_spanned! { span=> .await })
        } else {
            None
        }
    }

    pub fn use_associated_future(&self, attr: &EntraitFnAttr) -> bool {
        matches!(
            (attr.async_strategy(), self.fn_sig.asyncness),
            (SpanOpt(AsyncStrategy::AssociatedFuture, _), Some(_async))
        )
    }
}

fn opt_async_trait_attribute<'o>(
    attr: &EntraitFnAttr,
    output_fns: impl Iterator<Item = &'o OutputFn<'o>>,
) -> Option<TokenStream> {
    match (
        attr.async_strategy(),
        has_any_async(output_fns.map(|output_fn| output_fn.sig())),
    ) {
        (SpanOpt(AsyncStrategy::AsyncTrait, span), true) => {
            Some(quote_spanned! { span=> #[::entrait::__async_trait::async_trait] })
        }
        _ => None,
    }
}

fn detect_use_associated_future<'i>(
    attr: &EntraitFnAttr,
    input_fns: impl Iterator<Item = &'i InputFn>,
) -> generics::UseAssociatedFuture {
    generics::UseAssociatedFuture(matches!(
        (
            attr.async_strategy(),
            has_any_async(input_fns.map(|input_fn| &input_fn.fn_sig))
        ),
        (SpanOpt(AsyncStrategy::AssociatedFuture, _), true)
    ))
}

fn has_any_async<'s>(mut signatures: impl Iterator<Item = &'s syn::Signature>) -> bool {
    signatures.any(|sig| sig.asyncness.is_some())
}
