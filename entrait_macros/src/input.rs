//!
//! inputs to procedural macros
//!
//!

use syn::parse::{Parse, ParseStream};

pub enum Input {
    Fn(InputFn),
    Trait(syn::ItemTrait),
}

///
/// The "body" that is decorated with entrait.
///
pub struct InputFn {
    pub fn_attrs: Vec<syn::Attribute>,
    pub fn_vis: syn::Visibility,
    pub fn_sig: syn::Signature,
    // don't try to parse fn_body, just pass through the tokens:
    pub fn_body: proc_macro2::TokenStream,
}

impl Parse for InputFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fn_attrs = input.call(syn::Attribute::parse_outer)?;
        let fn_vis = input.parse()?;
        let fn_sig: syn::Signature = input.parse()?;
        let fn_body = input.parse()?;

        Ok(InputFn {
            fn_attrs,
            fn_vis,
            fn_sig,
            fn_body,
        })
    }
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse()?;

        // BUG (In theory): missing "unsafe" and "auto" traits
        if input.peek(syn::token::Trait) {
            let item_trait: syn::ItemTrait = input.parse()?;

            Ok(Input::Trait(syn::ItemTrait {
                attrs,
                vis,
                ..item_trait
            }))
        } else {
            let fn_sig: syn::Signature = input.parse()?;
            let fn_body = input.parse()?;

            Ok(Input::Fn(InputFn {
                fn_attrs: attrs,
                fn_vis: vis,
                fn_sig,
                fn_body,
            }))
        }
    }
}
