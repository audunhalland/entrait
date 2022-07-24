pub struct Generics {
    pub deps: Deps,
    pub trait_generics: syn::Generics,
}

impl Generics {
    pub fn new(deps: Deps, trait_generics: syn::Generics) -> Self {
        Self {
            deps,
            trait_generics,
        }
    }

    pub fn params_generator(&self) -> ParamsGenerator {
        ParamsGenerator {
            trait_generics: &self.trait_generics,
            impl_t: match &self.deps {
                Deps::Generic { idents, .. } => Some(&idents.impl_t),
                Deps::NoDeps { idents } => Some(&idents.impl_t),
                Deps::Concrete(_) => None,
            },
        }
    }

    pub fn arguments_generator(&self) -> ArgumentsGenerator {
        ArgumentsGenerator {
            trait_generics: &self.trait_generics,
        }
    }

    pub fn where_clause_generator(&self) -> WhereClauseGenerator {
        WhereClauseGenerator {
            trait_generics: &self.trait_generics,
            impl_predicates: Default::default(),
        }
    }
}

pub enum Deps {
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

pub struct GenericIdents {
    /// "entrait"
    pub entrait_crate: syn::Ident,

    /// "Impl"
    pub impl_self: syn::Ident,

    /// The "T" in `Impl<T>`
    pub impl_t: syn::Ident, //
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
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let span = self.1;
        syn::token::Colon2(span).to_tokens(tokens);
        self.0.entrait_crate.to_tokens(tokens);
        syn::token::Colon2(span).to_tokens(tokens);
        self.0.impl_self.to_tokens(tokens);
        syn::token::Lt(span).to_tokens(tokens);
        self.0.impl_t.to_tokens(tokens);
        syn::token::Gt(span).to_tokens(tokens);
    }
}

// Params as in impl<..Param>
pub struct ParamsGenerator<'g> {
    trait_generics: &'g syn::Generics,
    impl_t: Option<&'g syn::Ident>,
}

impl<'g> quote::ToTokens for ParamsGenerator<'g> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let has_trait_generics = !self.trait_generics.params.is_empty();

        if !has_trait_generics && self.impl_t.is_none() {
            return;
        }

        syn::token::Lt::default().to_tokens(tokens);
        if let Some(impl_t) = &self.impl_t {
            impl_t.to_tokens(tokens);

            if has_trait_generics {
                syn::token::Comma::default().to_tokens(tokens);
            }
        }
        self.trait_generics.params.to_tokens(tokens);
        syn::token::Gt::default().to_tokens(tokens);
    }
}

// Args as in impl<..Param> T for U<..Arg>
pub struct ArgumentsGenerator<'g> {
    trait_generics: &'g syn::Generics,
}

impl<'g> quote::ToTokens for ArgumentsGenerator<'g> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let has_trait_generics = !self.trait_generics.params.is_empty();

        if !has_trait_generics {
            return;
        }

        syn::token::Lt::default().to_tokens(tokens);
        for pair in self.trait_generics.params.pairs() {
            match pair.value() {
                syn::GenericParam::Type(type_param) => {
                    type_param.ident.to_tokens(tokens);
                }
                syn::GenericParam::Lifetime(lifetime_def) => {
                    lifetime_def.lifetime.to_tokens(tokens);
                }
                syn::GenericParam::Const(const_param) => {
                    const_param.ident.to_tokens(tokens);
                }
            }
            pair.punct().to_tokens(tokens);
        }

        syn::token::Gt::default().to_tokens(tokens);
    }
}

/// Join where clauses from the input function and the required ones for Impl<T>
pub struct WhereClauseGenerator<'g> {
    trait_generics: &'g syn::Generics,
    impl_predicates: syn::punctuated::Punctuated<syn::WherePredicate, syn::token::Comma>,
}

impl<'g> WhereClauseGenerator<'g> {
    pub fn push_impl_predicate(&mut self, predicate: syn::WherePredicate) {
        self.impl_predicates.push(predicate);
    }
}

impl<'g> quote::ToTokens for WhereClauseGenerator<'g> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let has_impl_preds = !self.impl_predicates.is_empty();
        if self.trait_generics.where_clause.is_none() && !has_impl_preds {
            return;
        }

        syn::token::Where::default().to_tokens(tokens);
        if let Some(where_clause) = &self.trait_generics.where_clause {
            let mut trailing_comma = false;
            for pair in where_clause.predicates.pairs() {
                pair.value().to_tokens(tokens);
                trailing_comma = match pair.punct() {
                    Some(punct) => {
                        punct.to_tokens(tokens);
                        true
                    }
                    None => false,
                }
            }

            if has_impl_preds && !trailing_comma {
                syn::token::Comma::default().to_tokens(tokens);
            }
        }

        self.impl_predicates.to_tokens(tokens);
    }
}
