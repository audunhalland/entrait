use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};

use crate::{
    analyze_generics::TraitFn,
    attributes,
    generics::{self, TraitDependencyMode},
    idents::CrateIdents,
    impl_fn_codegen,
    input::FnInputMode,
    opt::{Opts, SpanOpt},
    token_util::push_tokens,
};

pub struct TraitCodegen<'s> {
    pub opts: &'s Opts,
    pub crate_idents: &'s CrateIdents,
}

pub enum Supertraits {
    None,
    Some {
        colon_token: syn::token::Colon,
        bounds: syn::punctuated::Punctuated<syn::TypeParamBound, syn::token::Add>,
    },
}

impl<'s> TraitCodegen<'s> {
    pub fn gen_trait_def(
        &self,
        visibility: &syn::Visibility,
        trait_ident: &syn::Ident,
        trait_generics: &generics::TraitGenerics,
        supertraits: &Supertraits,
        trait_dependency_mode: &TraitDependencyMode,
        trait_fns: &[TraitFn],
        fn_input_mode: &FnInputMode<'_>,
    ) -> syn::Result<TokenStream> {
        let span = trait_ident.span();

        let opt_unimock_attr = match self.opts.default_option(self.opts.unimock, false) {
            SpanOpt(true, span) => Some(attributes::ExportGatedAttr {
                params: attributes::UnimockAttrParams {
                    crate_idents: &self.crate_idents,
                    trait_fns,
                    fn_input_mode,
                    span,
                },
                opts: &self.opts,
            }),
            _ => None,
        };

        let opt_entrait_for_trait_attr = match trait_dependency_mode {
            TraitDependencyMode::Concrete(_) => {
                Some(attributes::Attr(attributes::EntraitForTraitParams {
                    crate_idents: &self.crate_idents,
                }))
            }
            _ => None,
        };

        let opt_mockall_automock_attr = match self.opts.default_option(self.opts.mockall, false) {
            SpanOpt(true, span) => Some(attributes::ExportGatedAttr {
                params: attributes::MockallAutomockParams { span },
                opts: &self.opts,
            }),
            _ => None,
        };
        let opt_async_trait_attr = impl_fn_codegen::opt_async_trait_attribute(
            &self.opts,
            &self.crate_idents,
            trait_fns.iter(),
        );

        let trait_visibility = TraitVisibility {
            visibility,
            fn_input_mode,
        };

        let fn_defs = trait_fns.iter().map(|trait_fn| {
            let opt_associated_fut_decl = &trait_fn.entrait_sig.associated_fut_decl;
            let trait_fn_sig = trait_fn.sig();

            quote! {
                #opt_associated_fut_decl
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
            #trait_visibility trait #trait_ident #params #where_clause {
                #(#fn_defs)*
            }
        })
    }
}

struct TraitVisibility<'a> {
    visibility: &'a syn::Visibility,
    fn_input_mode: &'a FnInputMode<'a>,
}

impl<'a> ToTokens for TraitVisibility<'a> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        match &self.fn_input_mode {
            FnInputMode::Module(_) => {
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
            FnInputMode::SingleFn(_) | FnInputMode::RawTrait => {
                push_tokens!(stream, self.visibility);
            }
        }
    }
}
