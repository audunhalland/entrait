//! Implementation for invoking entrait on a trait!

pub mod input_attr;
mod out_trait;

use input_attr::EntraitTraitAttr;
use proc_macro2::Span;

use crate::analyze_generics::TraitFn;
use crate::attributes;
use crate::entrait_trait::input_attr::ImplTrait;
use crate::generics;
use crate::generics::TraitDependencyMode;
use crate::idents::GenericIdents;
use crate::input::FnInputMode;
use crate::input::LiteralAttrs;
use crate::opt::*;
use crate::token_util::*;
use crate::trait_codegen::Supertraits;
use crate::trait_codegen::TraitCodegen;

use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;

use self::out_trait::OutTrait;

#[derive(Clone, Copy)]
struct ContainsAsync(bool);

pub fn output_tokens(
    attr: EntraitTraitAttr,
    item_trait: syn::ItemTrait,
) -> syn::Result<TokenStream> {
    if let (None, Some(SpanOpt(Delegate::ByTrait(_), span))) =
        (&attr.impl_trait, &attr.delegation_kind)
    {
        return Err(syn::Error::new(
            *span,
            "Cannot use a custom delegating trait without a custom trait to delegate to. Use either `#[entrait(TraitImpl, delegate_by = DelegateTrait)]` or `#[entrait(delegate_by = ref)]`",
        ));
    }

    let trait_ident_span = item_trait.ident.span();
    let contains_async = ContainsAsync(item_trait.items.iter().any(|item| match item {
        syn::TraitItem::Fn(method) => method.sig.asyncness.is_some(),
        _ => false,
    }));
    let impl_attrs = item_trait
        .attrs
        .iter()
        .filter(|attr| {
            matches!(
                attr.path().segments.last(),
                Some(last_segment) if last_segment.ident == "async_trait"
            )
        })
        .cloned()
        .collect::<Vec<_>>();

    let out_trait = out_trait::analyze_trait(item_trait)?;
    let trait_dependency_mode = TraitDependencyMode::Generic(GenericIdents::new(
        &attr.crate_idents,
        out_trait.ident.span(),
    ));
    let generic_idents = match &trait_dependency_mode {
        TraitDependencyMode::Generic(idents) => idents,
        _ => panic!(),
    };

    let mut impl_async_trait_attr =
        attributes::opt_async_trait_attr(&attr.opts, &attr.crate_idents, out_trait.fns.iter());
    if !impl_attrs.is_empty() {
        impl_async_trait_attr = None;
    }

    let delegation_trait_def =
        gen_impl_delegation_trait_defs(&out_trait, &trait_dependency_mode, generic_idents, &attr)?;

    let trait_def = TraitCodegen {
        crate_idents: &attr.crate_idents,
        opts: &attr.opts,
        trait_indirection: generics::TraitIndirection::Trait,
        trait_dependency_mode: &trait_dependency_mode,
    }
    .gen_trait_def(
        &out_trait.vis,
        &out_trait.ident,
        &out_trait.generics,
        &out_trait.supertraits,
        &out_trait.fns,
        &FnInputMode::RawTrait(LiteralAttrs(&out_trait.attrs)),
    )?;

    let trait_ident = &out_trait.ident;
    let params = out_trait.generics.impl_params_from_idents(
        generic_idents,
        generics::UseAssociatedFuture(false),
        generics::TakesSelfByValue(false), // BUG?
    );
    let args = out_trait
        .generics
        .arguments(&generics::ImplIndirection::None);
    let self_ty = generic_idents.impl_path(trait_ident_span);
    let where_clause = ImplWhereClause {
        out_trait: &out_trait,
        contains_async,
        trait_generics: &out_trait.generics,
        generic_idents,
        attr: &attr,
        span: trait_ident_span,
    };

    let impl_assoc_types = out_trait.fns.iter().filter_map(|trait_fn| {
        trait_fn
            .entrait_sig
            .associated_fut_impl(generics::TraitIndirection::Plain, &attr.crate_idents)
    });

    let method_items = out_trait
        .fns
        .iter()
        .map(|trait_fn| gen_delegation_method(trait_fn, generic_idents, &attr, contains_async));

    Ok(quote! {
        #trait_def

        #delegation_trait_def

        #(#impl_attrs)*
        #impl_async_trait_attr
        impl #params #trait_ident #args for #self_ty #where_clause {
            #(#impl_assoc_types)*
            #(#method_items)*
        }
    })
}

