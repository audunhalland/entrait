use crate::idents::CrateIdents;
use crate::opt::*;

use syn::parse::{Parse, ParseStream};

// Input of #[entrait(ref|dyn?)] impl A for B {}
pub struct EntraitSimpleImplAttr {
    pub impl_kind: ImplKind,
    pub opts: Opts,
    pub crate_idents: CrateIdents,
}

#[derive(Clone, Copy)]
pub enum ImplKind {
    Static,
    DynRef,
}

impl Parse for EntraitSimpleImplAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();

        let ref_token: Option<syn::token::Ref> = input.parse()?;
        let dyn_token: Option<syn::token::Dyn> = input.parse()?;

        let mut debug = None;

        if !input.is_empty() {
            loop {
                match input.parse::<EntraitOpt>()? {
                    EntraitOpt::Debug(opt) => debug = Some(opt),
                    entrait_opt => {
                        return Err(syn::Error::new(entrait_opt.span(), "Unsupported option"))
                    }
                };

                if input.peek(syn::token::Comma) {
                    input.parse::<syn::token::Comma>()?;
                } else {
                    break;
                }
            }
        }

        Ok(Self {
            impl_kind: if dyn_token.is_some() || ref_token.is_some() {
                ImplKind::DynRef
            } else {
                ImplKind::Static
            },
            opts: Opts {
                default_span: span,
                no_deps: None,
                debug,
                async_strategy: None,
                export: None,
                mock_api: None,
                unimock: None,
                mockall: None,
            },
            crate_idents: CrateIdents::new(span),
        })
    }
}

pub struct EntraitImplAttr {
    pub opts: Opts,
    pub crate_idents: CrateIdents,
}

impl Parse for EntraitImplAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();

        let mut debug = None;

        if !input.is_empty() {
            loop {
                match input.parse::<EntraitOpt>()? {
                    EntraitOpt::Debug(opt) => debug = Some(opt),
                    entrait_opt => {
                        return Err(syn::Error::new(entrait_opt.span(), "Unsupported option"))
                    }
                };

                if input.peek(syn::token::Comma) {
                    input.parse::<syn::token::Comma>()?;
                } else {
                    break;
                }
            }
        }

        Ok(Self {
            opts: Opts {
                default_span: span,
                no_deps: None,
                debug,
                async_strategy: None,
                export: None,
                mock_api: None,
                unimock: None,
                mockall: None,
            },
            crate_idents: CrateIdents::new(span),
        })
    }
}
