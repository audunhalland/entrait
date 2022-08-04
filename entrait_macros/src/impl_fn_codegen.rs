use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

use crate::analyze_generics::TraitFn;
use crate::attributes;
use crate::generics;
use crate::generics::TraitDependencyMode;
use crate::idents::CrateIdents;
use crate::input::InputFn;
use crate::opt::{AsyncStrategy, Opts, SpanOpt};
use crate::token_util::push_tokens;
use crate::token_util::TokenPair;

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
pub fn gen_impl_block(
    opts: &Opts,
    crate_idents: &CrateIdents,
    trait_ref: &(impl ToTokens + Spanned),
    trait_generics: &generics::TraitGenerics,
    trait_dependency_mode: &TraitDependencyMode,
    trait_fns: &[TraitFn],
    use_associated_future: generics::UseAssociatedFuture,
) -> TokenStream {
    let span = trait_ref.span();

    let async_trait_attribute = opt_async_trait_attribute(opts, crate_idents, trait_fns.iter());
    let params = trait_generics.impl_params(trait_dependency_mode, use_associated_future);
    let args = trait_generics.arguments();
    let self_ty = SelfTy(trait_dependency_mode, span);
    let where_clause = trait_generics.impl_where_clause(trait_fns, trait_dependency_mode, span);

    let items = trait_fns.iter().map(|trait_fn| {
        let associated_fut_impl = &trait_fn.entrait_sig.associated_fut_impl;

        let fn_item = gen_delegating_fn_item(trait_fn, span);

        quote! {
            #associated_fut_impl
            #fn_item
        }
    });

    quote_spanned! { span=>
        #async_trait_attribute
        impl #params #trait_ref #args for #self_ty #where_clause {
            #(#items)*
        }
    }
}

/// Generate the fn (in the impl block) that calls the entraited fn
fn gen_delegating_fn_item(trait_fn: &TraitFn, span: Span) -> TokenStream {
    let entrait_sig = &trait_fn.entrait_sig;
    let trait_fn_sig = &trait_fn.sig();
    let deps = &trait_fn.deps;

    let mut fn_ident = trait_fn.source.fn_sig.ident.clone();
    fn_ident.set_span(span);

    let opt_self_comma = match (deps, entrait_sig.sig.inputs.first()) {
        (generics::FnDeps::NoDeps { .. }, _) | (_, None) => None,
        (_, Some(_)) => Some(TokenPair(
            syn::token::SelfValue(span),
            syn::token::Comma(span),
        )),
    };

    let arguments = entrait_sig
        .sig
        .inputs
        .iter()
        .filter_map(|fn_arg| match fn_arg {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                syn::Pat::Ident(pat_ident) => Some(&pat_ident.ident),
                _ => panic!("Found a non-ident pattern, this should be handled in signature.rs"),
            },
        });

    let mut opt_dot_await = trait_fn.source.opt_dot_await(span);
    if entrait_sig.associated_fut_decl.is_some() {
        opt_dot_await = None;
    }

    quote_spanned! { span=>
        #trait_fn_sig {
            #fn_ident(#opt_self_comma #(#arguments),*) #opt_dot_await
        }
    }
}

struct SelfTy<'g, 'c>(&'g TraitDependencyMode<'g, 'c>, Span);

impl<'g, 'c> quote::ToTokens for SelfTy<'g, 'c> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let span = self.1;
        match &self.0 {
            TraitDependencyMode::Generic(idents) => {
                push_tokens!(stream, idents.impl_path(span))
            }
            TraitDependencyMode::Concrete(ty) => {
                push_tokens!(stream, ty)
            }
        }
    }
}

pub fn opt_async_trait_attribute<'s, 'o>(
    opts: &'s Opts,
    crate_idents: &'s CrateIdents,
    trait_fns: impl Iterator<Item = &'o TraitFn<'o>>,
) -> Option<impl ToTokens + 's> {
    match (
        opts.async_strategy(),
        generics::has_any_async(trait_fns.map(|trait_fn| trait_fn.sig())),
    ) {
        (SpanOpt(AsyncStrategy::AsyncTrait, span), true) => {
            Some(attributes::Attr(attributes::AsyncTraitParams {
                crate_idents,
                span,
            }))
        }
        _ => None,
    }
}

impl InputFn {
    fn opt_dot_await(&self, span: Span) -> Option<impl ToTokens> {
        if self.fn_sig.asyncness.is_some() {
            Some(TokenPair(syn::token::Dot(span), syn::token::Await(span)))
        } else {
            None
        }
    }

    pub fn use_associated_future(&self, opts: &Opts) -> bool {
        matches!(
            (opts.async_strategy(), self.fn_sig.asyncness),
            (SpanOpt(AsyncStrategy::AssociatedFuture, _), Some(_async))
        )
    }
}