fn gen_impl_delegation_trait_defs(
    out_trait: &OutTrait,
    trait_dependency_mode: &TraitDependencyMode,
    generic_idents: &GenericIdents,
    attr: &EntraitTraitAttr,
) -> syn::Result<Option<TokenStream>> {
    let entrait = &generic_idents.crate_idents.entrait;

    let ImplTrait(_, impl_trait_ident) = match &attr.impl_trait {
        Some(impl_trait) => impl_trait,
        None => return Ok(None),
    };

    let mut trait_copy = out_trait.clone();
    trait_copy.ident = impl_trait_ident.clone();

    let no_mock_opts = Opts {
        mock_api: None,
        unimock: None,
        mockall: None,
        ..attr.opts
    };

    match &attr.delegation_kind {
        Some(SpanOpt(Delegate::ByTrait(delegation_ident), _)) => {
            trait_copy.generics.params.insert(
                0,
                syn::parse_quote! {
                    EntraitT
                },
            );
            for trait_fn in trait_copy.fns.iter_mut() {
                if !matches!(trait_fn.sig().inputs.first(), Some(syn::FnArg::Receiver(_))) {
                    continue;
                }

                if let Some(first_arg) = trait_fn.entrait_sig.sig.inputs.first_mut() {
                    if let syn::FnArg::Receiver(receiver) = first_arg {
                        *first_arg = if let Some((and, lifetime)) = receiver.reference.clone() {
                            syn::parse_quote! {
                                __impl: #and #lifetime ::#entrait::Impl<EntraitT>
                            }
                        } else {
                            syn::parse_quote! {
                                __impl: ::#entrait::Impl<EntraitT>
                            }
                        }
                    }
                }
            }

            let trait_def = TraitCodegen {
                crate_idents: &attr.crate_idents,
                opts: &no_mock_opts,
                trait_indirection: generics::TraitIndirection::StaticImpl,
                trait_dependency_mode,
            }
            .gen_trait_def(
                &trait_copy.vis,
                &trait_copy.ident,
                &trait_copy.generics,
                &Supertraits::Some {
                    colon_token: syn::token::Colon::default(),
                    bounds: syn::parse_quote! { 'static },
                },
                &trait_copy.fns,
                &FnInputMode::RawTrait(LiteralAttrs(&[])),
            )?;

            Ok(Some(quote! {
                #trait_def

                pub trait #delegation_ident<T> {
                    type Target: #impl_trait_ident<T>;
                }
            }))
        }
        Some(SpanOpt(Delegate::ByRef(_), _)) => {
            trait_copy.generics.params.insert(
                0,
                syn::parse_quote! {
                    EntraitT
                },
            );
            for trait_fn in trait_copy.fns.iter_mut() {
                if !matches!(trait_fn.sig().inputs.first(), Some(syn::FnArg::Receiver(_))) {
                    continue;
                }

                trait_fn.entrait_sig.sig.inputs.insert(
                    1,
                    syn::parse_quote! {
                        __impl: &::#entrait::Impl<EntraitT>
                    },
                );
            }

            let no_mock_opts = Opts {
                mock_api: None,
                unimock: None,
                mockall: None,
                ..attr.opts
            };

            let trait_def = TraitCodegen {
                crate_idents: &attr.crate_idents,
                opts: &no_mock_opts,
                trait_indirection: generics::TraitIndirection::DynamicImpl,
                trait_dependency_mode,
            }
            .gen_trait_def(
                &trait_copy.vis,
                &trait_copy.ident,
                &trait_copy.generics,
                &Supertraits::Some {
                    colon_token: syn::token::Colon::default(),
                    bounds: syn::parse_quote! { 'static },
                },
                &trait_copy.fns,
                &FnInputMode::RawTrait(LiteralAttrs(&[])),
            )?;

            Ok(Some(trait_def))
        }
        _ => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "Missing delegate_by",
        )),
    }
}

