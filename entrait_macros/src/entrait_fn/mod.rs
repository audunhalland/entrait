//! # entrait_macros
//!
//! Procedural macros used by entrait.
//!

pub mod input_attr;

use crate::analyze_generics;
use crate::analyze_generics::GenericsAnalyzer;
use crate::analyze_generics::TraitFnAnalyzer;
use crate::fn_delegation_codegen;
use crate::generics;
use crate::input::FnInputMode;
use crate::input::{InputFn, InputMod, ModItem};
use crate::signature;
use crate::trait_codegen::Supertraits;
use crate::trait_codegen::TraitCodegen;
use input_attr::*;

use proc_macro2::TokenStream;
use quote::quote;

use crate::analyze_generics::detect_trait_dependency_mode;

pub fn entrait_for_single_fn(attr: &EntraitFnAttr, input_fn: InputFn) -> syn::Result<TokenStream> {
    let fn_input_mode = FnInputMode::SingleFn(&input_fn.fn_sig.ident);
    let mut generics_analyzer = GenericsAnalyzer::new();

    let trait_fns = [TraitFnAnalyzer {
        impl_receiver_kind: signature::ImplReceiverKind::SelfRef,
        trait_span: attr.trait_ident.span(),
        crate_idents: &attr.crate_idents,
        opts: &attr.opts,
    }
    .analyze(input_fn.input_sig(), &mut generics_analyzer)?];

    let trait_dependency_mode = detect_trait_dependency_mode(
        &fn_input_mode,
        &trait_fns,
        &attr.crate_idents,
        attr.trait_ident.span(),
    )?;
    let use_associated_future =
        generics::detect_use_associated_future(&attr.opts, [&input_fn].into_iter());

    let trait_generics = generics_analyzer.into_trait_generics();
    let trait_def = TraitCodegen {
        opts: &attr.opts,
        crate_idents: &attr.crate_idents,
        trait_indirection: generics::TraitIndirection::Plain,
        trait_dependency_mode: &trait_dependency_mode,
    }
    .gen_trait_def(
        &attr.trait_visibility,
        &attr.trait_ident,
        &trait_generics,
        &Supertraits::None,
        &trait_fns,
        &fn_input_mode,
    )?;
    let impl_block = fn_delegation_codegen::FnDelegationCodegen {
        opts: &attr.opts,
        crate_idents: &attr.crate_idents,
        trait_ref: &attr.trait_ident,
        trait_span: attr.trait_ident.span(),
        impl_indirection: generics::ImplIndirection::None,
        trait_generics: &trait_generics,
        fn_input_mode: &fn_input_mode,
        trait_dependency_mode: &trait_dependency_mode,
        use_associated_future,
    }
    .gen_impl_block(&trait_fns);

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
        .map(|input_fn| {
            TraitFnAnalyzer {
                impl_receiver_kind: signature::ImplReceiverKind::SelfRef,
                trait_span: attr.trait_ident.span(),
                crate_idents: &attr.crate_idents,
                opts: &attr.opts,
            }
            .analyze(input_fn.input_sig(), &mut generics_analyzer)
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
    let trait_def = TraitCodegen {
        opts: &attr.opts,
        crate_idents: &attr.crate_idents,
        trait_indirection: generics::TraitIndirection::Plain,
        trait_dependency_mode: &trait_dependency_mode,
    }
    .gen_trait_def(
        &attr.trait_visibility,
        &attr.trait_ident,
        &trait_generics,
        &Supertraits::None,
        &trait_fns,
        &fn_input_mode,
    )?;
    let impl_block = fn_delegation_codegen::FnDelegationCodegen {
        opts: &attr.opts,
        crate_idents: &attr.crate_idents,
        trait_ref: &attr.trait_ident,
        trait_span: attr.trait_ident.span(),
        impl_indirection: generics::ImplIndirection::None,
        trait_generics: &trait_generics,
        fn_input_mode: &fn_input_mode,
        trait_dependency_mode: &trait_dependency_mode,
        use_associated_future,
    }
    .gen_impl_block(&trait_fns);

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
