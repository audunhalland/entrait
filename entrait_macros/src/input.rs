//!
//! inputs to procedural macros
//!

use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};

///
/// The `entrait` invocation
///
pub struct EntraitAttr {
    pub trait_visibility: syn::Visibility,
    pub trait_ident: syn::Ident,
    pub impl_target_type: Option<syn::Type>,
    pub debug: Option<Span>,
    pub async_trait: Option<Span>,
    pub unimock: Option<(EnabledValue, Span)>,
    pub disable_unmock: Option<Span>,
    pub mockall: Option<(EnabledValue, Span)>,
}

///
/// "keyword args" to `entrait`.
///
pub enum Extension {
    Debug(Span),
    AsyncTrait(Span),
    Unimock(EnabledValue, Span),
    DisableUnmock(Span),
    Mockall(EnabledValue, Span),
}

pub enum EnabledValue {
    Always,
    TestOnly,
}

enum Maybe<T> {
    Some(T),
    None,
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

impl Parse for EntraitAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let trait_visibility: syn::Visibility = input.parse()?;

        let trait_ident = input.parse()?;

        let impl_target_type = if input.peek(syn::token::For) {
            input.parse::<syn::token::For>()?;
            Some(input.parse()?)
        } else {
            None
        };

        let mut debug = None;
        let mut async_trait = None;
        let mut unimock = None;
        let mut disable_unmock = None;
        let mut mockall = None;

        while input.peek(syn::token::Comma) {
            input.parse::<syn::token::Comma>()?;

            match input.parse::<Maybe<Extension>>()? {
                Maybe::Some(Extension::Debug(span)) => debug = Some(span),
                Maybe::Some(Extension::AsyncTrait(span)) => async_trait = Some(span),
                Maybe::Some(Extension::Unimock(enabled, span)) => unimock = Some((enabled, span)),
                Maybe::Some(Extension::DisableUnmock(span)) => disable_unmock = Some(span),
                Maybe::Some(Extension::Mockall(enabled, span)) => mockall = Some((enabled, span)),
                _ => {}
            };
        }

        Ok(EntraitAttr {
            trait_visibility,
            trait_ident,
            impl_target_type,
            debug,
            async_trait,
            unimock,
            disable_unmock,
            mockall,
        })
    }
}

impl Parse for Maybe<Extension> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        let span = ident.span();
        let ident_string = ident.to_string();

        input.parse::<syn::token::Eq>()?;

        match ident_string.as_str() {
            "debug" => Ok(if input.parse::<syn::LitBool>()?.value() {
                Maybe::Some(Extension::Debug(span))
            } else {
                Maybe::None
            }),
            "async_trait" => Ok(if input.parse::<syn::LitBool>()?.value() {
                Maybe::Some(Extension::AsyncTrait(span))
            } else {
                Maybe::None
            }),
            "unimock" => Ok(if let Maybe::Some(enabled) = input.parse()? {
                Maybe::Some(Extension::Unimock(enabled, span))
            } else {
                Maybe::None
            }),
            "test_unimock" => Ok(if input.parse::<syn::LitBool>()?.value {
                Maybe::Some(Extension::Unimock(EnabledValue::TestOnly, span))
            } else {
                Maybe::None
            }),
            "unmock" => Ok(if input.parse::<syn::LitBool>()?.value {
                Maybe::None
            } else {
                Maybe::Some(Extension::DisableUnmock(span))
            }),
            "mockall" => Ok(if let Maybe::Some(enabled) = input.parse()? {
                Maybe::Some(Extension::Mockall(enabled, span))
            } else {
                Maybe::None
            }),
            "test_mockall" => Ok(if input.parse::<syn::LitBool>()?.value {
                Maybe::Some(Extension::Mockall(EnabledValue::TestOnly, span))
            } else {
                Maybe::None
            }),
            _ => Err(syn::Error::new(
                span,
                format!("Unkonwn entrait extension \"{ident_string}\""),
            )),
        }
    }
}

impl Parse for Maybe<EnabledValue> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(syn::Ident) {
            let ident: syn::Ident = input.parse()?;
            match ident.to_string().as_ref() {
                "test" => Ok(Maybe::Some(EnabledValue::TestOnly)),
                _ => Err(syn::Error::new(ident.span(), "unrecognized keyword")),
            }
        } else {
            if input.parse::<syn::LitBool>()?.value() {
                Ok(Maybe::Some(EnabledValue::Always))
            } else {
                Ok(Maybe::None)
            }
        }
    }
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
