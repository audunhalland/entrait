pub mod input_attr;

use crate::analyze_generics;
use crate::analyze_generics::detect_trait_dependency_mode;
use crate::analyze_generics::TraitFnAnalyzer;
use crate::fn_delegation_codegen;
use crate::generics;
use crate::input::ImplItem;
use crate::input::InputImpl;
use crate::input::{InputMod, ModItem};
use crate::opt::AsyncStrategy;
use crate::opt::SpanOpt;
use crate::signature;
use input_attr::EntraitImplAttr;

use quote::quote;
use syn::spanned::Spanned;

use self::input_attr::EntraitSimpleImplAttr;

#[derive(Clone, Copy)]
pub enum ImplKind {
    Static,
    Dyn,
}

pub fn output_tokens_for_impl(
    mut attr: EntraitSimpleImplAttr,
    InputImpl {
        attrs,
        unsafety,
        impl_token,
        trait_path,
        for_token: _,
        self_ty,
        brace_token: _,
        items,
    }: InputImpl,
) -> syn::Result<proc_macro2::TokenStream> {
    let impl_kind = if attr.dyn_token.is_some() {
        ImplKind::Dyn
    } else {
        ImplKind::Static
    };

    // Using a dyn implementation implies boxed futures.
    if matches!(impl_kind, ImplKind::Dyn) {
        attr.opts.async_strategy = Some(SpanOpt(AsyncStrategy::AsyncTrait, self_ty.span()));
    }

    let trait_span = trait_path
        .segments
        .last()
        .map(|segment| segment.span())
        .unwrap_or_else(proc_macro2::Span::call_site);

    let mut generics_analyzer = analyze_generics::GenericsAnalyzer::new();
    let trait_fns = items
        .iter()
        .filter_map(ImplItem::filter_fn)
        .map(|input_fn| {
            TraitFnAnalyzer {
                impl_receiver_kind: match impl_kind {
                    ImplKind::Static => signature::ImplReceiverKind::StaticImpl,
                    ImplKind::Dyn => signature::ImplReceiverKind::DynamicImpl,
                },
                trait_span,
                crate_idents: &attr.crate_idents,
                opts: &attr.opts,
            }
            .analyze(input_fn.input_sig(), &mut generics_analyzer)
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let trait_generics = generics_analyzer.into_trait_generics();

    let fn_input_mode = crate::input::FnInputMode::ImplBlock(&self_ty);
    let trait_dependency_mode =
        detect_trait_dependency_mode(&fn_input_mode, &trait_fns, &attr.crate_idents, trait_span)?;
    let use_associated_future = generics::detect_use_associated_future(
        &attr.opts,
        items.iter().filter_map(ImplItem::filter_fn),
    );

    let impl_indirection = match impl_kind {
        ImplKind::Static => generics::ImplIndirection::Static { ty: &self_ty },
        ImplKind::Dyn => generics::ImplIndirection::Dynamic { ty: &self_ty },
    };

    let impl_block = fn_delegation_codegen::FnDelegationCodegen {
        opts: &attr.opts,
        crate_idents: &attr.crate_idents,
        trait_ref: &trait_path,
        trait_span,
        impl_indirection,
        trait_generics: &trait_generics,
        fn_input_mode: &fn_input_mode,
        trait_dependency_mode: &trait_dependency_mode,
        use_associated_future,
    }
    .gen_impl_block(&trait_fns);

    Ok(quote! {
        #(#attrs)*
        #unsafety #impl_token #self_ty {
            #(#items)*
        }
        #impl_block
    })
}

pub fn output_tokens_for_mod(
    mut attr: EntraitImplAttr,
    input_mod: InputMod,
    impl_kind: ImplKind,
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

    // Using a dyn implementation implies boxed futures.
    if matches!(impl_kind, ImplKind::Dyn) {
        attr.opts.async_strategy = Some(SpanOpt(AsyncStrategy::AsyncTrait, input_mod.ident.span()));
    }

    let impl_ty = &derive_impl.ty;
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
        .map(|input_fn| {
            TraitFnAnalyzer {
                impl_receiver_kind: match impl_kind {
                    ImplKind::Static => signature::ImplReceiverKind::StaticImpl,
                    ImplKind::Dyn => signature::ImplReceiverKind::DynamicImpl,
                },
                trait_span,
                crate_idents: &attr.crate_idents,
                opts: &attr.opts,
            }
            .analyze(input_fn.input_sig(), &mut generics_analyzer)
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let trait_generics = generics_analyzer.into_trait_generics();

    let fn_input_mode = crate::input::FnInputMode::Module(&input_mod.ident);
    let trait_dependency_mode =
        detect_trait_dependency_mode(&fn_input_mode, &trait_fns, &attr.crate_idents, trait_span)?;
    let use_associated_future = generics::detect_use_associated_future(
        &attr.opts,
        input_mod.items.iter().filter_map(ModItem::filter_pub_fn),
    );
    let impl_indirection = match impl_kind {
        ImplKind::Static => generics::ImplIndirection::Static { ty: impl_ty },
        ImplKind::Dyn => generics::ImplIndirection::Dynamic { ty: impl_ty },
    };

    let impl_block = fn_delegation_codegen::FnDelegationCodegen {
        opts: &attr.opts,
        crate_idents: &attr.crate_idents,
        trait_ref: &derive_impl.trait_path.0,
        trait_span,
        impl_indirection,
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
    } = &input_mod;

    Ok(quote! {
        #(#attrs)*
        #vis #mod_token #mod_ident {
            #(#items)*
            #impl_block
        }
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
        format!("Module {mod_ident} contains no `#[derive_impl(Trait)] pub struct SomeStruct;`. Without this, it will be impossible to delegate to this implementation"),
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
