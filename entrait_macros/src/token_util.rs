use proc_macro2::TokenStream;
use quote::ToTokens;

macro_rules! push_tokens {
    ($stream:expr, $token:expr) => {
        $token.to_tokens($stream)
    };
    ($stream:expr, $token:expr, $($rest:expr),+) => {
        $token.to_tokens($stream);
        push_tokens!($stream, $($rest),*)
    };
}

pub(crate) use push_tokens;

pub struct TokenPair<T, U>(pub T, pub U);

impl<T: ToTokens, U: ToTokens> quote::ToTokens for TokenPair<T, U> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        push_tokens!(stream, self.0, self.1);
    }
}

pub struct EmptyToken;

impl quote::ToTokens for EmptyToken {
    fn to_tokens(&self, _: &mut TokenStream) {}
}

pub struct Punctuator<'s, S, P, E: ToTokens> {
    stream: &'s mut TokenStream,
    position: usize,
    start: S,
    punct: P,
    end: E,
}

pub fn comma_sep(
    stream: &mut TokenStream,
    span: proc_macro2::Span,
) -> Punctuator<EmptyToken, syn::token::Comma, EmptyToken> {
    Punctuator::new(stream, EmptyToken, syn::token::Comma(span), EmptyToken)
}

impl<'s, S, P, E> Punctuator<'s, S, P, E>
where
    S: quote::ToTokens,
    P: quote::ToTokens,
    E: quote::ToTokens,
{
    pub fn new(stream: &'s mut TokenStream, start: S, punct: P, end: E) -> Self {
        Self {
            stream,
            position: 0,
            start,
            punct,
            end,
        }
    }

    pub fn push<T: quote::ToTokens>(&mut self, tokens: T) {
        self.sep();
        tokens.to_tokens(self.stream);
    }

    pub fn push_fn<F>(&mut self, f: F)
    where
        F: FnOnce(&mut TokenStream),
    {
        self.sep();
        f(self.stream);
    }

    fn sep(&mut self) {
        if self.position == 0 {
            self.start.to_tokens(self.stream);
        } else {
            self.punct.to_tokens(self.stream);
        }

        self.position += 1;
    }
}

impl<'s, S, P, E> Drop for Punctuator<'s, S, P, E>
where
    E: quote::ToTokens,
{
    fn drop(&mut self) {
        if self.position > 0 {
            self.end.to_tokens(self.stream);
        }
    }
}
