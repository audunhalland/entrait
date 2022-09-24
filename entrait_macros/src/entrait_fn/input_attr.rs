use crate::idents::CrateIdents;
use crate::opt::*;

use syn::parse::{Parse, ParseStream};

/// The `entrait` invocation for functions
pub struct EntraitFnAttr {
    pub trait_visibility: syn::Visibility,
    pub trait_ident: syn::Ident,
    pub opts: Opts,

    pub crate_idents: CrateIdents,
}

impl Parse for EntraitFnAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();
        let trait_visibility: syn::Visibility = input.parse()?;

        let trait_ident: syn::Ident = input.parse()?;

        let mut no_deps = None;
        let mut debug = None;
        let mut async_strategy = None;
        let mut export = None;
        let mut mock_api = None;
        let mut unimock = None;
        let mut mockall = None;

        while input.peek(syn::token::Comma) {
            input.parse::<syn::token::Comma>()?;

            match input.parse::<EntraitOpt>()? {
                EntraitOpt::NoDeps(opt) => no_deps = Some(opt),
                EntraitOpt::Debug(opt) => debug = Some(opt),
                EntraitOpt::BoxFuture(opt) => {
                    async_strategy = Some(SpanOpt(AsyncStrategy::BoxFuture, opt.1))
                }
                EntraitOpt::AssociatedFuture(opt) => {
                    async_strategy = Some(SpanOpt(AsyncStrategy::AssociatedFuture, opt.1))
                }
                EntraitOpt::Export(opt) => export = Some(opt),
                EntraitOpt::MockApi(ident) => mock_api = Some(ident),
                EntraitOpt::Unimock(opt) => unimock = Some(opt),
                EntraitOpt::Mockall(opt) => mockall = Some(opt),
                opt => return Err(syn::Error::new(opt.span(), "Unsupported option")),
            };
        }

        let default_span = trait_ident.span();

        Ok(EntraitFnAttr {
            trait_visibility,
            trait_ident,
            opts: Opts {
                default_span,
                no_deps,
                debug,
                async_strategy,
                export,
                mock_api,
                unimock,
                mockall,
            },
            crate_idents: CrateIdents::new(span),
        })
    }
}
