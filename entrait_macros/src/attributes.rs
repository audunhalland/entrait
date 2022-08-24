use crate::analyze_generics::TraitFn;
use crate::generics;
use crate::idents::CrateIdents;
use crate::input::FnInputMode;
use crate::opt::{AsyncStrategy, Opts, SpanOpt};
use crate::token_util::{comma_sep, push_tokens};

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, ToTokens};

pub struct Attr<P>(pub P);

impl<P: ToTokens> ToTokens for Attr<P> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        push_tokens!(stream, syn::token::Pound::default());
        syn::token::Bracket::default().surround(stream, |stream| {
            push_tokens!(stream, self.0);
        });
    }
}

pub struct ExportGatedAttr<'a, P: ToTokens> {
    pub params: P,
    pub opts: &'a Opts,
}

impl<'a, P: ToTokens> ToTokens for ExportGatedAttr<'a, P> {
    fn to_tokens(&self, stream: &mut TokenStream) {
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
            Colon2::default(),
            self.crate_idents.entrait,
            Colon2::default(),
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
    pub crate_idents: &'s CrateIdents,
    pub trait_fns: &'s [TraitFn],
    pub(super) fn_input_mode: &'s FnInputMode<'s>,
    pub span: Span,
}

impl<'s> ToTokens for UnimockAttrParams<'s> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        use syn::token::*;
        use syn::Ident;

        let span = self.span;

        push_tokens!(
            stream,
            Colon2(span),
            self.crate_idents.entrait,
            Colon2(span),
            self.crate_idents.__unimock,
            Colon2(span),
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
                    Colon2(span),
                    self.crate_idents.entrait,
                    Colon2(span),
                    self.crate_idents.__unimock
                );
            });

            if !matches!(self.fn_input_mode, FnInputMode::RawTrait(_)) {
                // mod=!
                if matches!(self.fn_input_mode, FnInputMode::SingleFn(_)) {
                    punctuator.push_fn(|stream| {
                        push_tokens!(stream, Mod(span), Eq(span), Bang(span));
                    });
                }

                /*
                if let FnInputMode::SingleFn(fn_ident) = &self.fn_input_mode {
                    push_tokens!(stream, Mod(span), Eq(span), fn_ident);
                } else {
                    push_tokens!(stream, Mod(span), Eq(span), Star(span));
                }
                punctuator.push_fn(|stream| {
                    if let FnInputMode::SingleFn(fn_ident) = &self.fn_input_mode {
                        push_tokens!(stream, Mod(span), Eq(span), fn_ident);
                    } else {
                        push_tokens!(stream, Mod(span), Eq(span), Star(span));
                    }
                });

                // as=Fn
                punctuator.push_fn(|stream| {
                    push_tokens!(
                        stream,
                        Ident::new("as", span),
                        Eq(span),
                        Ident::new("Fn", span)
                    );
                });

                // unmocked=[...]
                if !self.trait_fns.is_empty() {
                    punctuator.push_fn(|stream| {
                        self.unmocked(stream);
                    });
                }
                */
            }
        });
    }
}

pub struct UnimockMethodAttrParams<'s> {
    pub crate_idents: &'s CrateIdents,
    pub trait_ident: &'s syn::Ident,
    pub trait_fn: &'s TraitFn,
    pub(super) fn_input_mode: &'s FnInputMode<'s>,
    pub span: Span,
}

impl<'s> ToTokens for UnimockMethodAttrParams<'s> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        use syn::token::*;

        let span = self.span;

        push_tokens!(stream, self.crate_idents.unimock);

        Paren(span).surround(stream, |stream| {
            let mut punctuator = comma_sep(stream, span);

            // struct = MockTrait
            if matches!(self.fn_input_mode, FnInputMode::SingleFn(_)) {
                punctuator.push_fn(|stream| {
                    push_tokens!(
                        stream,
                        Struct(span),
                        Eq(span),
                        format_ident!("{}Mock", self.trait_ident)
                    );
                });
            }

            if !matches!(self.fn_input_mode, FnInputMode::RawTrait(_)) {
                // unmock_with=..
                match &self.trait_fn.deps {
                    generics::FnDeps::Generic { .. } | generics::FnDeps::NoDeps => {
                        punctuator.push_fn(|stream| {
                            self.unmock_with(stream);
                        });
                    }
                    generics::FnDeps::Concrete(_) => {}
                }
            }
        });
    }
}

impl<'s> UnimockMethodAttrParams<'s> {
    fn unmock_with(&self, stream: &mut TokenStream) {
        let span = self.span;

        use syn::token::*;
        use syn::Ident;

        push_tokens!(stream, Ident::new("unmock_with", span), Eq(span));

        let sig = self.trait_fn.sig();
        let fn_ident = &sig.ident;

        match &self.trait_fn.deps {
            generics::FnDeps::Generic { .. } => {
                push_tokens!(stream, fn_ident);
            }
            generics::FnDeps::Concrete(_) => {}
            generics::FnDeps::NoDeps { .. } => {
                // fn_ident(a, b, c)
                push_tokens!(stream, fn_ident);

                Paren(span).surround(stream, |stream| {
                    let mut punctuator = comma_sep(stream, span);
                    for fn_arg in sig.inputs.iter() {
                        if let syn::FnArg::Typed(pat_type) = fn_arg {
                            if let syn::Pat::Ident(pat_ident) = pat_type.pat.as_ref() {
                                punctuator.push(&pat_ident.ident);
                            }
                        }
                    }
                });
            }
        }
    }
}

pub struct MockallAutomockParams {
    pub span: Span,
}

impl ToTokens for MockallAutomockParams {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let span = self.span;
        push_tokens!(
            stream,
            syn::token::Colon2(span),
            syn::Ident::new("mockall", span),
            syn::token::Colon2(span),
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
        (SpanOpt(AsyncStrategy::AsyncTrait, span), true) => Some(Attr(AsyncTraitParams {
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
                syn::token::Colon2(span),
                self.crate_idents.entrait,
                syn::token::Colon2(span),
                syn::Ident::new("static_async", span),
                syn::token::Colon2(span),
                syn::Ident::new("async_trait", span)
            );
        } else {
            push_tokens!(
                stream,
                syn::token::Colon2(span),
                self.crate_idents.entrait,
                syn::token::Colon2(span),
                syn::Ident::new("__async_trait", span),
                syn::token::Colon2(span),
                syn::Ident::new("async_trait", span)
            );
        }
    }
}
