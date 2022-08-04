use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::{quote, quote_spanned};

use crate::analyze_generics::TraitFn;
use crate::attributes;
use crate::generics;
use crate::generics::ImplIndirection;
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
    trait_ref: &impl ToTokens,
    trait_span: Span,
    impl_indirection: ImplIndirection,
    trait_generics: &generics::TraitGenerics,
    trait_dependency_mode: &TraitDependencyMode,
    trait_fns: &[TraitFn],
    use_associated_future: generics::UseAssociatedFuture,
) -> TokenStream {
    let async_trait_attribute = opt_async_trait_attribute(opts, crate_idents, trait_fns.iter());
    let params = trait_generics.impl_params(
        trait_dependency_mode,
        &impl_indirection,
        use_associated_future,
    );
    let args = trait_generics.arguments(&impl_indirection);
    let self_ty = SelfTy(trait_dependency_mode, &impl_indirection, trait_span);
    let where_clause = trait_generics.impl_where_clause(
        trait_fns,
        trait_dependency_mode,
        &impl_indirection,
        trait_span,
    );

    let items = trait_fns.iter().map(|trait_fn| {
        let associated_fut_impl = &trait_fn.entrait_sig.associated_fut_impl;

        let fn_item = gen_delegating_fn_item(trait_fn, &impl_indirection, trait_span);

        quote! {
            #associated_fut_impl
            #fn_item
        }
    });

    quote_spanned! { trait_span=>
        #async_trait_attribute
        impl #params #trait_ref #args for #self_ty #where_clause {
            #(#items)*
        }
    }
}

/// Generate the fn (in the impl block) that calls the entraited fn
fn gen_delegating_fn_item(
    trait_fn: &TraitFn,
    impl_indirection: &ImplIndirection,
    span: Span,
) -> TokenStream {
    let entrait_sig = &trait_fn.entrait_sig;
    let trait_fn_sig = &trait_fn.sig();
    let deps = &trait_fn.deps;

    let mut fn_ident = trait_fn.source.fn_sig.ident.clone();
    fn_ident.set_span(span);

    let opt_self_comma = match (deps, entrait_sig.sig.inputs.first(), impl_indirection) {
        (generics::FnDeps::NoDeps { .. }, _, _) | (_, None, _) => None,
        (_, _, ImplIndirection::DynCopy { .. }) => None,
        (_, Some(_), _) => Some(SelfArgComma(impl_indirection, span)),
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

struct SelfTy<'g, 'c>(
    &'g TraitDependencyMode<'g, 'c>,
    &'g ImplIndirection<'g>,
    Span,
);

impl<'g, 'c> quote::ToTokens for SelfTy<'g, 'c> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let span = self.2;
        match &self.0 {
            TraitDependencyMode::Generic(idents) => match self.1 {
                ImplIndirection::None => {
                    push_tokens!(stream, idents.impl_path(span))
                }
                ImplIndirection::ImplRef { ref_lifetime } => {
                    push_tokens!(
                        stream,
                        syn::Ident::new("__ImplRef", span),
                        syn::token::Lt(span),
                        ref_lifetime,
                        syn::token::Comma(span),
                        idents.impl_t,
                        syn::token::Gt(span)
                    );
                }
                ImplIndirection::DynCopy { ident } => {
                    push_tokens!(stream, ident);
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

impl<'g> quote::ToTokens for SelfArgComma<'g> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let span = self.1;
        match &self.0 {
            ImplIndirection::None => {
                push_tokens!(stream, syn::token::SelfValue(span), syn::token::Comma(span));
            }
            ImplIndirection::ImplRef { .. } => {
                push_tokens!(
                    stream,
                    syn::token::SelfValue(span),
                    syn::token::Dot(span),
                    syn::LitInt::new("0", span),
                    syn::token::Comma(span)
                );
            }
            ImplIndirection::DynCopy { .. } => {
                push_tokens!(
                    stream,
                    syn::Ident::new("__impl", span),
                    syn::token::Comma(span)
                );
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
