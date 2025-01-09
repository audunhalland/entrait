use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;

use crate::{
    analyze_generics::TraitFn,
    attributes,
    generics::{self, TraitDependencyMode, TraitIndirection},
    idents::CrateIdents,
    input::FnInputMode,
    opt::{Opts, SpanOpt},
    signature::EntraitSignature,
    sub_attributes::{contains_async_trait, SubAttribute},
    token_util::push_tokens,
};

pub struct TraitCodegen<'s> {
    pub opts: &'s Opts,
    pub crate_idents: &'s CrateIdents,
    pub trait_indirection: TraitIndirection,
    pub trait_dependency_mode: &'s TraitDependencyMode<'s, 's>,
    pub sub_attributes: &'s [SubAttribute<'s>],
}

impl TraitCodegen<'_> {
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
        let trait_visibility = TraitVisibility {
            visibility,
            fn_input_mode,
        };

        let fn_defs = trait_fns.iter().map(|trait_fn| {
            let attrs = &trait_fn.attrs;
            let trait_fn_sig =
                make_trait_fn_sig(&trait_fn.entrait_sig, self.sub_attributes, self.opts);

            quote! {
                #(#attrs)*
                #trait_fn_sig;
            }
        });

        let params = trait_generics.trait_params();
        let where_clause = trait_generics.trait_where_clause();

        let trait_sub_attributes = self.sub_attributes.iter().filter(|attr| {
            matches!(
                attr,
                SubAttribute::AsyncTrait(_) | SubAttribute::Automock(_)
            )
        });

        Ok(quote_spanned! { span=>
            #opt_unimock_attr
            #opt_entrait_for_trait_attr
            #opt_mockall_automock_attr
            #(#trait_sub_attributes)*
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

impl ToTokens for TraitVisibility<'_> {
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

fn make_trait_fn_sig(
    entrait_sig: &EntraitSignature,
    sub_attributes: &[SubAttribute],
    opts: &Opts,
) -> syn::Signature {
    let mut sig = entrait_sig.sig.clone();

    if entrait_sig.sig.asyncness.is_some() && !contains_async_trait(sub_attributes) {
        sig.asyncness = None;

        let mut return_type = syn::ReturnType::Default;
        std::mem::swap(&mut return_type, &mut sig.output);

        let span = return_type.span();

        let output_type: syn::Type = match return_type {
            syn::ReturnType::Default => syn::parse_quote! { () },
            syn::ReturnType::Type(_, ty) => *ty,
        };

        let mut bounds: Vec<proc_macro2::TokenStream> = vec![quote! {
            ::core::future::Future<Output = #output_type>
        }];

        if opts.future_send().0 {
            bounds.push(quote! {
                ::core::marker::Send
            });
        }

        sig.output = syn::parse_quote_spanned! {span=>
            -> impl #(#bounds)+*
        };
    }

    sig
}
