use crate::idents::CrateIdents;
use crate::opt::*;

use syn::parse::{Parse, ParseStream};

pub struct EntraitTraitAttr {
    pub delegation_trait: Option<DelegationTrait>,
    pub opts: Opts,
    pub delegation_kind: Option<SpanOpt<DelegationKind>>,
    pub crate_idents: CrateIdents,
}

pub struct DelegationTrait {
    pub trait_visibility: syn::Visibility,
    pub trait_ident: syn::Ident,
}

impl Parse for EntraitTraitAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();

        let mut debug = None;
        let mut unimock = None;
        let mut mockall = None;
        let mut delegation_kind = None;

        if !input.is_empty() {
            loop {
                match input.parse::<EntraitOpt>()? {
                    EntraitOpt::Unimock(opt) => unimock = Some(opt),
                    EntraitOpt::Mockall(opt) => mockall = Some(opt),
                    EntraitOpt::Debug(opt) => debug = Some(opt),
                    EntraitOpt::DelegateBy(kind) => delegation_kind = Some(kind),
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
            delegation_trait: None,
            opts: Opts {
                no_deps: None,
                debug,
                async_strategy: None,
                export: None,
                unimock,
                mockall,
            },
            delegation_kind,
            crate_idents: CrateIdents::new(span),
        })
    }
}
