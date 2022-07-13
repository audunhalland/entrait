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
    pub no_deps: Option<SpanOpt<()>>,
    pub debug: Option<SpanOpt<()>>,
    pub async_strategy: Option<SpanOpt<AsyncStrategy>>,

    /// Whether to export mocks (i.e. not gated with cfg(test))
    pub export: Option<SpanOpt<bool>>,

    /// Mocking with unimock
    pub unimock: Option<SpanOpt<bool>>,

    /// Mocking with mockall
    pub mockall: Option<SpanOpt<bool>>,
}

impl EntraitAttr {
    pub fn set_fallback_async_strategy(&mut self, strategy: AsyncStrategy) {
        self.async_strategy.get_or_insert(SpanOpt::of(strategy));
    }

    pub fn get_async_strategy(&self) -> SpanOpt<AsyncStrategy> {
        self.default_option(self.async_strategy, AsyncStrategy::NoHack)
    }

    pub fn default_option<T>(&self, option: Option<SpanOpt<T>>, default: T) -> SpanOpt<T> {
        match option {
            Some(option) => option,
            None => SpanOpt(default, self.trait_ident.span()),
        }
    }
}

#[derive(Copy, Clone)]
pub struct SpanOpt<T>(pub T, pub Span);

impl<T> SpanOpt<T> {
    pub fn of(value: T) -> Self {
        Self(value, proc_macro2::Span::call_site())
    }
}

#[derive(Clone, Copy)]
pub enum AsyncStrategy {
    NoHack,
    AsyncTrait,
    AssociatedFuture,
}

///
/// "keyword args" to `entrait`.
///
pub enum EntraitOpt {
    NoDeps(SpanOpt<()>),
    Debug(SpanOpt<()>),
    AsyncTrait(SpanOpt<()>),
    AssociatedFuture(SpanOpt<()>),
    /// Whether to export mocks
    Export(SpanOpt<bool>),
    /// Whether to generate unimock impl
    Unimock(SpanOpt<bool>),
    /// Whether to generate mockall impl
    Mockall(SpanOpt<bool>),
}

#[derive(Clone, Copy)]
pub enum FeatureCfg {
    Never,
    TestOnly,
    Always,
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
        let mut async_strategy = None;
        let mut export = None;
        let mut unimock = None;
        let mut mockall = None;

        while input.peek(syn::token::Comma) {
            input.parse::<syn::token::Comma>()?;

            match input.parse::<Maybe<EntraitOpt>>()? {
                Maybe::Some(EntraitOpt::NoDeps(opt)) => no_deps = Some(opt),
                Maybe::Some(EntraitOpt::Debug(opt)) => debug = Some(opt),
                Maybe::Some(EntraitOpt::AsyncTrait(opt)) => async_strategy = Some(SpanOpt(AsyncStrategy::AsyncTrait, opt.1)),
                Maybe::Some(EntraitOpt::AssociatedFuture(opt)) => async_strategy = Some(SpanOpt(AsyncStrategy::AssociatedFuture, opt.1)),
                Maybe::Some(EntraitOpt::Export(opt)) => export = Some(opt),
                Maybe::Some(EntraitOpt::Unimock(opt)) => unimock = Some(opt),
                Maybe::Some(EntraitOpt::Mockall(opt)) => mockall = Some(opt),
                Maybe::None => {}
            };
        }

        Ok(EntraitAttr {
            no_deps,
            trait_visibility,
            trait_ident,
            debug,
            async_strategy,
            export,
            unimock,
            mockall,
        })
    }
}

impl Parse for Maybe<EntraitOpt> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        let span = ident.span();
        let ident_string = ident.to_string();

        match ident_string.as_str() {
            "no_deps" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(EntraitOpt::NoDeps(SpanOpt((), span)))
                } else {
                    Maybe::None
                },
            ),
            "debug" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(EntraitOpt::Debug(SpanOpt((), span)))
                } else {
                    Maybe::None
                },
            ),
            "async_trait" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(EntraitOpt::AsyncTrait(SpanOpt((), span)))
                } else {
                    Maybe::None
                },
            ),
            "associated_future" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(EntraitOpt::AssociatedFuture(SpanOpt((), span)))
                } else {
                    Maybe::None
                },
            ),
            "export" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(EntraitOpt::Export(SpanOpt(true, span)))
                } else {
                    Maybe::None
                },
            ),
            "unimock" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(EntraitOpt::Unimock(SpanOpt(true, span)))
                } else {
                    Maybe::None
                },
            ),
            "mockall" => Ok(
                if parse_ext_default_val(input, true, |b: syn::LitBool| b.value())? {
                    Maybe::Some(EntraitOpt::Mockall(SpanOpt(true, span)))
                } else {
                    Maybe::None
                },
            ),
            _ => Err(syn::Error::new(
                span,
                format!("Unkonwn entrait option \"{ident_string}\""),
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