fn gen_delegation_method<'s>(
    trait_fn: &'s TraitFn,
    generic_idents: &'s GenericIdents,
    attr: &'s EntraitTraitAttr,
    contains_async: ContainsAsync,
) -> DelegatingMethod<'s> {
    let fn_sig = &trait_fn.sig();
    let fn_ident = &fn_sig.ident;
    let impl_t = &generic_idents.impl_t;

    let arguments = fn_sig.inputs.iter().filter_map(|arg| match arg {
        syn::FnArg::Receiver(_) => None,
        syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
            syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_token_stream()),
            _ => panic!("Found a non-ident pattern, this should be handled in signature.rs"),
        },
    });
    let core = &generic_idents.crate_idents.core;

    match (&attr.impl_trait, &attr.delegation_kind) {
        (Some(ImplTrait(_, impl_trait_ident)), Some(SpanOpt(Delegate::ByTrait(_), _))) => {
            DelegatingMethod {
                attr,
                trait_fn,
                needs_async_move: true,
                call: quote! {
                    // TODO: pass additional generic arguments(?)
                    <#impl_t::Target as #impl_trait_ident<#impl_t>>::#fn_ident(self, #(#arguments),*)
                },
            }
        }
        (Some(ImplTrait(_, impl_trait_ident)), Some(SpanOpt(Delegate::ByRef(ref_delegate), _))) => {
            let plus_sync = if contains_async.0 {
                Some(TokenPair(
                    syn::token::Plus::default(),
                    syn::Ident::new("Sync", Span::call_site()),
                ))
            } else {
                None
            };
            let call = match ref_delegate {
                RefDelegate::AsRef => {
                    quote! {
                        <#impl_t as ::#core::convert::AsRef<dyn #impl_trait_ident<#impl_t> #plus_sync>>::as_ref(&*self)
                            .#fn_ident(self, #(#arguments),*)
                    }
                }
                RefDelegate::Borrow => {
                    quote! {
                        <#impl_t as ::#core::borrow::Borrow<dyn #impl_trait_ident<#impl_t> #plus_sync>>::borrow(&*self)
                            .#fn_ident(self, #(#arguments),*)
                    }
                }
            };

            DelegatingMethod {
                attr,
                trait_fn,
                needs_async_move: false,
                call,
            }
        }
        (None, Some(SpanOpt(Delegate::ByRef(RefDelegate::AsRef), _))) => DelegatingMethod {
            attr,
            trait_fn,
            needs_async_move: false,
            call: quote! {
                self.as_ref().as_ref().#fn_ident(#(#arguments),*)
            },
        },
        (None, Some(SpanOpt(Delegate::ByRef(RefDelegate::Borrow), _))) => DelegatingMethod {
            attr,
            trait_fn,
            needs_async_move: false,
            call: quote! {
                self.as_ref().borrow().#fn_ident(#(#arguments),*)
            },
        },
        _ => DelegatingMethod {
            attr,
            trait_fn,
            needs_async_move: false,
            call: quote! {
                self.as_ref().#fn_ident(#(#arguments),*)
            },
        },
    }
}

struct DelegatingMethod<'s> {
    attr: &'s EntraitTraitAttr,
    trait_fn: &'s TraitFn,
    needs_async_move: bool,
    call: TokenStream,
}

impl<'s> DelegatingMethod<'s> {
    fn should_inline(&self) -> bool {
        if matches!(
            &self.attr.delegation_kind,
            Some(SpanOpt(Delegate::ByRef(_), _))
        ) {
            return false;
        }

        if self.trait_fn.originally_async
            && matches!(
                self.attr.opts.async_strategy(),
                SpanOpt(AsyncStrategy::BoxFuture, _)
            )
        {
            return false;
        }

        true
    }
}

impl<'s> ToTokens for DelegatingMethod<'s> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        // Just "mirroring" all the attributes from
        // the trait definition to the implementation
        // is maybe a bit naive..
        // There's a risk this will not always work in all cases.
        for attr in &self.trait_fn.attrs {
            push_tokens!(stream, attr);
        }

        if self.should_inline() {
            quote! { #[inline] }.to_tokens(stream);
        }
        self.trait_fn.sig().to_tokens(stream);
        syn::token::Brace::default().surround(stream, |stream| {
            if self.needs_async_move && self.trait_fn.entrait_sig.associated_fut.is_some() {
                push_tokens!(
                    stream,
                    syn::token::Async::default(),
                    syn::token::Move::default()
                );
                syn::token::Brace::default().surround(stream, |stream| {
                    self.call.to_tokens(stream);
                    push_tokens!(
                        stream,
                        syn::token::Dot::default(),
                        syn::token::Await::default()
                    );
                });
            } else if self.trait_fn.originally_async {
                self.call.to_tokens(stream);
                push_tokens!(
                    stream,
                    syn::token::Dot::default(),
                    syn::token::Await::default()
                );
            } else {
                self.call.to_tokens(stream);
            }
        });
    }
}

struct ImplWhereClause<'g, 'c> {
    out_trait: &'g OutTrait,
    contains_async: ContainsAsync,
    trait_generics: &'g generics::TraitGenerics,
    generic_idents: &'g GenericIdents<'c>,
    attr: &'g EntraitTraitAttr,
    span: proc_macro2::Span,
}

