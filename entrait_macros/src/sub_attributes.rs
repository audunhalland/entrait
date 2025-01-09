//! The other attributes specified _under_ entrait

use quote::ToTokens;

#[derive(Clone, Copy)]
pub enum SubAttribute<'t> {
    AsyncTrait(&'t syn::Attribute),
    Other(&'t syn::Attribute),
    Automock(&'t syn::Attribute),
}

pub fn analyze_sub_attributes(attributes: &[syn::Attribute]) -> Vec<SubAttribute<'_>> {
    attributes
        .iter()
        .map(|attribute| {
            let last_segment = attribute.path().segments.last();
            let ident = last_segment.map(|segment| segment.ident.to_string());

            if let Some(ident) = ident {
                match ident.as_str() {
                    "async_trait" => SubAttribute::AsyncTrait(attribute),
                    "automock" => SubAttribute::Automock(attribute),
                    _ => SubAttribute::Other(attribute),
                }
            } else {
                SubAttribute::Other(attribute)
            }
        })
        .collect()
}

pub fn contains_async_trait(sub_attributes: &[SubAttribute]) -> bool {
    sub_attributes
        .iter()
        .any(|sub_attributes| matches!(sub_attributes, SubAttribute::AsyncTrait(_)))
}

impl ToTokens for SubAttribute<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::AsyncTrait(attr) => attr.to_tokens(tokens),
            Self::Automock(attr) => attr.to_tokens(tokens),
            Self::Other(attr) => attr.to_tokens(tokens),
        }
    }
}
