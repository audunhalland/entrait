pub mod input_attr;

use crate::analyze_generics;
use crate::input::{InputMod, ModItem};
use crate::signature;
use input_attr::EntraitImplAttr;

use quote::quote;
use syn::spanned::Spanned;

pub fn output_tokens(
    attr: EntraitImplAttr,
    input_mod: InputMod,
) -> syn::Result<proc_macro2::TokenStream> {
    let type_path = &attr.type_path;
    let trait_span = attr.trait_path.span();

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
                trait_span,
                &attr.opts,
            )
        })
        .collect::<syn::Result<Vec<_>>>()?;

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

            const _: () = {
                pub struct __ImplRef<'i, T>(&'i ::entrait::Impl<T>);

                impl<'i, T> ::entrait::ImplRef<'i, T> for __ImplRef<'i, T> {
                    fn from_impl(_impl: &'i ::entrait::Impl<T>) -> Self {
                        Self(_impl)
                    }
                    fn as_impl(&self) -> &'i ::entrait::Impl<T> {
                        self.0
                    }
                }

                impl<'i, T: 'static> ::entrait::BorrowImplRef<'i, T> for super::#type_path {
                    type Ref = __ImplRef<'i, T>;
                }

                impl<T: 'static> ::entrait::BorrowImpl<T> for super::#type_path {}

                /*
                impl<'i, T> Foo3 for MyImplRef<'i, T>
                where
                    Impl<T>: SomeDep,
                {
                    fn foo3(&self) -> i32 {
                        self.as_impl().bar()
                    }
                }
                */
            };
        }
    })
}
