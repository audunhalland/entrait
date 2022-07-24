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
