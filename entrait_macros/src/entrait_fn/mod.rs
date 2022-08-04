//! # entrait_macros
//!
//! Procedural macros used by entrait.
//!

pub mod input_attr;

use crate::analyze_generics;
use crate::analyze_generics::GenericsAnalyzer;
use crate::analyze_generics::TraitFn;
use crate::attributes;
use crate::generics::{self, TraitDependencyMode};
use crate::impl_fn_codegen;
use crate::input::FnInputMode;
use crate::input::{InputFn, InputMod, ModItem};
use crate::opt::*;
use crate::signature;
use crate::token_util::push_tokens;
use input_attr::*;

use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote_spanned;
use quote::{quote, ToTokens};

use crate::analyze_generics::detect_trait_dependency_mode;

pub fn entrait_for_single_fn(attr: &EntraitFnAttr, input_fn: InputFn) -> syn::Result<TokenStream> {
    let fn_input_mode = FnInputMode::SingleFn(&input_fn.fn_sig.ident);
    let mut generics_analyzer = GenericsAnalyzer::new();
    let trait_fns = [TraitFn::analyze(
        &input_fn,
        &mut generics_analyzer,
        signature::FnIndex(0),
        attr.trait_ident.span(),
        &attr.opts,
    )?];

    let trait_dependency_mode = detect_trait_dependency_mode(
        &fn_input_mode,
        &trait_fns,
        &attr.crate_idents,
        attr.trait_ident.span(),
    )?;
    let use_associated_future =
        generics::detect_use_associated_future(&attr.opts, [&input_fn].into_iter());

    let trait_generics = generics_analyzer.into_trait_generics();
    let trait_def = gen_trait_def(
        attr,
        &trait_generics,
        &trait_dependency_mode,
        &trait_fns,
        &fn_input_mode,
    )?;
    let impl_block = impl_fn_codegen::gen_impl_block(
        &attr.opts,
        &attr.crate_idents,
        &attr.trait_ident,
        attr.trait_ident.span(),
        generics::ImplIndirection::None,
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
    let fn_input_mode = FnInputMode::Module(&input_mod.ident);
    let mut generics_analyzer = analyze_generics::GenericsAnalyzer::new();
    let trait_fns = input_mod
        .items
        .iter()
        .filter_map(ModItem::filter_pub_fn)
        .enumerate()
        .map(|(index, input_fn)| {
            TraitFn::analyze(
                input_fn,
                &mut generics_analyzer,
                signature::FnIndex(index),
                attr.trait_ident.span(),
                &attr.opts,
            )
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let trait_dependency_mode = detect_trait_dependency_mode(
        &fn_input_mode,
        &trait_fns,
        &attr.crate_idents,
        attr.trait_ident.span(),
    )?;
    let use_associated_future = generics::detect_use_associated_future(
        &attr.opts,
        input_mod.items.iter().filter_map(ModItem::filter_pub_fn),
    );

    let trait_generics = generics_analyzer.into_trait_generics();
    let trait_def = gen_trait_def(
        attr,
        &trait_generics,
        &trait_dependency_mode,
        &trait_fns,
        &fn_input_mode,
    )?;
    let impl_block = impl_fn_codegen::gen_impl_block(
        &attr.opts,
        &attr.crate_idents,
        &attr.trait_ident,
        attr.trait_ident.span(),
        generics::ImplIndirection::None,
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

    let trait_vis = &attr.trait_visibility;
    let trait_ident = &attr.trait_ident;

    Ok(quote! {
        #(#attrs)*
        #vis #mod_token #mod_ident {
            #(#items)*

            #trait_def
            #impl_block
        }

        #trait_vis use #mod_ident::#trait_ident;
    })
}

fn gen_trait_def(
    attr: &EntraitFnAttr,
    trait_generics: &generics::TraitGenerics,
    trait_dependency_mode: &TraitDependencyMode,
    trait_fns: &[TraitFn],
    fn_input_mode: &FnInputMode<'_>,
) -> syn::Result<TokenStream> {
    let span = attr.trait_ident.span();

    let opt_unimock_attr = match attr.opts.default_option(attr.opts.unimock, false) {
        SpanOpt(true, span) => Some(attributes::ExportGatedAttr {
            params: attributes::UnimockAttrParams {
                crate_idents: &attr.crate_idents,
                trait_fns,
                mode: fn_input_mode,
                span,
            },
            opts: &attr.opts,
        }),
        _ => None,
    };

    // let opt_unimock_attr = attr.opt_unimock_attribute(trait_fns, mode);
    let opt_entrait_for_trait_attr = match trait_dependency_mode {
        TraitDependencyMode::Concrete(_) => {
            Some(attributes::Attr(attributes::EntraitForTraitParams {
                crate_idents: &attr.crate_idents,
            }))
        }
        _ => None,
    };

    let opt_mockall_automock_attr = match attr.opts.default_option(attr.opts.mockall, false) {
        SpanOpt(true, span) => Some(attributes::ExportGatedAttr {
            params: attributes::MockallAutomockParams { span },
            opts: &attr.opts,
        }),
        _ => None,
    };
    let opt_async_trait_attr = impl_fn_codegen::opt_async_trait_attribute(
        &attr.opts,
        &attr.crate_idents,
        trait_fns.iter(),
    );

    let trait_visibility = TraitVisibility {
        attr,
        fn_input_mode,
    };
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

struct TraitVisibility<'a> {
    attr: &'a EntraitFnAttr,
    fn_input_mode: &'a FnInputMode<'a>,
}

impl<'a> ToTokens for TraitVisibility<'a> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        match &self.fn_input_mode {
            FnInputMode::Module(_) => {
                match &self.attr.trait_visibility {
                    syn::Visibility::Inherited => {
                        // When the trait is "private", it should only be accessible to the module outside,
                        // so use `pub(super)`.
                        // This is because the trait is syntacitally "defined" outside the module, because
                        // the attribute is an outer attribute.
                        // If proc-macros supported inner attributes, and this was invoked with that, we wouldn't do this.
                        push_tokens!(stream, syn::token::Pub(Span::call_site()));
                        syn::token::Paren::default().surround(stream, |stream| {
                            push_tokens!(stream, syn::token::Super::default());
                        });
                    }
                    _ => {
                        push_tokens!(stream, self.attr.trait_visibility);
                    }
                }
            }
            FnInputMode::SingleFn(_) => {
                push_tokens!(stream, self.attr.trait_visibility);
            }
        }
    }
}
