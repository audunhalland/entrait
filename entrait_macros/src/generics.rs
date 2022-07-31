use crate::{
    entrait_fn::OutputFn,
    token_util::{push_tokens, EmptyToken, Punctuator, TokenPair},
};

pub struct UseAssociatedFuture(pub bool);

pub enum FnDeps {
    Generic {
        generic_param: Option<syn::Ident>,
        trait_bounds: Vec<syn::TypeParamBound>,
    },
    Concrete(Box<syn::Type>),
    NoDeps,
}

pub enum TraitDependencyMode<'t> {
    Generic(GenericIdents),
    Concrete(&'t syn::Type),
}

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
        }
    }

    pub fn trait_where_clause(&self) -> TraitWhereClauseGenerator<'_> {
        TraitWhereClauseGenerator {
            where_predicates: &self.where_predicates,
        }
    }

    pub fn impl_params<'s, 'i>(
        &'i self,
        trait_dependency_mode: &'i TraitDependencyMode<'i>,
        use_associated_future: UseAssociatedFuture,
    ) -> ParamsGenerator<'_> {
        ParamsGenerator {
            params: &self.params,
            impl_t: match trait_dependency_mode {
                TraitDependencyMode::Generic(idents) => Some(&idents.impl_t),
                TraitDependencyMode::Concrete(_) => None,
            },
            use_associated_future,
        }
    }

    pub fn impl_params_from_idents<'s, 'i>(
        &'i self,
        idents: &'i GenericIdents,
        use_associated_future: UseAssociatedFuture,
    ) -> ParamsGenerator<'_> {
        ParamsGenerator {
            params: &self.params,
            impl_t: Some(&idents.impl_t),
            use_associated_future,
        }
    }

    pub fn impl_where_clause<'g, 's>(
        &'g self,
        output_fns: &'s [OutputFn],
        trait_dependency_mode: &'s TraitDependencyMode<'s>,
        span: proc_macro2::Span,
    ) -> ImplWhereClauseGenerator<'g, 's> {
        ImplWhereClauseGenerator {
            trait_where_predicates: &self.where_predicates,
            trait_dependency_mode,
            output_fns,
            span,
        }
    }

    pub fn arguments(&self) -> ArgumentsGenerator {
        ArgumentsGenerator {
            params: &self.params,
        }
    }
}

pub struct GenericIdents {
    /// "entrait"
    pub entrait_crate: syn::Ident,

    /// "Impl"
    pub impl_self: syn::Ident,

    /// The "T" in `Impl<T>`
    pub impl_t: syn::Ident,
}

impl GenericIdents {
    pub fn new(span: proc_macro2::Span) -> Self {
        Self {
            entrait_crate: syn::Ident::new("entrait", span),
            impl_self: syn::Ident::new("Impl", span),
            impl_t: syn::Ident::new("EntraitT", span),
        }
    }

    /// `::entrait::Impl<EntraitT>`
    pub fn impl_path(&self, span: proc_macro2::Span) -> ImplPath<'_> {
        ImplPath(self, span)
    }
}

/// `::entrait::Impl<EntraitT>`
pub struct ImplPath<'g>(&'g GenericIdents, proc_macro2::Span);

impl<'g> quote::ToTokens for ImplPath<'g> {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let span = self.1;

        push_tokens!(
            stream,
            syn::token::Colon2(span),
            self.0.entrait_crate,
            syn::token::Colon2(span),
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

                if self.use_associated_future.0 {
                    // Deps must be 'static for zero-cost futures to work
                    push_tokens!(
                        stream,
                        syn::token::Add::default(),
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
}

impl<'g> quote::ToTokens for ArgumentsGenerator<'g> {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Lt::default(),
            syn::token::Comma::default(),
            syn::token::Gt::default(),
        );

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

pub struct ImplWhereClauseGenerator<'g, 's> {
    trait_where_predicates: &'g syn::punctuated::Punctuated<syn::WherePredicate, syn::token::Comma>,
    trait_dependency_mode: &'s TraitDependencyMode<'s>,
    output_fns: &'s [OutputFn<'s>],
    span: proc_macro2::Span,
}

impl<'g, 's> quote::ToTokens for ImplWhereClauseGenerator<'g, 's> {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        let mut punctuator = Punctuator::new(
            stream,
            syn::token::Where(self.span),
            syn::token::Comma(self.span),
            EmptyToken,
        );

        // The where clause looks quite different depending on what kind of Deps is used in the function.
        match &self.trait_dependency_mode {
            TraitDependencyMode::Generic(_) => {
                // Self bounds

                let has_bounds = self
                    .output_fns
                    .iter()
                    .any(|output_fn| match &output_fn.deps {
                        FnDeps::Generic { trait_bounds, .. } => !trait_bounds.is_empty(),
                        _ => false,
                    });

                if has_bounds {
                    punctuator.push_fn(|stream| {
                        let mut bound_punctuator = Punctuator::new(
                            stream,
                            TokenPair(
                                syn::token::SelfType(self.span),
                                syn::token::Colon(self.span),
                            ),
                            syn::token::Add(self.span),
                            EmptyToken,
                        );

                        for output_fn in self.output_fns {
                            if let FnDeps::Generic { trait_bounds, .. } = &output_fn.deps {
                                for bound in trait_bounds {
                                    bound_punctuator.push(bound);
                                }
                            }
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
