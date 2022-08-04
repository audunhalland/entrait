use crate::idents::CrateIdents;
use crate::opt::*;

use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;

pub struct EntraitImplAttr {
    pub dyn_token: Option<syn::token::Dyn>,
    pub trait_path: syn::Path,
    pub for_token: syn::token::For,
    pub type_path: syn::Path,
    pub opts: Opts,
    pub crate_idents: CrateIdents,
}

impl Parse for EntraitImplAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();

        let dyn_token = input.parse()?;
        let trait_path: syn::Path = input.parse()?;
        let for_token = input.parse()?;
        let type_path = input.parse()?;

        let default_span = trait_path.span();

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
            trait_path,
            for_token,
            type_path,
            opts: Opts {
                default_span,
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
