use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

use crate::analyze_generics::TraitFn;
use crate::generics;
use crate::generics::ImplIndirection;
use crate::generics::TraitDependencyMode;
use crate::idents::CrateIdents;
use crate::input::FnInputMode;
use crate::opt::Mockable;
use crate::opt::Opts;
use crate::sub_attributes::SubAttribute;
use crate::token_util::push_tokens;
use crate::token_util::TokenPair;

/// Generate impls that call standalone generic functions
pub struct FnDelegationCodegen<'s, TR> {
    pub opts: &'s Opts,
    #[expect(unused)]
    pub crate_idents: &'s CrateIdents,
    pub trait_ref: &'s TR,
    pub trait_span: Span,
    pub impl_indirection: ImplIndirection<'s>,
    pub trait_generics: &'s generics::TraitGenerics,
    pub fn_input_mode: &'s FnInputMode<'s>,
    pub trait_dependency_mode: &'s TraitDependencyMode<'s, 's>,
    pub sub_attributes: &'s [SubAttribute<'s>],
}

impl<TR: ToTokens> FnDelegationCodegen<'_, TR> {
    ///
    /// Generate code like
    ///
    /// ```no_compile
    /// impl<__T: ::entrait::Impl + Deps> Trait for __T {
    ///     fn the_func(&self, args...) {
    ///         the_func(self, args)
    ///     }
    /// }
    /// ```
    ///
    pub fn gen_impl_block(&self, trait_fns: &[TraitFn]) -> TokenStream {
        let params = self.trait_generics.impl_params(
            self.trait_dependency_mode,
            generics::has_any_self_by_value(trait_fns.iter().map(|trait_fn| trait_fn.sig())),
        );
        let args = self.trait_generics.arguments(&self.impl_indirection);
        let self_ty = SelfTy {
            trait_dependency_mode: self.trait_dependency_mode,
            impl_indirection: &self.impl_indirection,
            mockable: self.opts.mockable(),
            span: self.trait_span,
        };
        let where_clause = self.trait_generics.impl_where_clause(
            trait_fns,
            self.trait_dependency_mode,
            &self.impl_indirection,
            self.trait_span,
        );

        let opt_self_scoping = if let FnInputMode::ImplBlock(ty) = self.fn_input_mode {
            Some(TokenPair(
                syn::token::SelfType(ty.span()),
                syn::token::PathSep(ty.span()),
            ))
        } else {
            None
        };

        let items = trait_fns.iter().map(|trait_fn| {
            let fn_item = self.gen_delegating_fn_item(trait_fn, self.trait_span, &opt_self_scoping);

            quote! {
                #fn_item
            }
        });

        let trait_impl_sub_attributes = self
            .sub_attributes
            .iter()
            .copied()
            .filter(|sub_attr| matches!(sub_attr, SubAttribute::AsyncTrait(_)));

        let trait_span = self.trait_span;
        let trait_ref = &self.trait_ref;

        quote_spanned! { trait_span=>
            #(#trait_impl_sub_attributes)*
            impl #params #trait_ref #args for #self_ty #where_clause {
                #(#items)*
            }
        }
    }

    /// Generate the fn (in the impl block) that calls the entraited fn
    fn gen_delegating_fn_item(
        &self,
        trait_fn: &TraitFn,
        span: Span,
        opt_self_scoping: &impl ToTokens,
    ) -> TokenStream {
        let entrait_sig = &trait_fn.entrait_sig;
        let trait_fn_sig = &trait_fn.sig();
        let deps = &trait_fn.deps;

        let mut fn_ident = trait_fn.sig().ident.clone();
        fn_ident.set_span(span);

        let opt_self_comma = match (deps, entrait_sig.sig.inputs.first(), &self.impl_indirection) {
            (generics::FnDeps::NoDeps { .. }, _, _) | (_, None, _) => None,
            (_, _, ImplIndirection::Static { .. } | ImplIndirection::Dynamic { .. }) => None,
            (_, Some(_), _) => Some(SelfArgComma(&self.impl_indirection, span)),
        };

        let arguments = entrait_sig
            .sig
            .inputs
            .iter()
            .filter_map(|fn_arg| match fn_arg {
                syn::FnArg::Receiver(_) => None,
                syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                    syn::Pat::Ident(pat_ident) => Some(&pat_ident.ident),
                    _ => {
                        panic!("Found a non-ident pattern, this should be handled in signature.rs")
                    }
                },
            });

        let opt_dot_await = trait_fn.opt_dot_await(span);

        quote_spanned! { span=>
            #trait_fn_sig {
                #opt_self_scoping #fn_ident(#opt_self_comma #(#arguments),*) #opt_dot_await
            }
        }
    }
}

struct SelfTy<'g, 'c> {
    trait_dependency_mode: &'g TraitDependencyMode<'g, 'c>,
    impl_indirection: &'g ImplIndirection<'g>,
    mockable: Mockable,
    span: Span,
}

impl quote::ToTokens for SelfTy<'_, '_> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        match &self.trait_dependency_mode {
            TraitDependencyMode::Generic(idents) => match self.impl_indirection {
                ImplIndirection::None => {
                    if self.mockable.yes() {
                        push_tokens!(stream, idents.impl_path(self.span))
                    } else {
                        push_tokens!(stream, idents.impl_t)
                    }
                }
                ImplIndirection::Static { ty } => {
                    push_tokens!(stream, ty);
                }
                ImplIndirection::Dynamic { ty } => {
                    push_tokens!(stream, ty);
                }
            },
            TraitDependencyMode::Concrete(ty) => {
                push_tokens!(stream, ty)
            }
        }
    }
}

// i.e. `self,`
struct SelfArgComma<'g>(&'g ImplIndirection<'g>, Span);

impl quote::ToTokens for SelfArgComma<'_> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let span = self.1;
        match &self.0 {
            ImplIndirection::None => {
                push_tokens!(stream, syn::token::SelfValue(span), syn::token::Comma(span));
            }
            ImplIndirection::Static { .. } => {
                push_tokens!(
                    stream,
                    syn::Ident::new("__impl", span),
                    syn::token::Comma(span)
                );
            }
            ImplIndirection::Dynamic { .. } => {
                push_tokens!(
                    stream,
                    syn::Ident::new("__impl", span),
                    syn::token::Comma(span)
                );
            }
        }
    }
}
