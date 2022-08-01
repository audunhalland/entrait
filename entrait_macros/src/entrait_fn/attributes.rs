use super::{attr::EntraitFnAttr, Mode, TraitFn};
use crate::{
    generics,
    token_util::{comma_sep, push_tokens},
};

use proc_macro2::{Span, TokenStream};
use quote::ToTokens;

pub struct Attr<P>(pub P);

impl<P: ToTokens> ToTokens for Attr<P> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        push_tokens!(stream, syn::token::Pound::default());
        syn::token::Bracket::default().surround(stream, |stream| {
            self.0.to_tokens(stream);
        });
    }
}

pub struct ExportGatedAttr<'a, P: ToTokens> {
    pub params: P,
    pub attr: &'a EntraitFnAttr,
}

impl<'a, P: ToTokens> ToTokens for ExportGatedAttr<'a, P> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        push_tokens!(stream, syn::token::Pound::default());
        syn::token::Bracket::default().surround(stream, |stream| {
            if self.attr.export_value() {
                self.params.to_tokens(stream);
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

pub struct EntraitForTraitParams;

impl ToTokens for EntraitForTraitParams {
    fn to_tokens(&self, stream: &mut TokenStream) {
        use syn::token::*;
        use syn::Ident;

        let entrait = Ident::new("entrait", Span::call_site());

        push_tokens!(
            stream,
            Colon2::default(),
            entrait,
            Colon2::default(),
            entrait
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
    pub trait_fns: &'s [TraitFn<'i>],
    pub(super) mode: &'s Mode<'s>,
    pub span: Span,
}

impl<'s, 'i> ToTokens for UnimockAttrParams<'s, 'i> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        use syn::token::*;
        use syn::Ident;

        let span = self.span;
        let entrait_ident = Ident::new("entrait", span);
        let __unimock_ident = Ident::new("__unimock", span);

        push_tokens!(
            stream,
            Colon2(span),
            entrait_ident,
            Colon2(span),
            __unimock_ident,
            Colon2(span),
            Ident::new("unimock", span) // ::entrait::__unimock::unimock(prefix=::entrait::__unimock #mock_mod #opt_unmocked)
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
                    entrait_ident,
                    Colon2(span),
                    __unimock_ident
                );
            });

            // mod=?
            punctuator.push_fn(|stream| {
                if let Mode::SingleFn(fn_ident) = &self.mode {
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
                                    match fn_arg {
                                        syn::FnArg::Receiver(_) => {}
                                        syn::FnArg::Typed(pat_type) => {
                                            match pat_type.pat.as_ref() {
                                                syn::Pat::Ident(pat_ident) => {
                                                    punctuator.push(&pat_ident.ident);
                                                }
                                                _ => {}
                                            }
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

pub struct AsyncTraitParams {
    pub span: Span,
}

impl ToTokens for AsyncTraitParams {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let span = self.span;
        push_tokens!(
            stream,
            syn::token::Colon2(span),
            syn::Ident::new("entrait", span),
            syn::token::Colon2(span),
            syn::Ident::new("__async_trait", span),
            syn::token::Colon2(span),
            syn::Ident::new("async_trait", span)
        );
    }
}
