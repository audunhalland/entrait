//! # entrait_macros
//!
//! Procedural macros used by entrait.
//!

pub mod attr;

mod analyze_generics;
mod attributes;
mod signature;

use crate::generics::{self, TraitDependencyMode};
use crate::input::{InputFn, InputMod, ModItem};
use crate::opt::*;
use crate::token_util::{push_tokens, TokenPair};
use analyze_generics::GenericsAnalyzer;
use attr::*;

use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;

use self::analyze_generics::detect_trait_dependency_mode;

enum Mode<'a> {
    SingleFn(&'a syn::Ident),
    Module,
}

pub fn entrait_for_single_fn(attr: &EntraitFnAttr, input_fn: InputFn) -> syn::Result<TokenStream> {
    let mut generics_analyzer = analyze_generics::GenericsAnalyzer::new();
    let trait_fns = [TraitFn::analyze(&input_fn, &mut generics_analyzer, attr)?];

    let trait_dependency_mode = detect_trait_dependency_mode(&trait_fns, attr.trait_ident.span());
    let use_associated_future = detect_use_associated_future(attr, [&input_fn].into_iter());

    let trait_generics = generics_analyzer.into_trait_generics();
    let trait_def = gen_trait_def(
        attr,
        &trait_generics,
        &trait_dependency_mode,
        &trait_fns,
        &Mode::SingleFn(&input_fn.fn_sig.ident),
    )?;
    let impl_block = gen_impl_block(
        attr,
        &trait_generics,
        &trait_dependency_mode,
        &trait_fns,
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

pub fn entrait_for_mod(attr: &EntraitFnAttr, input_mod: InputMod) -> syn::Result<TokenStream> {
    let mut generics_analyzer = analyze_generics::GenericsAnalyzer::new();
    let trait_fns = input_mod
        .items
        .iter()
        .filter_map(ModItem::filter_pub_fn)
        .map(|input_fn| TraitFn::analyze(input_fn, &mut generics_analyzer, attr))
        .collect::<syn::Result<Vec<_>>>()?;

    let trait_dependency_mode = detect_trait_dependency_mode(&trait_fns, attr.trait_ident.span());
    let use_associated_future = detect_use_associated_future(
        attr,
        input_mod.items.iter().filter_map(ModItem::filter_pub_fn),
    );

    let trait_generics = generics_analyzer.into_trait_generics();
    let trait_def = gen_trait_def(
        attr,
        &trait_generics,
        &trait_dependency_mode,
        &trait_fns,
        &Mode::Module,
    )?;
    let impl_block = gen_impl_block(
        attr,
        &trait_generics,
        &trait_dependency_mode,
        &trait_fns,
        use_associated_future,
    );

    let InputMod {
        attrs,
        vis,
        mod_token,
        ident: mod_ident,
        items,
        ..
    } = input_mod;

    let use_stmt = {
        let trait_vis = &attr.trait_visibility;
        let trait_ident = &attr.trait_ident;

        quote! {
            #trait_vis use #mod_ident::#trait_ident;
        }
    };

    Ok(quote! {
        #(#attrs)*
        #vis #mod_token #mod_ident {
            #(#items)*

            #trait_def
            #impl_block
        }

        #use_stmt
    })
}

pub struct TraitFn<'i> {
    source: &'i InputFn,
    pub deps: generics::FnDeps,
    entrait_sig: signature::EntraitSignature,
}

impl<'i> TraitFn<'i> {
    fn analyze(
        source: &'i InputFn,
        analyzer: &mut GenericsAnalyzer,
        attr: &EntraitFnAttr,
    ) -> syn::Result<Self> {
        let deps = analyzer.analyze_fn_deps(source, attr)?;
        let entrait_sig = signature::SignatureConverter::new(attr, source, &deps).convert();
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

fn gen_trait_def(
    attr: &EntraitFnAttr,
    trait_generics: &generics::TraitGenerics,
    trait_dependency_mode: &TraitDependencyMode,
    trait_fns: &[TraitFn],
    mode: &Mode<'_>,
) -> syn::Result<TokenStream> {
    let span = attr.trait_ident.span();

    let opt_unimock_attr = match attr.default_option(attr.opts.unimock, false) {
        SpanOpt(true, span) => Some(attributes::ExportGatedAttr {
            params: attributes::UnimockAttrParams {
                trait_fns,
                mode,
                span,
            },
            attr,
        }),
        _ => None,
    };

    // let opt_unimock_attr = attr.opt_unimock_attribute(trait_fns, mode);
    let opt_entrait_for_trait_attr = match trait_dependency_mode {
        TraitDependencyMode::Concrete(_) => {
            Some(quote! { #[::entrait::entrait(unimock = false, mockall = false)] })
        }
        _ => None,
    };
    let opt_mockall_automock_attr = attr.opt_mockall_automock_attribute();
    let opt_async_trait_attr = opt_async_trait_attribute(attr, trait_fns.iter());

    let trait_visibility = &attr.trait_visibility;
    let trait_ident = &attr.trait_ident;

    let fn_defs = trait_fns.iter().map(|trait_fn| {
        let opt_associated_fut_decl = &trait_fn.entrait_sig.associated_fut_decl;
        let trait_fn_sig = trait_fn.sig();

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
    trait_dependency_mode: &TraitDependencyMode,
    trait_fns: &[TraitFn],
    use_associated_future: generics::UseAssociatedFuture,
) -> TokenStream {
    let span = attr.trait_ident.span();

    let async_trait_attribute = opt_async_trait_attribute(attr, trait_fns.iter());
    let params = trait_generics.impl_params(trait_dependency_mode, use_associated_future);
    let trait_ident = &attr.trait_ident;
    let args = trait_generics.arguments();
    let self_ty = SelfTy(trait_dependency_mode, span);
    let where_clause = trait_generics.impl_where_clause(trait_fns, trait_dependency_mode, span);

    let items = trait_fns.iter().map(|trait_fn| {
        let associated_fut_impl = &trait_fn.entrait_sig.associated_fut_impl;

        let fn_item = gen_delegating_fn_item(trait_fn, span);

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
fn gen_delegating_fn_item(trait_fn: &TraitFn, span: Span) -> TokenStream {
    let entrait_sig = &trait_fn.entrait_sig;
    let trait_fn_sig = &trait_fn.sig();
    let deps = &trait_fn.deps;

    let mut fn_ident = trait_fn.source.fn_sig.ident.clone();
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

    let mut opt_dot_await = trait_fn.source.opt_dot_await(span);
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
    trait_fns: impl Iterator<Item = &'o TraitFn<'o>>,
) -> Option<TokenStream> {
    match (
        attr.async_strategy(),
        has_any_async(trait_fns.map(|trait_fn| trait_fn.sig())),
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
