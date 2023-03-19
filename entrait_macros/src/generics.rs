use proc_macro2::TokenStream;

use crate::{
    analyze_generics::TraitFn,
    idents::GenericIdents,
    input::InputFn,
    opt::{AsyncStrategy, Opts, SpanOpt},
    token_util::{push_tokens, EmptyToken, Punctuator, TokenPair},
};

#[derive(Clone)]
pub enum ImplIndirection<'s> {
    None,
    Static { ty: &'s syn::Type },
    Dynamic { ty: &'s syn::Type },
}

impl<'s> ImplIndirection<'s> {
    pub fn to_trait_indirection(&'s self) -> TraitIndirection {
        match self {
            Self::None => TraitIndirection::Plain,
            Self::Static { .. } => TraitIndirection::StaticImpl,
            Self::Dynamic { .. } => TraitIndirection::DynamicImpl,
        }
    }
}

#[derive(Clone, Copy)]
pub enum TraitIndirection {
    /// Normal entrait/fn, entrait/mod etc
    Plain,
    /// Trait indirection is the original entraited trait
    Trait,
    /// The static "impl" trait of an entraited trait
    StaticImpl,
    /// The dynamic/borrow "impl" trait of an entraited trait
    DynamicImpl,
}

#[derive(Clone, Copy)]
pub struct UseAssociatedFuture(pub bool);

pub fn detect_use_associated_future<'i>(
    opts: &Opts,
    input_fns: impl Iterator<Item = &'i InputFn>,
) -> UseAssociatedFuture {
    UseAssociatedFuture(matches!(
        (
            opts.async_strategy(),
            has_any_async(input_fns.map(|input_fn| &input_fn.fn_sig))
        ),
        (SpanOpt(AsyncStrategy::AssociatedFuture, _), true)
    ))
}

pub fn has_any_async<'s>(mut signatures: impl Iterator<Item = &'s syn::Signature>) -> bool {
    signatures.any(|sig| sig.asyncness.is_some())
}

#[derive(Clone, Copy)]
pub struct TakesSelfByValue(pub bool);

pub fn has_any_self_by_value<'s>(
    mut signatures: impl Iterator<Item = &'s syn::Signature>,
) -> TakesSelfByValue {
    TakesSelfByValue(signatures.any(|sig| match sig.inputs.first() {
        Some(syn::FnArg::Receiver(receiver)) => receiver.reference.is_none(),
        _ => false,
    }))
}

#[derive(Clone)]
pub enum FnDeps {
    Generic {
        generic_param: Option<syn::Ident>,
        trait_bounds: Vec<syn::TypeParamBound>,
    },
    Concrete(Box<syn::Type>),
    NoDeps,
}

pub enum TraitDependencyMode<'t, 'c> {
    Generic(GenericIdents<'c>),
    Concrete(&'t syn::Type),
}

#[derive(Clone)]
pub struct TraitGenerics {
    pub params: syn::punctuated::Punctuated<syn::GenericParam, syn::token::Comma>,
    pub where_predicates: syn::punctuated::Punctuated<syn::WherePredicate, syn::token::Comma>,
}

impl TraitGenerics {
    pub fn trait_params(&self) -> ParamsGenerator<'_> {
        ParamsGenerator {
            params: &self.params,
            impl_t: None,
            use_associated_future: UseAssociatedFuture(false),
            takes_self_by_value: TakesSelfByValue(false),
        }
    }

    pub fn trait_where_clause(&self) -> TraitWhereClauseGenerator<'_> {
        TraitWhereClauseGenerator {
            where_predicates: &self.where_predicates,
        }
    }

