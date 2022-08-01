pub struct CrateIdents {
    pub entrait: syn::Ident,
}

impl CrateIdents {
    pub fn new(span: proc_macro2::Span) -> Self {
        Self {
            entrait: syn::Ident::new("entrait", span),
        }
    }
}

pub struct GenericIdents<'c> {
    pub crate_idents: &'c CrateIdents,

    /// "Impl"
    pub impl_self: syn::Ident,

    /// The "T" in `Impl<T>`
    pub impl_t: syn::Ident,
}

impl<'c> GenericIdents<'c> {
    pub fn new(crate_idents: &'c CrateIdents, span: proc_macro2::Span) -> Self {
        Self {
            crate_idents,
            impl_self: syn::Ident::new("Impl", span),
            impl_t: syn::Ident::new("EntraitT", span),
        }
    }
}
