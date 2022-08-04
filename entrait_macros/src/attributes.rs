use crate::analyze_generics::TraitFn;
use crate::generics;
use crate::idents::CrateIdents;
use crate::input::FnInputMode;
use crate::opt::Opts;
use crate::token_util::{comma_sep, push_tokens};

use proc_macro2::{Span, TokenStream};
use quote::ToTokens;

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

pub struct UnimockAttrParams<'s, 'i> {
    pub crate_idents: &'s CrateIdents,
    pub trait_fns: &'s [TraitFn<'i>],
    pub(super) mode: &'s FnInputMode<'s>,
    pub span: Span,
}

impl<'s, 'i> ToTokens for UnimockAttrParams<'s, 'i> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        use syn::token::*;
        use syn::Ident;

        let span = self.span;
        let __unimock_ident = Ident::new("__unimock", span);

        push_tokens!(
            stream,
            Colon2(span),
            self.crate_idents.entrait,
            Colon2(span),
            __unimock_ident,
            Colon2(span),
            Ident::new("unimock", span)
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
                    __unimock_ident
                );
            });

            // mod=?
            punctuator.push_fn(|stream| {
                if let FnInputMode::SingleFn(fn_ident) = &self.mode {
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
        });
    }
}

impl<'s, 'i> UnimockAttrParams<'s, 'i> {
    fn unmocked(&self, stream: &mut TokenStream) {
        use syn::token::*;
        use syn::Ident;

        let span = self.span;

        push_tokens!(stream, Ident::new("unmocked", span), Eq(span));

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

pub struct AsyncTraitParams<'a> {
    pub crate_idents: &'a CrateIdents,
    pub span: Span,
}

impl<'a> ToTokens for AsyncTraitParams<'a> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let span = self.span;
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
