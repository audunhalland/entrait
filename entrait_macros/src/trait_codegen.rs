use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};

use crate::{
    analyze_generics::TraitFn,
    attributes,
    generics::{self, TraitDependencyMode, TraitIndirection},
    idents::CrateIdents,
    input::FnInputMode,
    opt::{Opts, SpanOpt},
    token_util::push_tokens,
};

pub struct TraitCodegen<'s> {
    pub opts: &'s Opts,
    pub crate_idents: &'s CrateIdents,
    pub trait_indirection: TraitIndirection,
    pub trait_dependency_mode: &'s TraitDependencyMode<'s, 's>,
}

impl<'s> TraitCodegen<'s> {
    pub fn gen_trait_def(
        &self,
        visibility: &syn::Visibility,
        trait_ident: &syn::Ident,
        trait_generics: &generics::TraitGenerics,
        supertraits: &Supertraits,
        trait_fns: &[TraitFn],
        fn_input_mode: &FnInputMode<'_>,
    ) -> syn::Result<TokenStream> {
        let span = trait_ident.span();

        let opt_unimock_attr = match self.opts.default_option(self.opts.unimock, false) {
            SpanOpt(true, span) => Some(attributes::ExportGatedAttr {
                params: attributes::UnimockAttrParams {
                    trait_ident,
                    mock_api: self.opts.mock_api.as_ref(),
                    trait_indirection: self.trait_indirection,
                    crate_idents: self.crate_idents,
                    trait_fns,
                    fn_input_mode,
                    span,
                },
                opts: self.opts,
            }),
            _ => None,
        };

        let opt_entrait_for_trait_attr = match self.trait_dependency_mode {
            TraitDependencyMode::Concrete(_) => {
                Some(attributes::Attr(attributes::EntraitForTraitParams {
                    crate_idents: self.crate_idents,
                }))
            }
            _ => None,
        };

        let opt_mockall_automock_attr = match self.opts.default_option(self.opts.mockall, false) {
            SpanOpt(true, span) => Some(attributes::ExportGatedAttr {
                params: attributes::MockallAutomockParams { span },
                opts: self.opts,
            }),
            _ => None,
        };
        let opt_async_trait_attr =
            attributes::opt_async_trait_attr(self.opts, self.crate_idents, trait_fns.iter());

        let literal_attrs = if let FnInputMode::RawTrait(literal_attrs) = fn_input_mode {
            Some(literal_attrs)
        } else {
            None
        };

        let trait_visibility = TraitVisibility {
            visibility,
            fn_input_mode,
        };

        let fn_defs = trait_fns.iter().map(|trait_fn| {
            let opt_associated_fut_decl = &trait_fn
                .entrait_sig
                .associated_fut_decl(self.trait_indirection, self.crate_idents);
            let attrs = &trait_fn.attrs;
            let trait_fn_sig = trait_fn.sig();

            quote! {
                #opt_associated_fut_decl
                #(#attrs)*
                #trait_fn_sig;
            }
        });

        let params = trait_generics.trait_params();
        let where_clause = trait_generics.trait_where_clause();

        Ok(quote_spanned! { span=>
            #opt_unimock_attr
            #opt_entrait_for_trait_attr
            #opt_mockall_automock_attr
            #opt_async_trait_attr
            #literal_attrs
            #trait_visibility trait #trait_ident #params #supertraits #where_clause {
                #(#fn_defs)*
            }
        })
    }
}

#[derive(Clone)]
pub enum Supertraits {
    None,
    Some {
        colon_token: syn::token::Colon,
        bounds: syn::punctuated::Punctuated<syn::TypeParamBound, syn::token::Plus>,
    },
}

impl ToTokens for Supertraits {
    fn to_tokens(&self, stream: &mut TokenStream) {
        if let Self::Some {
            colon_token,
            bounds,
        } = self
        {
            push_tokens!(stream, colon_token, bounds);
        }
    }
}

struct TraitVisibility<'a> {
    visibility: &'a syn::Visibility,
    fn_input_mode: &'a FnInputMode<'a>,
}

impl<'a> ToTokens for TraitVisibility<'a> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        match &self.fn_input_mode {
            FnInputMode::Module(_) | FnInputMode::ImplBlock(_) => {
                match &self.visibility {
                    syn::Visibility::Inherited => {
                        // When the trait is "private", it should only be accessible to the module outside,
                        // so use `pub(super)`.
                        // This is because the trait is syntacitally "defined" outside the module, because
                        // the attribute is an outer attribute.
                        // If proc-macros supported inner attributes, and this was invoked with that, we wouldn't do this.
                        push_tokens!(stream, syn::token::Pub(Span::call_site()));
                        syn::token::Paren::default().surround(stream, |stream| {
                            push_tokens!(stream, syn::token::Super::default());
                        });
                    }
                    _ => {
                        push_tokens!(stream, self.visibility);
                    }
                }
            }
            FnInputMode::SingleFn(_) | FnInputMode::RawTrait(_) => {
                push_tokens!(stream, self.visibility);
            }
        }
    }
}
