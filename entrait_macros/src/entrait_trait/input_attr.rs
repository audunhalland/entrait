use crate::idents::CrateIdents;
use crate::opt::*;

use syn::parse::{Parse, ParseStream};

pub struct EntraitTraitAttr {
    pub impl_trait: Option<ImplTrait>,
    pub opts: Opts,
    pub delegation_kind: Option<SpanOpt<Delegate>>,
    pub crate_idents: CrateIdents,
}

pub struct ImplTrait(pub syn::Visibility, pub syn::Ident);

impl Parse for EntraitTraitAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();

        let mut impl_trait = None;

        if !input.is_empty() && input.fork().parse::<EntraitOpt>().is_err() {
            let vis: syn::Visibility = input.parse()?;
            let ident: syn::Ident = input.parse()?;

            impl_trait = Some(ImplTrait(vis, ident));

            if input.peek(syn::token::Comma) {
                input.parse::<syn::token::Comma>()?;
            }
        }

        let mut debug = None;
        let mut async_strategy = None;
        let mut mock_api = None;
        let mut unimock = None;
        let mut mockall = None;
        let mut delegation_kind = None;

        if !input.is_empty() {
            loop {
                match input.parse::<EntraitOpt>()? {
                    EntraitOpt::Debug(opt) => debug = Some(opt),
                    EntraitOpt::BoxFuture(opt) => {
                        async_strategy = Some(SpanOpt(AsyncStrategy::BoxFuture, opt.1))
                    }
                    EntraitOpt::AssociatedFuture(opt) => {
                        async_strategy = Some(SpanOpt(AsyncStrategy::AssociatedFuture, opt.1))
                    }
                    EntraitOpt::MockApi(ident) => mock_api = Some(ident),
                    EntraitOpt::Unimock(opt) => unimock = Some(opt),
                    EntraitOpt::Mockall(opt) => mockall = Some(opt),
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
            impl_trait,
            opts: Opts {
                default_span: proc_macro2::Span::call_site(),
                no_deps: None,
                debug,
                async_strategy,
                export: None,
                mock_api,
                unimock,
                mockall,
            },
            delegation_kind,
            crate_idents: CrateIdents::new(span),
        })
    }
}
