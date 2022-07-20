use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};

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
    AsyncTrait(SpanOpt<bool>),
    AssociatedFuture(SpanOpt<bool>),
    /// Whether to export mocks
    Export(SpanOpt<bool>),
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
            Self::AsyncTrait(opt) => opt.1,
            Self::AssociatedFuture(opt) => opt.1,
            Self::Export(opt) => opt.1,
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

        match ident_string.as_str() {
            "no_deps" => Ok(EntraitOpt::NoDeps(SpanOpt(
                parse_eq_bool_or_true(input)?,
                span,
            ))),
            "debug" => Ok(EntraitOpt::Debug(SpanOpt(
                parse_eq_bool_or_true(input)?,
                span,
            ))),
            "async_trait" => Ok(EntraitOpt::AsyncTrait(SpanOpt(
                parse_eq_bool_or_true(input)?,
                span,
            ))),
            "associated_future" => Ok(EntraitOpt::AssociatedFuture(SpanOpt(
                parse_eq_bool_or_true(input)?,
                span,
            ))),
            "export" => Ok(EntraitOpt::Export(SpanOpt(
                parse_eq_bool_or_true(input)?,
                span,
            ))),
            "unimock" => Ok(EntraitOpt::Unimock(SpanOpt(
                parse_eq_bool_or_true(input)?,
                span,
            ))),
            "mockall" => Ok(EntraitOpt::Mockall(SpanOpt(
                parse_eq_bool_or_true(input)?,
                span,
            ))),
            _ => Err(syn::Error::new(
                span,
                format!("Unkonwn entrait option \"{ident_string}\""),
            )),
        }
    }
}

fn parse_eq_bool_or_true(input: ParseStream) -> syn::Result<bool> {
    parse_eq_value_or_default(input, true, |b: syn::LitBool| b.value())
}

fn parse_eq_value_or_default<V, F, O>(
    input: ParseStream,
    default_value: O,
    mapper: F,
) -> syn::Result<O>
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
