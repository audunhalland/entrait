pub mod input_attr;

use crate::analyze_generics;
use crate::analyze_generics::detect_trait_dependency_mode;
use crate::analyze_generics::TraitFnAnalyzer;
use crate::fn_delegation_codegen;
use crate::generics;
use crate::input::ImplItem;
use crate::input::InputImpl;
use crate::signature;
use crate::sub_attributes::analyze_sub_attributes;
use crate::sub_attributes::SubAttribute;

use quote::quote;
use syn::spanned::Spanned;

use self::input_attr::EntraitSimpleImplAttr;
use self::input_attr::ImplKind;

pub fn output_tokens_for_impl(
    attr: EntraitSimpleImplAttr,
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
                impl_receiver_kind: match attr.impl_kind {
                    ImplKind::Static => signature::ImplReceiverKind::StaticImpl,
                    ImplKind::DynRef => signature::ImplReceiverKind::DynamicImpl,
                },
                trait_span,
                crate_idents: &attr.crate_idents,
                opts: &attr.opts,
            }
            .analyze(input_fn.input_sig(), &mut generics_analyzer)
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let sub_attributes = analyze_sub_attributes(&attrs);

    let trait_generics = generics_analyzer.into_trait_generics();

    let fn_input_mode = crate::input::FnInputMode::ImplBlock(&self_ty);
    let trait_dependency_mode =
        detect_trait_dependency_mode(&fn_input_mode, &trait_fns, &attr.crate_idents, trait_span)?;

    let impl_indirection = match attr.impl_kind {
        ImplKind::Static => generics::ImplIndirection::Static { ty: &self_ty },
        ImplKind::DynRef => generics::ImplIndirection::Dynamic { ty: &self_ty },
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
        sub_attributes: &sub_attributes,
    }
    .gen_impl_block(&trait_fns);

    let inherent_sub_attrs = sub_attributes
        .iter()
        .filter(|sub_attr| !matches!(sub_attr, SubAttribute::AsyncTrait(_)));

    Ok(quote! {
        #(#inherent_sub_attrs)*
        #unsafety #impl_token #self_ty {
            #(#items)*
        }
        #impl_block
    })
}