    pub fn impl_params<'i>(
        &'i self,
        trait_dependency_mode: &'i TraitDependencyMode<'i, '_>,
        use_associated_future: UseAssociatedFuture,
        takes_self_by_value: TakesSelfByValue,
    ) -> ParamsGenerator<'_> {
        ParamsGenerator {
            params: &self.params,
            impl_t: match trait_dependency_mode {
                TraitDependencyMode::Generic(idents) => Some(&idents.impl_t),
                TraitDependencyMode::Concrete(_) => None,
            },
            use_associated_future,
            takes_self_by_value,
        }
    }

    pub fn impl_params_from_idents<'i>(
        &'i self,
        idents: &'i GenericIdents,
        use_associated_future: UseAssociatedFuture,
        takes_self_by_value: TakesSelfByValue,
    ) -> ParamsGenerator<'_> {
        ParamsGenerator {
            params: &self.params,
            impl_t: Some(&idents.impl_t),
            use_associated_future,
            takes_self_by_value,
        }
    }

    pub fn impl_where_clause<'g, 's, 'c>(
        &'g self,
        trait_fns: &'s [TraitFn],
        trait_dependency_mode: &'s TraitDependencyMode<'s, 'c>,
        impl_indirection: &'s ImplIndirection,
        span: proc_macro2::Span,
    ) -> ImplWhereClauseGenerator<'g, 's, 'c> {
        ImplWhereClauseGenerator {
            trait_where_predicates: &self.where_predicates,
            trait_dependency_mode,
            impl_indirection,
            trait_fns,
            span,
        }
    }

    pub fn arguments<'s>(
        &'s self,
        impl_indirection: &'s ImplIndirection,
    ) -> ArgumentsGenerator<'s> {
        ArgumentsGenerator {
            params: &self.params,
            impl_indirection,
        }
    }
}

impl<'c> GenericIdents<'c> {
    /// `::entrait::Impl<EntraitT>`
    pub fn impl_path<'s>(&'s self, span: proc_macro2::Span) -> ImplPath<'s, 'c> {
        ImplPath(self, span)
    }
}

/// `::entrait::Impl<EntraitT>`
pub struct ImplPath<'g, 'c>(&'g GenericIdents<'c>, proc_macro2::Span);

impl<'g, 'c> quote::ToTokens for ImplPath<'g, 'c> {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let span = self.1;

        push_tokens!(
            stream,
            syn::token::PathSep(span),
            self.0.crate_idents.entrait,
            syn::token::PathSep(span),
            self.0.impl_self,
            syn::token::Lt(span),
            self.0.impl_t,
            syn::token::Gt(span)
        );
    }
}

// Params as in impl<..Param>
pub struct ParamsGenerator<'g> {
    params: &'g syn::punctuated::Punctuated<syn::GenericParam, syn::token::Comma>,
    impl_t: Option<&'g syn::Ident>,
    use_associated_future: UseAssociatedFuture,
    takes_self_by_value: TakesSelfByValue,
}

impl<'g> quote::ToTokens for ParamsGenerator<'g> {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Lt::default(),
            syn::token::Comma::default(),
            syn::token::Gt::default(),
        );

        if let Some(impl_t) = &self.impl_t {
            punctuator.push_fn(|stream| {
                push_tokens!(
                    stream,
                    impl_t,
                    syn::token::Colon::default(),
                    syn::Ident::new("Sync", proc_macro2::Span::call_site())
                );

                if self.takes_self_by_value.0 {
                    push_tokens!(
                        stream,
                        syn::token::Plus::default(),
                        // In case T is not a reference, it has to be Send
                        syn::Ident::new("Send", proc_macro2::Span::call_site())
                    );
                }

                if self.use_associated_future.0 {
                    push_tokens!(
                        stream,
                        syn::token::Plus::default(),
                        // Deps must be 'static for zero-cost futures to work
                        syn::Lifetime::new("'static", proc_macro2::Span::call_site())
                    );
                }
            });
        }

        for param in self.params {
            punctuator.push(param);
        }
    }
}

// Args as in impl<..Param> T for U<..Arg>
pub struct ArgumentsGenerator<'g> {
    params: &'g syn::punctuated::Punctuated<syn::GenericParam, syn::token::Comma>,
    impl_indirection: &'g ImplIndirection<'g>,
}

