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
    no_deps: Option<SpanOpt<bool>>,
    debug: Option<SpanOpt<bool>>,
    async_strategy: Option<SpanOpt<AsyncStrategy>>,

    /// Whether to export mocks (i.e. not gated with cfg(test))
    pub export: Option<SpanOpt<bool>>,

    /// Mocking with unimock
    pub unimock: Option<SpanOpt<bool>>,

    /// Mocking with mockall
    pub mockall: Option<SpanOpt<bool>>,
}

impl EntraitAttr {
    pub fn no_deps_value(&self) -> bool {
        self.default_option(self.no_deps, false).0
    }

    pub fn debug_value(&self) -> bool {
        self.default_option(self.debug, false).0
    }

    pub fn set_fallback_async_strategy(&mut self, strategy: AsyncStrategy) {
        self.async_strategy.get_or_insert(SpanOpt::of(strategy));
    }

    pub fn async_strategy(&self) -> SpanOpt<AsyncStrategy> {
        self.default_option(self.async_strategy, AsyncStrategy::NoHack)
    }

    pub fn export_value(&self) -> bool {
        self.default_option(self.export, false).0
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
    NoDeps(SpanOpt<bool>),
    Debug(SpanOpt<bool>),
    AsyncTrait(SpanOpt<bool>),
    AssociatedFuture(SpanOpt<bool>),
    /// Whether to export mocks
    Export(SpanOpt<bool>),
    /// Whether to generate unimock impl
    Unimock(SpanOpt<bool>),
    /// Whether to generate mockall impl
    Mockall(SpanOpt<bool>),
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

            match input.parse::<EntraitOpt>()? {
                EntraitOpt::NoDeps(opt) => no_deps = Some(opt),
                EntraitOpt::Debug(opt) => debug = Some(opt),
                EntraitOpt::AsyncTrait(opt) => {
                    async_strategy = Some(SpanOpt(AsyncStrategy::AsyncTrait, opt.1))
                }
                EntraitOpt::AssociatedFuture(opt) => {
                    async_strategy = Some(SpanOpt(AsyncStrategy::AssociatedFuture, opt.1))
                }
                EntraitOpt::Export(opt) => export = Some(opt),
                EntraitOpt::Unimock(opt) => unimock = Some(opt),
                EntraitOpt::Mockall(opt) => mockall = Some(opt),
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

impl Parse for EntraitOpt {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        let span = ident.span();
        let ident_string = ident.to_string();

        match ident_string.as_str() {
            "no_deps" => Ok(EntraitOpt::NoDeps(SpanOpt(
                parse_ext_bool_default_true(input)?,
                span,
            ))),
            "debug" => Ok(EntraitOpt::Debug(SpanOpt(
                parse_ext_bool_default_true(input)?,
                span,
            ))),
            "async_trait" => Ok(EntraitOpt::AsyncTrait(SpanOpt(
                parse_ext_bool_default_true(input)?,
                span,
            ))),
            "associated_future" => Ok(EntraitOpt::AssociatedFuture(SpanOpt(
                parse_ext_bool_default_true(input)?,
                span,
            ))),
            "export" => Ok(EntraitOpt::Export(SpanOpt(
                parse_ext_bool_default_true(input)?,
                span,
            ))),
            "unimock" => Ok(EntraitOpt::Unimock(SpanOpt(
                parse_ext_bool_default_true(input)?,
                span,
            ))),
            "mockall" => Ok(EntraitOpt::Mockall(SpanOpt(
                parse_ext_bool_default_true(input)?,
                span,
            ))),
            _ => Err(syn::Error::new(
                span,
                format!("Unkonwn entrait option \"{ident_string}\""),
            )),
        }
    }
}

fn parse_ext_bool_default_true(input: ParseStream) -> syn::Result<bool> {
    parse_ext_default_val(input, true, |b: syn::LitBool| b.value())
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
