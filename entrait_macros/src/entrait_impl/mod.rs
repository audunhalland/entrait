pub mod input_attr;

use crate::input::InputMod;
use input_attr::EntraitImplAttr;

use quote::quote;

pub fn output_tokens(
    attr: EntraitImplAttr,
    input_mod: InputMod,
) -> syn::Result<proc_macro2::TokenStream> {
    let type_path = &attr.type_path;

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
