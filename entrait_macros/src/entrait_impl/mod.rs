pub mod input_attr;

use crate::analyze_generics;
use crate::analyze_generics::detect_trait_dependency_mode;
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
        .unwrap_or(proc_macro2::Span::call_site());

    let mut generics_analyzer = analyze_generics::GenericsAnalyzer::new();
    let trait_fns = input_mod
        .items
        .iter()
        .filter_map(ModItem::filter_pub_fn)
        .enumerate()
        .map(|(index, input_fn)| {
            analyze_generics::TraitFn::analyze(
                input_fn,
                &mut generics_analyzer,
                signature::FnIndex(index),
                signature::InjectDynImplParam(matches!(kind, ImplKind::Dyn)),
                trait_span,
                &attr.opts,
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
        ImplKind::Static => generics::ImplIndirection::ImplRef {
            ref_lifetime: syn::Lifetime::new("'impl_life", trait_span),
        },
        ImplKind::Dyn => generics::ImplIndirection::DynCopy { ident: &impl_ident },
    };

    let impl_block = impl_fn_codegen::gen_impl_block(
        &attr.opts,
        &attr.crate_idents,
        &derive_impl.trait_path.0,
        trait_span,
        impl_indirection,
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
    } = &input_mod;

    Ok(match kind {
        ImplKind::Static => quote! {
            #(#attrs)*
            #vis #mod_token #mod_ident {
                #(#items)*

                const _: () = {
                    pub struct __ImplRef<'i, T>(&'i ::entrait::Impl<T>);

                    impl<'i, T> ::entrait::ImplRef<'i, T> for __ImplRef<'i, T> {
                        fn from_impl(_impl: &'i ::entrait::Impl<T>) -> Self {
                            Self(_impl)
                        }
                    }

                    impl<'i, T: 'static> ::entrait::BorrowImplRef<'i, T> for #impl_ident {
                        type Ref = __ImplRef<'i, T>;
                    }

                    impl<T: 'static> ::entrait::BorrowImpl<T> for #impl_ident {}

                    #impl_block
                };
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
