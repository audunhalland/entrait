pub mod input_attr;

use crate::analyze_generics;
use crate::analyze_generics::detect_trait_dependency_mode;
use crate::analyze_generics::TraitFnAnalyzer;
use crate::generics;
use crate::impl_fn_codegen;
use crate::input::{InputMod, ModItem};
use crate::signature;
use input_attr::EntraitImplAttr;

use quote::quote;
use syn::spanned::Spanned;

#[derive(Clone, Copy)]
pub enum ImplKind {
    Static,
    Dyn,
}

pub fn output_tokens(
    attr: EntraitImplAttr,
    input_mod: InputMod,
    kind: ImplKind,
) -> syn::Result<proc_macro2::TokenStream> {
    let derive_impl = match input_mod
        .items
        .iter()
        .filter_map(ModItem::filter_derive_impl)
        .next()
    {
        Some(derive_impl) => derive_impl,
        None => {
            return missing_derive_impl(input_mod);
        }
    };

    let impl_ident = &derive_impl.ident;
    let trait_span = derive_impl
        .trait_path
        .0
        .segments
        .last()
        .map(|segment| segment.span())
        .unwrap_or_else(proc_macro2::Span::call_site);

    let mut generics_analyzer = analyze_generics::GenericsAnalyzer::new();
    let trait_fns = input_mod
        .items
        .iter()
        .filter_map(ModItem::filter_pub_fn)
        .enumerate()
        .map(|(index, input_fn)| {
            TraitFnAnalyzer {
                impl_receiver_kind: match kind {
                    ImplKind::Static => signature::ImplReceiverKind::StaticImpl,
                    ImplKind::Dyn => signature::ImplReceiverKind::DynamicImpl,
                },
                trait_span,
                crate_idents: &attr.crate_idents,
                opts: &attr.opts,
            }
            .analyze(
                input_fn.input_sig(),
                signature::FnIndex(index),
                &mut generics_analyzer,
            )
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let trait_generics = generics_analyzer.into_trait_generics();

    let trait_dependency_mode = detect_trait_dependency_mode(
        &crate::input::FnInputMode::Module(&input_mod.ident),
        &trait_fns,
        &attr.crate_idents,
        trait_span,
    )?;
    let use_associated_future = generics::detect_use_associated_future(
        &attr.opts,
        input_mod.items.iter().filter_map(ModItem::filter_pub_fn),
    );
    let impl_indirection = match kind {
        ImplKind::Static => generics::ImplIndirection::Static { ident: impl_ident },
        ImplKind::Dyn => generics::ImplIndirection::Dynamic { ident: impl_ident },
    };

    let impl_block = impl_fn_codegen::ImplCodegen {
        opts: &attr.opts,
        crate_idents: &attr.crate_idents,
        trait_ref: &derive_impl.trait_path.0,
        trait_span,
        impl_indirection,
        trait_generics: &trait_generics,
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
    } = &input_mod;

    // BUG: Identical
    Ok(match kind {
        ImplKind::Static => quote! {
            #(#attrs)*
            #vis #mod_token #mod_ident {
                #(#items)*
                #impl_block
            }
        },
        ImplKind::Dyn => quote! {
            #(#attrs)*
            #vis #mod_token #mod_ident {
                #(#items)*
                #impl_block
            }
        },
    })
}

fn missing_derive_impl(input_mod: InputMod) -> syn::Result<proc_macro2::TokenStream> {
    let InputMod {
        attrs,
        vis,
        mod_token,
        ident: mod_ident,
        items,
        ..
    } = &input_mod;

    let error = syn::Error::new(
        proc_macro2::Span::call_site(),
        format!("Module {mod_ident} contains no `#[derive_impl(Trait)] pub struct SomeStruct;`"),
    )
    .into_compile_error();

    Ok(quote! {
        #(#attrs)*
        #vis #mod_token #mod_ident {
            #(#items)*
        }

        #error
    })
}
