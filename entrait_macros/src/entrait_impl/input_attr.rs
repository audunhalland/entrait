use crate::idents::CrateIdents;
use crate::opt::*;

use syn::parse::{Parse, ParseStream};

// Input of #[entrait(dyn?)] impl A for B {}
pub struct EntraitSimpleImplAttr {
    pub dyn_token: Option<syn::token::Dyn>,
    pub opts: Opts,
    pub crate_idents: CrateIdents,
}

impl Parse for EntraitSimpleImplAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();

        let dyn_token = input.parse()?;

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
            dyn_token,
            opts: Opts {
                default_span: span,
                no_deps: None,
                debug,
                async_strategy: None,
                export: None,
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
                unimock: None,
                mockall: None,
            },
            crate_idents: CrateIdents::new(span),
        })
    }
}
