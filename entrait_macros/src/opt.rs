use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};

pub struct Opts {
    pub default_span: Span,

    pub no_deps: Option<SpanOpt<bool>>,
    pub debug: Option<SpanOpt<bool>>,
    pub async_strategy: Option<SpanOpt<AsyncStrategy>>,

    /// Whether to export mocks (i.e. not gated with cfg(test))
    pub export: Option<SpanOpt<bool>>,

    pub mock_api: Option<MockApiIdent>,

    /// Mocking with unimock
    pub unimock: Option<SpanOpt<bool>>,

    /// Mocking with mockall
    pub mockall: Option<SpanOpt<bool>>,
}

impl Opts {
    pub fn set_fallback_async_strategy(&mut self, strategy: AsyncStrategy) {
        self.async_strategy.get_or_insert(SpanOpt::of(strategy));
    }

    pub fn no_deps_value(&self) -> bool {
        self.default_option(self.no_deps, false).0
    }

    pub fn debug_value(&self) -> bool {
        self.default_option(self.debug, false).0
    }

    pub fn async_strategy(&self) -> SpanOpt<AsyncStrategy> {
        self.default_option(self.async_strategy, AsyncStrategy::NoHack)
    }

    pub fn export_value(&self) -> bool {
        self.default_option(self.export, false).0
    }

    pub fn mockable(&self) -> Mockable {
        if (self.unimock.is_some() && self.mock_api.is_some()) || self.mockall.is_some() {
            Mockable::Yes
        } else {
            Mockable::No
        }
    }

    pub fn default_option<T>(&self, option: Option<SpanOpt<T>>, default: T) -> SpanOpt<T> {
        match option {
            Some(option) => option,
            None => SpanOpt(default, self.default_span),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Mockable {
    Yes,
    No,
}

impl Mockable {
    pub fn yes(self) -> bool {
        matches!(self, Self::Yes)
    }
}

#[derive(Clone, Copy)]
pub enum AsyncStrategy {
    NoHack,
    BoxFuture,
    AssociatedFuture,
}

#[derive(Clone)]
#[allow(clippy::enum_variant_names)]
pub enum Delegate {
    BySelf,
    ByRef(RefDelegate),
    ByTrait(syn::Ident),
}

#[derive(Clone)]
pub enum RefDelegate {
    AsRef,
    Borrow,
}

#[derive(Copy, Clone)]
pub struct SpanOpt<T>(pub T, pub Span);

impl<T> SpanOpt<T> {
    pub fn of(value: T) -> Self {
        Self(value, proc_macro2::Span::call_site())
    }

    pub fn value(&self) -> &T {
        &self.0
    }
}

///
/// "keyword args" to `entrait`.
///
pub enum EntraitOpt {
    NoDeps(SpanOpt<bool>),
    Debug(SpanOpt<bool>),
    BoxFuture(SpanOpt<bool>),
    AssociatedFuture(SpanOpt<bool>),
    DelegateBy(SpanOpt<Delegate>),
    /// Whether to export mocks
    Export(SpanOpt<bool>),
    /// How to name the mock API
    MockApi(MockApiIdent),
    /// Whether to generate unimock impl
    Unimock(SpanOpt<bool>),
    /// Whether to generate mockall impl
    Mockall(SpanOpt<bool>),
}

impl EntraitOpt {
    pub fn span(&self) -> proc_macro2::Span {
        match self {
            Self::NoDeps(opt) => opt.1,
            Self::Debug(opt) => opt.1,
            Self::BoxFuture(opt) => opt.1,
            Self::AssociatedFuture(opt) => opt.1,
            Self::DelegateBy(opt) => opt.1,
            Self::Export(opt) => opt.1,
            Self::MockApi(ident) => ident.0.span(),
            Self::Unimock(opt) => opt.1,
            Self::Mockall(opt) => opt.1,
        }
    }
}

impl Parse for EntraitOpt {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        let span = ident.span();
        let ident_string = ident.to_string();

        use EntraitOpt::*;

        match ident_string.as_str() {
            "no_deps" => Ok(NoDeps(parse_eq_bool(input, true, span)?)),
            "debug" => Ok(Debug(parse_eq_bool(input, true, span)?)),
            "box_future" => Ok(BoxFuture(parse_eq_bool(input, true, span)?)),
            "associated_future" => Ok(AssociatedFuture(parse_eq_bool(input, true, span)?)),
            "delegate_by" => Ok(DelegateBy(parse_eq_delegate_by(
                input,
                Delegate::BySelf,
                span,
            )?)),
            "export" => Ok(Export(parse_eq_bool(input, true, span)?)),
            "mock_api" => {
                let _: syn::token::Eq = input.parse()?;
                Ok(Self::MockApi(MockApiIdent(input.parse()?)))
            }
            "unimock" => Ok(Unimock(parse_eq_bool(input, true, span)?)),
            "mockall" => Ok(Mockall(parse_eq_bool(input, true, span)?)),
            _ => Err(syn::Error::new(
                span,
                format!("Unkonwn entrait option \"{ident_string}\""),
            )),
        }
    }
}

pub struct MockApiIdent(pub syn::Ident);

fn parse_eq_bool(input: ParseStream, default: bool, span: Span) -> syn::Result<SpanOpt<bool>> {
    parse_eq_value_or_default(input, default, |b: syn::LitBool| Ok(b.value()), span)
}

fn parse_eq_delegate_by(
    input: ParseStream,
    default: Delegate,
    span: Span,
) -> syn::Result<SpanOpt<Delegate>> {
    if !input.peek(syn::token::Eq) {
        return Ok(SpanOpt(default, span));
    }

    input.parse::<syn::token::Eq>()?;

    if input.peek(syn::token::Ref) {
        let _: syn::token::Ref = input.parse()?;

        return Ok(SpanOpt(Delegate::ByRef(RefDelegate::AsRef), span));
    }

    let ident = input.parse::<syn::Ident>()?;

    Ok(SpanOpt(
        match ident.to_string().as_str() {
            "Self" => Delegate::BySelf,
            "Borrow" => Delegate::ByRef(RefDelegate::Borrow),
            _ => Delegate::ByTrait(ident),
        },
        span,
    ))
}

fn parse_eq_value_or_default<V, F, O>(
    input: ParseStream,
    default_value: O,
    mapper: F,
    span: Span,
) -> syn::Result<SpanOpt<O>>
where
    V: syn::parse::Parse,
    F: FnOnce(V) -> syn::Result<O>,
{
    if !input.peek(syn::token::Eq) {
        return Ok(SpanOpt(default_value, span));
    }

    input.parse::<syn::token::Eq>()?;

    let parsed = input.parse::<V>()?;

    Ok(SpanOpt(mapper(parsed)?, span))
}
