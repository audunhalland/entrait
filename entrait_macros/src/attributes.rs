use crate::analyze_generics::TraitFn;
use crate::generics::{self, TraitIndirection};
use crate::idents::CrateIdents;
use crate::input::FnInputMode;
use crate::opt::{AsyncStrategy, MockApiIdent, Opts, SpanOpt};
use crate::token_util::{comma_sep, push_tokens};

use proc_macro2::{Span, TokenStream};
use quote::ToTokens;

pub struct Attr<P>(pub P);

pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl<P: ToTokens> ToTokens for Attr<P> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        push_tokens!(stream, syn::token::Pound::default());
        syn::token::Bracket::default().surround(stream, |stream| {
            push_tokens!(stream, self.0);
        });
    }
}

pub struct ExportGatedAttr<'a, P: ToTokens + IsEmpty> {
    pub params: P,
    pub opts: &'a Opts,
}

impl<'a, P: ToTokens + IsEmpty> ToTokens for ExportGatedAttr<'a, P> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        if self.params.is_empty() {
            return;
        }
        push_tokens!(stream, syn::token::Pound::default());
        syn::token::Bracket::default().surround(stream, |stream| {
            if self.opts.export_value() {
                push_tokens!(stream, self.params);
            } else {
                push_tokens!(stream, syn::Ident::new("cfg_attr", Span::call_site()));
                syn::token::Paren::default().surround(stream, |stream| {
                    push_tokens!(
                        stream,
                        syn::Ident::new("test", Span::call_site()),
                        syn::token::Comma::default(),
                        self.params
                    );
                });
            }
        });
    }
}

pub struct EntraitForTraitParams<'a> {
    pub crate_idents: &'a CrateIdents,
}

impl<'a> ToTokens for EntraitForTraitParams<'a> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        use syn::token::*;
        use syn::Ident;

        push_tokens!(
            stream,
            PathSep::default(),
            self.crate_idents.entrait,
            PathSep::default(),
            self.crate_idents.entrait
        );
        Paren::default().surround(stream, |stream| {
            push_tokens!(
                stream,
                Ident::new("unimock", Span::call_site()),
                Eq::default(),
                syn::LitBool::new(false, Span::call_site()),
                Comma::default(),
                Ident::new("mockall", Span::call_site()),
                Eq::default(),
                syn::LitBool::new(false, Span::call_site())
            );
        });
    }
}

pub struct UnimockAttrParams<'s> {
    pub trait_ident: &'s syn::Ident,
    pub mock_api: Option<&'s MockApiIdent>,
    pub trait_indirection: TraitIndirection,
    pub crate_idents: &'s CrateIdents,
    pub trait_fns: &'s [TraitFn],
    pub(super) fn_input_mode: &'s FnInputMode<'s>,
    pub span: Span,
}

impl<'s> IsEmpty for UnimockAttrParams<'s> {
    fn is_empty(&self) -> bool {
        matches!(self.trait_indirection, TraitIndirection::Plain) && self.mock_api.is_none()
    }
}

impl<'s> ToTokens for UnimockAttrParams<'s> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        if self.is_empty() {
            return;
        }

        use syn::token::*;
        use syn::Ident;

        let span = self.span;

        push_tokens!(
            stream,
            PathSep(span),
            self.crate_idents.entrait,
            PathSep(span),
            self.crate_idents.__unimock,
            PathSep(span),
            self.crate_idents.unimock
        );

        Paren(span).surround(stream, |stream| {
            let mut punctuator = comma_sep(stream, span);

            // prefix=::entrait::__unimock
            punctuator.push_fn(|stream| {
                push_tokens!(
                    stream,
                    Ident::new("prefix", span),
                    Eq(span),
                    PathSep(span),
                    self.crate_idents.entrait,
                    PathSep(span),
                    self.crate_idents.__unimock
                );
            });

            if let Some(mock_api) = &self.mock_api {
                punctuator.push_fn(|stream| {
                    push_tokens!(stream, Ident::new("api", span), Eq(span));

                    // flatten=[TraitMock] for single-fn entraits
                    if matches!(self.fn_input_mode, FnInputMode::SingleFn(_)) {
                        Bracket(span).surround(stream, |stream| push_tokens!(stream, mock_api.0));
                    } else {
                        push_tokens!(stream, mock_api.0);
                    }
                });
            }

            if !matches!(self.fn_input_mode, FnInputMode::RawTrait(_)) {
                // unmock_with=[...]
                if !self.trait_fns.is_empty() {
                    punctuator.push_fn(|stream| {
                        self.unmock_with(stream);
                    });
                }
            }
        });
    }
}