impl<'g> quote::ToTokens for ArgumentsGenerator<'g> {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Lt::default(),
            syn::token::Comma::default(),
            syn::token::Gt::default(),
        );

        if matches!(
            &self.impl_indirection,
            ImplIndirection::Static { .. } | ImplIndirection::Dynamic { .. }
        ) {
            punctuator.push(syn::Ident::new("EntraitT", proc_macro2::Span::call_site()));
        }

        for pair in self.params.pairs() {
            match pair.value() {
                syn::GenericParam::Type(type_param) => {
                    punctuator.push(&type_param.ident);
                }
                syn::GenericParam::Lifetime(lifetime_def) => {
                    punctuator.push(&lifetime_def.lifetime);
                }
                syn::GenericParam::Const(const_param) => {
                    punctuator.push(&const_param.ident);
                }
            }
        }
    }
}

pub struct TraitWhereClauseGenerator<'g> {
    where_predicates: &'g syn::punctuated::Punctuated<syn::WherePredicate, syn::token::Comma>,
}

impl<'g> quote::ToTokens for TraitWhereClauseGenerator<'g> {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        if self.where_predicates.is_empty() {
            return;
        }

        push_tokens!(stream, syn::token::Where::default());

        for pair in self.where_predicates.pairs() {
            push_tokens!(stream, pair);
        }
    }
}

pub struct ImplWhereClauseGenerator<'g, 's, 'c> {
    trait_where_predicates: &'g syn::punctuated::Punctuated<syn::WherePredicate, syn::token::Comma>,
    trait_dependency_mode: &'s TraitDependencyMode<'s, 'c>,
    impl_indirection: &'s ImplIndirection<'s>,
    trait_fns: &'s [TraitFn],
    span: proc_macro2::Span,
}

impl<'g, 's, 'c> quote::ToTokens for ImplWhereClauseGenerator<'g, 's, 'c> {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Where(self.span),
            syn::token::Comma(self.span),
            EmptyToken,
        );

        // The where clause looks quite different depending on what kind of Deps is used in the function.
        match &self.trait_dependency_mode {
            TraitDependencyMode::Generic(generic_idents) => {
                // Impl<T> bounds

                let has_bounds = self.trait_fns.iter().any(|trait_fn| match &trait_fn.deps {
                    FnDeps::Generic { trait_bounds, .. } => !trait_bounds.is_empty(),
                    _ => false,
                });

                if has_bounds {
                    punctuator.push_fn(|stream| match self.impl_indirection {
                        ImplIndirection::None => {
                            push_impl_t_bounds(
                                stream,
                                syn::token::SelfType(self.span),
                                self.trait_fns,
                                self.span,
                            );
                        }
                        ImplIndirection::Static { .. } | ImplIndirection::Dynamic { .. } => {
                            push_impl_t_bounds(
                                stream,
                                generic_idents.impl_path(self.span),
                                self.trait_fns,
                                self.span,
                            );
                        }
                    });
                }
            }
            TraitDependencyMode::Concrete(_) => {
                // NOTE: the impl for Impl<T> is generated by invoking #[entrait] on the trait(!),
                // So we need only one impl here: for the path (the `T` in `Impl<T>`).
            }
        };

        for predicate in self.trait_where_predicates {
            punctuator.push(predicate);
        }
    }
}

fn push_impl_t_bounds(
    stream: &mut TokenStream,
    bound_param: impl quote::ToTokens,
    trait_fns: &[TraitFn],
    span: proc_macro2::Span,
) {
    let mut bound_punctuator = Punctuator::new(
        stream,
        TokenPair(bound_param, syn::token::Colon(span)),
        syn::token::Plus(span),
        EmptyToken,
    );

    for trait_fn in trait_fns {
        if let FnDeps::Generic { trait_bounds, .. } = &trait_fn.deps {
            for bound in trait_bounds {
                bound_punctuator.push(bound);
            }
        }
    }
}
