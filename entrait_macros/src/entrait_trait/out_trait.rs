use crate::analyze_generics::TraitFn;

pub struct OutTrait {
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub trait_token: syn::token::Trait,
    pub ident: syn::Ident,
    pub methods: Vec<TraitFn>,
}

pub fn analyze_trait<'i>(item_trait: &'i syn::ItemTrait) {}