impl<'s> UnimockAttrParams<'s> {
    fn unmock_with(&self, stream: &mut TokenStream) {
        use syn::token::*;
        use syn::Ident;

        let span = self.span;

        push_tokens!(stream, Ident::new("unmock_with", span), Eq(span));

        Bracket(span).surround(stream, |stream| {
            let mut punctuator = comma_sep(stream, span);

            for trait_fn in self.trait_fns {
                let fn_ident = &trait_fn.sig().ident;

                match &trait_fn.deps {
                    generics::FnDeps::Generic { .. } => {
                        punctuator.push(fn_ident);
                    }
                    generics::FnDeps::Concrete(_) => {
                        punctuator.push(Underscore(span));
                    }
                    generics::FnDeps::NoDeps { .. } => {
                        // fn_ident(a, b, c)
                        punctuator.push_fn(|stream| {
                            push_tokens!(stream, fn_ident);

                            Paren(span).surround(stream, |stream| {
                                let mut punctuator = comma_sep(stream, span);
                                for fn_arg in trait_fn.sig().inputs.iter() {
                                    if let syn::FnArg::Typed(pat_type) = fn_arg {
                                        if let syn::Pat::Ident(pat_ident) = pat_type.pat.as_ref() {
                                            punctuator.push(&pat_ident.ident);
                                        }
                                    }
                                }
                            });
                        });
                    }
                }
            }
        });
    }
}

pub struct MockallAutomockParams {
    pub span: Span,
}

impl IsEmpty for MockallAutomockParams {
    fn is_empty(&self) -> bool {
        false
    }
}

impl ToTokens for MockallAutomockParams {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let span = self.span;
        push_tokens!(
            stream,
            syn::token::PathSep(span),
            syn::Ident::new("mockall", span),
            syn::token::PathSep(span),
            syn::Ident::new("automock", span)
        );
    }
}

pub fn opt_async_trait_attr<'s, 'o>(
    opts: &'s Opts,
    crate_idents: &'s CrateIdents,
    trait_fns: impl Iterator<Item = &'o TraitFn>,
) -> Option<impl ToTokens + 's> {
    match (
        opts.async_strategy(),
        generics::has_any_async(trait_fns.map(|trait_fn| trait_fn.sig())),
    ) {
        (SpanOpt(AsyncStrategy::BoxFuture, span), true) => Some(Attr(AsyncTraitParams {
            crate_idents,
            use_static: false,
            span,
        })),
        (SpanOpt(AsyncStrategy::AssociatedFuture, span), true) => Some(Attr(AsyncTraitParams {
            crate_idents,
            use_static: true,
            span,
        })),
        _ => None,
    }
}

pub struct AsyncTraitParams<'a> {
    pub crate_idents: &'a CrateIdents,
    pub use_static: bool,
    pub span: Span,
}

impl<'a> ToTokens for AsyncTraitParams<'a> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let span = self.span;
        if self.use_static {
            push_tokens!(
                stream,
                syn::token::PathSep(span),
                self.crate_idents.entrait,
                syn::token::PathSep(span),
                syn::Ident::new("static_async", span),
                syn::token::PathSep(span),
                syn::Ident::new("async_trait", span)
            );
        } else {
            push_tokens!(
                stream,
                syn::token::PathSep(span),
                self.crate_idents.entrait,
                syn::token::PathSep(span),
                syn::Ident::new("__async_trait", span),
                syn::token::PathSep(span),
                syn::Ident::new("async_trait", span)
            );
        }
    }
}