impl<'g, 'c> ImplWhereClause<'g, 'c> {
    fn push_impl_t_bounds(&self, stream: &mut TokenStream) {
        use syn::token::*;

        push_tokens!(stream, self.generic_idents.impl_t, Colon(self.span));

        match (&self.attr.impl_trait, &self.attr.delegation_kind) {
            (Some(_), Some(SpanOpt(Delegate::ByTrait(delegate_ident), _))) => {
                push_tokens!(
                    stream,
                    delegate_ident,
                    Lt(self.span),
                    self.generic_idents.impl_t,
                    Gt(self.span),
                    self.plus_sync(),
                    self.plus_static()
                );
            }
            (
                Some(ImplTrait(_, impl_trait_ident)),
                Some(SpanOpt(Delegate::ByRef(ref_delegate), _)),
            ) => {
                self.push_core_delegation_trait(stream, ref_delegate);
                push_tokens!(
                    stream,
                    // Generic arguments:
                    Lt(self.span),
                    Dyn(self.span),
                    impl_trait_ident,
                    Lt(self.span),
                    self.generic_idents.impl_t,
                    Gt(self.span),
                    if self.contains_async.0 {
                        Some(self.plus_sync())
                    } else {
                        None
                    },
                    Gt(self.span)
                );

                if self.contains_async.0 {
                    push_tokens!(stream, self.plus_send(), self.plus_sync());
                }
                push_tokens!(stream, self.plus_static());
            }
            (None, Some(SpanOpt(Delegate::ByRef(ref_delegate), _))) => {
                self.push_core_delegation_trait(stream, ref_delegate);
                push_tokens!(
                    stream,
                    Lt(self.span),
                    Dyn(self.span),
                    self.trait_with_arguments(),
                    Gt(self.span)
                );

                if self.contains_async.0 {
                    push_tokens!(stream, self.plus_send(), self.plus_sync());
                }
                push_tokens!(stream, self.plus_static());
            }
            _delegate_to_impl_t => {
                push_tokens!(stream, self.trait_with_arguments(), self.plus_sync());
                if self.contains_async.0 {
                    // There will be a `self.as_ref().fn().await`,
                    // that borrow will need to be 'static for the future to be Send
                    push_tokens!(stream, self.plus_static());
                }
            }
        }
    }

    fn push_core_delegation_trait(&self, stream: &mut TokenStream, ref_delegate: &RefDelegate) {
        use syn::token::*;
        match ref_delegate {
            RefDelegate::AsRef => {
                push_tokens!(
                    stream,
                    PathSep(self.span),
                    self.generic_idents.crate_idents.core,
                    PathSep(self.span),
                    syn::Ident::new("convert", self.span),
                    PathSep(self.span),
                    syn::Ident::new("AsRef", self.span)
                );
            }
            RefDelegate::Borrow => {
                push_tokens!(
                    stream,
                    PathSep(self.span),
                    self.generic_idents.crate_idents.core,
                    PathSep(self.span),
                    syn::Ident::new("borrow", self.span),
                    PathSep(self.span),
                    syn::Ident::new("Borrow", self.span)
                );
            }
        }
    }

    fn trait_with_arguments(&self) -> TokenPair<impl ToTokens + '_, impl ToTokens + '_> {
        TokenPair(
            &self.out_trait.ident,
            self.trait_generics
                .arguments(&generics::ImplIndirection::None),
        )
    }

    fn plus_static(&self) -> TokenPair<impl ToTokens, impl ToTokens> {
        TokenPair(
            syn::token::Plus(self.span),
            syn::Lifetime::new("'static", self.span),
        )
    }

    fn plus_send(&self) -> TokenPair<impl ToTokens, impl ToTokens> {
        TokenPair(
            syn::token::Plus(self.span),
            syn::Ident::new("Send", self.span),
        )
    }

    fn plus_sync(&self) -> TokenPair<impl ToTokens, impl ToTokens> {
        TokenPair(
            syn::token::Plus(self.span),
            syn::Ident::new("Sync", self.span),
        )
    }
}

impl<'g, 'c> quote::ToTokens for ImplWhereClause<'g, 'c> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Where(self.span),
            syn::token::Comma(self.span),
            EmptyToken,
        );

        // Bounds on the `T` in `Impl<T>`:
        punctuator.push_fn(|stream| {
            self.push_impl_t_bounds(stream);
        });

        for predicate in &self.trait_generics.where_predicates {
            punctuator.push(predicate);
        }
    }
}
