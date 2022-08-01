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

impl EntraitFnAttr {
    pub fn no_deps_value(&self) -> bool {
        self.default_option(self.opts.no_deps, false).0
    }

    pub fn debug_value(&self) -> bool {
        self.default_option(self.opts.debug, false).0
    }

    pub fn async_strategy(&self) -> SpanOpt<AsyncStrategy> {
        self.default_option(self.opts.async_strategy, AsyncStrategy::NoHack)
    }

    pub fn export_value(&self) -> bool {
        self.default_option(self.opts.export, false).0
    }

    pub fn default_option<T>(&self, option: Option<SpanOpt<T>>, default: T) -> SpanOpt<T> {
        match option {
            Some(option) => option,
            None => SpanOpt(default, self.trait_ident.span()),
        }
    }
}

impl Parse for EntraitFnAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();
        let trait_visibility: syn::Visibility = input.parse()?;

        let trait_ident = input.parse()?;

        let mut no_deps = None;
        let mut debug = None;
        let mut async_strategy = None;
        let mut export = None;
        let mut unimock = None;
        let mut mockall = None;

        while input.peek(syn::token::Comma) {
            input.parse::<syn::token::Comma>()?;

            match input.parse::<EntraitOpt>()? {
                EntraitOpt::NoDeps(opt) => no_deps = Some(opt),
                EntraitOpt::Debug(opt) => debug = Some(opt),
                EntraitOpt::AsyncTrait(opt) => {
                    async_strategy = Some(SpanOpt(AsyncStrategy::AsyncTrait, opt.1))
                }
                EntraitOpt::AssociatedFuture(opt) => {
                    async_strategy = Some(SpanOpt(AsyncStrategy::AssociatedFuture, opt.1))
                }
                EntraitOpt::Export(opt) => export = Some(opt),
                EntraitOpt::Unimock(opt) => unimock = Some(opt),
                EntraitOpt::Mockall(opt) => mockall = Some(opt),
                opt => return Err(syn::Error::new(opt.span(), "Unsupported option")),
            };
        }

        Ok(EntraitFnAttr {
            trait_visibility,
            trait_ident,
            opts: Opts {
                no_deps,
                debug,
                async_strategy,
                export,
                unimock,
                mockall,
            },
            crate_idents: CrateIdents::new(span),
        })
    }
}
