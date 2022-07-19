use quote::quote;
use syn::parse::{Parse, ParseStream};

pub struct DelegateImplInput;

impl Parse for DelegateImplInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if !input.is_empty() {
            return Err(syn::Error::new(input.span(), "No arguments expected"));
        }

        Ok(Self)
    }
}

pub fn gen_delegate_impl(item_trait: syn::ItemTrait) -> proc_macro::TokenStream {
    let tokens = quote! {
        #item_trait
    };

    tokens.into()
}
