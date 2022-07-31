use crate::token_util::{push_tokens, Punctuator};

pub struct FnGenerics {
    pub deps: FnDeps,
    pub trait_generics: syn::Generics, // TODO: Remove
}

pub struct UseAssociatedFuture(pub bool);

impl FnGenerics {
    pub fn new(deps: FnDeps, trait_generics: syn::Generics) -> Self {
        Self {
            deps,
            trait_generics,
        }
    }

    pub fn params_generator(&self, use_associated_future: UseAssociatedFuture) -> ParamsGenerator {
        ParamsGenerator {
            params: &self.trait_generics.params,
            impl_t: match &self.deps {
                FnDeps::Generic { idents, .. } => Some(&idents.impl_t),
                FnDeps::NoDeps { idents } => Some(&idents.impl_t),
                FnDeps::Concrete(_) => None,
            },
            use_associated_future,
        }
    }

    pub fn arguments_generator(&self) -> ArgumentsGenerator {
        ArgumentsGenerator {
            params: &self.trait_generics.params,
        }
    }
}

pub enum FnDeps {
    Generic {
        generic_param: Option<syn::Ident>,
        trait_bounds: Vec<syn::TypeParamBound>,
        idents: GenericIdents,
    },
    Concrete(Box<syn::Type>),
    NoDeps {
        idents: GenericIdents,
    },
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

    pub fn trait_where_clause(&self) -> WhereClauseGenerator<'_> {
        WhereClauseGenerator {
            where_predicates: &self.where_predicates,
        }
    }

    pub fn impl_params<'s, 'i>(
        &'i self,
        generic_idents: Option<&'i GenericIdents>,
        use_associated_future: UseAssociatedFuture,
    ) -> ParamsGenerator<'_> {
        ParamsGenerator {
            params: &self.params,
            impl_t: generic_idents.map(|idents| &idents.impl_t),
            use_associated_future,
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

pub struct WhereClauseGenerator<'g> {
    where_predicates: &'g syn::punctuated::Punctuated<syn::WherePredicate, syn::token::Comma>,
}

impl<'g> quote::ToTokens for WhereClauseGenerator<'g> {
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
