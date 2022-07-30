//!
//! inputs to procedural macros
//!
//!

use syn::parse::{Parse, ParseStream};

pub enum Input {
    Fn(InputFn),
    Trait(syn::ItemTrait),
    Mod(InputMod),
}

pub struct InputFn {
    pub fn_attrs: Vec<syn::Attribute>,
    pub fn_vis: syn::Visibility,
    pub fn_sig: syn::Signature,
    // don't try to parse fn_body, just pass through the tokens:
    pub fn_body: proc_macro2::TokenStream,
}

pub struct InputMod {
    attrs: Vec<syn::Attribute>,
    vis: syn::Visibility,
    mod_token: syn::token::Mod,
    ident: syn::Ident,
    brace_token: syn::token::Brace,
    items: Vec<InputModItem>,
}

pub enum InputModItem {
    Fn(InputFn),
    Verbatim(proc_macro2::TokenStream),
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
        } else if input.peek(syn::token::Mod) {
            let mod_token = input.parse()?;
            let ident = input.parse()?;

            let lookahead = input.lookahead1();
            if lookahead.peek(syn::token::Brace) {
                let content;
                let brace_token = syn::braced!(content in input);

                let items = vec![];

                Ok(Input::Mod(InputMod {
                    attrs,
                    vis,
                    mod_token,
                    ident,
                    brace_token,
                    items,
                }))
            } else {
                Err(lookahead.error())
            }
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
