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
    pub no_deps: Option<Span>,
    pub debug: Option<Span>,
    pub async_trait: Option<Span>,
    pub associated_future: Option<Span>,

    /// Mocking in general: None implies Always. This is a way to _constrain_ mock support.
    pub mock: (FeatureCfg, Span),

    /// Mocking with unimock
    pub unimock: Option<(FeatureCfg, Span)>,

    /// Mocking with mockall
    pub mockall: Option<(FeatureCfg, Span)>,
}

///
/// "keyword args" to `entrait`.
///
pub enum Extension {
    NoDeps(Span),
    Debug(Span),
    AsyncTrait(Span),
    AssociatedFuture(Span),
    Mock(FeatureCfg, Span),
    Unimock(FeatureCfg, Span),
    Mockall(FeatureCfg, Span),
}

#[derive(Clone, Copy)]
pub enum FeatureCfg {
    Never,
    TestOnly,
    Always,
}

impl FeatureCfg {
    fn weight(self) -> u8 {
        match self {
            Self::Never => 0,
            Self::TestOnly => 1,
            Self::Always => 2,
        }
    }

    pub fn constrain(self, with: FeatureCfg) -> FeatureCfg {
        if self.weight() > with.weight() {
            with
        } else {
            self
        }
    }
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

        let mut no_deps = None;
        let mut debug = None;
        let mut async_trait = None;
        let mut associated_future = None;
        let mut mock = (FeatureCfg::Always, proc_macro2::Span::call_site());
        let mut unimock = None;
        let mut mockall = None;

        while input.peek(syn::token::Comma) {
            input.parse::<syn::token::Comma>()?;

            match input.parse::<Maybe<Extension>>()? {
                Maybe::Some(Extension::NoDeps(span)) => no_deps = Some(span),
                Maybe::Some(Extension::Debug(span)) => debug = Some(span),
                Maybe::Some(Extension::AsyncTrait(span)) => async_trait = Some(span),
                Maybe::Some(Extension::AssociatedFuture(span)) => associated_future = Some(span),
                Maybe::Some(Extension::Mock(cfg, span)) => mock = (cfg, span),
                Maybe::Some(Extension::Unimock(cfg, span)) => unimock = Some((cfg, span)),
                Maybe::Some(Extension::Mockall(cfg, span)) => mockall = Some((cfg, span)),
                _ => {}
            };
        }

        Ok(EntraitAttr {
            no_deps,
            trait_visibility,
            trait_ident,
            debug,
            async_trait,
            associated_future,
            mock,
            unimock,
            mockall,
        })
    }
}

impl Parse for Maybe<Extension> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        let span = ident.span();
        let ident_string = ident.to_string();

        match ident_string.as_str() {
            "no_deps" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(Extension::NoDeps(span))
                } else {
                    Maybe::None
                },
            ),
            "debug" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(Extension::Debug(span))
                } else {
                    Maybe::None
                },
            ),
            "async_trait" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(Extension::AsyncTrait(span))
                } else {
                    Maybe::None
                },
            ),
            "associated_future" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(Extension::AssociatedFuture(span))
                } else {
                    Maybe::None
                },
            ),
            "mock" => Ok(
                if let Maybe::Some(cfg) =
                    parse_ext_default_val(input, Maybe::Some(FeatureCfg::Always), |v| v)?
                {
                    Maybe::Some(Extension::Mock(cfg, span))
                } else {
                    Maybe::Some(Extension::Mock(
                        FeatureCfg::Always,
                        proc_macro2::Span::call_site(),
                    ))
                },
            ),
            "unimock" => Ok(
                if let Maybe::Some(cfg) =
                    parse_ext_default_val(input, Maybe::Some(FeatureCfg::Always), |v| v)?
                {
                    Maybe::Some(Extension::Unimock(cfg, span))
                } else {
                    Maybe::None
                },
            ),
            "test_unimock" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(Extension::Unimock(FeatureCfg::TestOnly, span))
                } else {
                    Maybe::None
                },
            ),
            "mockall" => Ok(
                if let Maybe::Some(cfg) =
                    parse_ext_default_val(input, Maybe::Some(FeatureCfg::Always), |v| v)?
                {
                    Maybe::Some(Extension::Mockall(cfg, span))
                } else {
                    Maybe::None
                },
            ),
            "test_mockall" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(Extension::Mockall(FeatureCfg::TestOnly, span))
                } else {
                    Maybe::None
                },
            ),
            _ => Err(syn::Error::new(
                span,
                format!("Unkonwn entrait extension \"{ident_string}\""),
            )),
        }
    }
}

fn parse_ext_default_val<V, F, O>(input: ParseStream, default_value: O, mapper: F) -> syn::Result<O>
where
    V: syn::parse::Parse,
    F: FnOnce(V) -> O,
{
    if !input.peek(syn::token::Eq) {
        return Ok(default_value);
    }

    input.parse::<syn::token::Eq>()?;

    let parsed = input.parse::<V>()?;

    Ok(mapper(parsed))
}

impl Parse for Maybe<FeatureCfg> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(syn::Ident) {
            let ident: syn::Ident = input.parse()?;
            match ident.to_string().as_ref() {
                "test" => Ok(Maybe::Some(FeatureCfg::TestOnly)),
                _ => Err(syn::Error::new(ident.span(), "unrecognized keyword")),
            }
        } else {
            if input.parse::<syn::LitBool>()?.value() {
                Ok(Maybe::Some(FeatureCfg::Always))
            } else {
                Ok(Maybe::Some(FeatureCfg::Never))
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
