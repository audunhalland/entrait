//!
//! inputs to procedural macros
//!
//!
//!

use crate::{signature::InputSig, token_util::push_tokens};

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

pub enum FnInputMode<'a> {
    SingleFn(&'a syn::Ident),
    Module(&'a syn::Ident),
    ImplBlock(&'a syn::Type),
    RawTrait(LiteralAttrs<'a>),
}

pub struct LiteralAttrs<'a>(pub &'a [syn::Attribute]);

impl<'a> ToTokens for LiteralAttrs<'a> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        for attr in self.0 {
            attr.to_tokens(stream);
        }
    }
}

pub enum Input {
    Fn(InputFn),
    Trait(syn::ItemTrait),
    Mod(InputMod),
    Impl(InputImpl),
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse()?;

        let unsafety: Option<syn::token::Unsafe> = input.parse()?;
        let auto_token: Option<syn::token::Auto> = input.parse()?;

        // BUG (In theory): missing and "auto" traits
        if input.peek(syn::token::Trait) {
            let item_trait: syn::ItemTrait = input.parse()?;

            Ok(Input::Trait(syn::ItemTrait {
                attrs,
                vis,
                unsafety,
                auto_token,
                ..item_trait
            }))
        } else if input.peek(syn::token::Impl) {
            disallow_token(auto_token)?;
            Ok(Input::Impl(parse_impl(attrs, unsafety, input)?))
        } else if input.peek(syn::token::Mod) {
            disallow_token(unsafety)?;
            disallow_token(auto_token)?;
            Ok(Input::Mod(parse_mod(attrs, vis, input)?))
        } else {
            let fn_sig: syn::Signature = input.parse()?;
            let fn_body = input.parse()?;

            Ok(Input::Fn(InputFn {
                fn_attrs: attrs,
                fn_vis: vis,
                fn_sig,
                fn_body,
            }))
        }
    }
}

pub struct InputFn {
    pub fn_attrs: Vec<syn::Attribute>,
    pub fn_vis: syn::Visibility,
    pub fn_sig: syn::Signature,
    // don't try to parse fn_body, just pass through the tokens:
    pub fn_body: proc_macro2::TokenStream,
}

impl InputFn {
    pub fn input_sig(&self) -> InputSig<'_> {
        InputSig::new(&self.fn_sig)
    }
}

pub struct InputMod {
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub mod_token: syn::token::Mod,
    pub ident: syn::Ident,
    pub brace_token: syn::token::Brace,
    pub items: Vec<ModItem>,
}

impl Parse for InputMod {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse()?;
        parse_mod(attrs, vis, input)
    }
}

impl ToTokens for InputMod {
    fn to_tokens(&self, stream: &mut TokenStream) {
        for attr in &self.attrs {
            push_tokens!(stream, attr);
        }
        push_tokens!(stream, self.vis, self.mod_token, self.ident);
        self.brace_token.surround(stream, |stream| {
            for item in &self.items {
                item.to_tokens(stream);
            }
        });
    }
}

pub enum ModItem {
    PubFn(Box<InputFn>),
    Unknown(ItemUnknown),
}

impl ModItem {
    // We include all functions that have a visibility keyword into the trait
    pub fn filter_pub_fn(&self) -> Option<&InputFn> {
        match self {
            Self::PubFn(input_fn) => Some(input_fn),
            _ => None,
        }
    }
}

impl ToTokens for ModItem {
    fn to_tokens(&self, stream: &mut TokenStream) {
        match self {
            ModItem::PubFn(input_fn) => {
                let InputFn {
                    fn_attrs,
                    fn_vis,
                    fn_sig,
                    fn_body,
                } = input_fn.as_ref();
                for attr in fn_attrs {
                    push_tokens!(stream, attr);
                }
                push_tokens!(stream, fn_vis, fn_sig, fn_body);
            }
            ModItem::Unknown(unknown) => {
                unknown.to_tokens(stream);
            }
        }
    }
}

pub struct DeriveImplTraitPath(pub syn::Path);

/// An impl block
/// Note: No support for generics
pub struct InputImpl {
    pub attrs: Vec<syn::Attribute>,
    pub unsafety: Option<syn::token::Unsafe>,
    pub impl_token: syn::token::Impl,
    pub trait_path: syn::Path,
    pub for_token: syn::token::For,
    pub self_ty: syn::Type,
    pub brace_token: syn::token::Brace,
    pub items: Vec<ImplItem>,
}

pub enum ImplItem {
    Fn(Box<InputFn>),
    Unknown(ItemUnknown),
}

impl ImplItem {
    // We include all functions that have a visibility keyword into the trait
    pub fn filter_fn(&self) -> Option<&InputFn> {
        match self {
            Self::Fn(input_fn) => Some(input_fn),
            _ => None,
        }
    }
}

impl ToTokens for ImplItem {
    fn to_tokens(&self, stream: &mut TokenStream) {
        match self {
            ImplItem::Fn(input_fn) => {
                let InputFn {
                    fn_attrs,
                    fn_vis,
                    fn_sig,
                    fn_body,
                } = input_fn.as_ref();
                for attr in fn_attrs {
                    push_tokens!(stream, attr);
                }
                push_tokens!(stream, fn_vis, fn_sig, fn_body);
            }
            ImplItem::Unknown(unknown) => {
                unknown.to_tokens(stream);
            }
        }
    }
}

pub struct ItemUnknown {
    attrs: Vec<syn::Attribute>,
    vis: syn::Visibility,
    tokens: TokenStream,
}

impl ToTokens for ItemUnknown {
    fn to_tokens(&self, stream: &mut TokenStream) {
        for attr in &self.attrs {
            attr.to_tokens(stream);
        }
        push_tokens!(stream, self.vis, self.tokens);
    }
}

fn parse_mod(
    attrs: Vec<syn::Attribute>,
    vis: syn::Visibility,
    input: ParseStream,
) -> syn::Result<InputMod> {
    let mod_token = input.parse()?;
    let ident = input.parse()?;

    let lookahead = input.lookahead1();
    if lookahead.peek(syn::token::Brace) {
        let content;
        let brace_token = syn::braced!(content in input);

        let mut items = vec![];

        while !content.is_empty() {
            items.push(content.parse()?);
        }

        Ok(InputMod {
            attrs,
            vis,
            mod_token,
            ident,
            brace_token,
            items,
        })
    } else {
        Err(lookahead.error())
    }
}

impl Parse for ModItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;

        let vis: syn::Visibility = input.parse()?;
        let unknown = input.fork();

        if peek_pub_fn(input, &vis) {
            let sig: syn::Signature = input.parse()?;
            if input.peek(syn::token::Semi) {
                let _ = input.parse::<syn::token::Semi>()?;
                Ok(ModItem::Unknown(ItemUnknown {
                    attrs,
                    vis,
                    tokens: verbatim_between(unknown, input),
                }))
            } else {
                let fn_body = parse_matched_braces_or_ending_semi(input)?;
                Ok(ModItem::PubFn(Box::new(InputFn {
                    fn_attrs: attrs,
                    fn_vis: vis,
                    fn_sig: sig,
                    fn_body,
                })))
            }
        } else {
            let tokens = parse_matched_braces_or_ending_semi(input)?;
            Ok(ModItem::Unknown(ItemUnknown { attrs, vis, tokens }))
        }
    }
}

fn parse_impl(
    attrs: Vec<syn::Attribute>,
    unsafety: Option<syn::token::Unsafe>,
    input: ParseStream,
) -> syn::Result<InputImpl> {
    let impl_token = input.parse()?;
    let trait_path = input.parse()?;
    let for_token = input.parse()?;
    let self_ty = input.parse()?;

    let lookahead = input.lookahead1();
    if lookahead.peek(syn::token::Brace) {
        let content;
        let brace_token = syn::braced!(content in input);

        let mut items = vec![];

        while !content.is_empty() {
            items.push(content.parse()?);
        }

        Ok(InputImpl {
            attrs,
            unsafety,
            impl_token,
            trait_path,
            for_token,
            self_ty,
            brace_token,
            items,
        })
    } else {
        Err(lookahead.error())
    }
}

impl Parse for ImplItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;

        let vis: syn::Visibility = input.parse()?;
        let unknown = input.fork();

        if peek_fn(input) {
            let sig: syn::Signature = input.parse()?;
            if input.peek(syn::token::Semi) {
                let _ = input.parse::<syn::token::Semi>()?;
                Ok(ImplItem::Unknown(ItemUnknown {
                    attrs,
                    vis,
                    tokens: verbatim_between(unknown, input),
                }))
            } else {
                let fn_body = parse_matched_braces_or_ending_semi(input)?;
                Ok(ImplItem::Fn(Box::new(InputFn {
                    fn_attrs: attrs,
                    fn_vis: vis,
                    fn_sig: sig,
                    fn_body,
                })))
            }
        } else {
            let tokens = parse_matched_braces_or_ending_semi(input)?;
            Ok(ImplItem::Unknown(ItemUnknown { attrs, vis, tokens }))
        }
    }
}

impl Parse for DeriveImplTraitPath {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        let _ = syn::parenthesized!(content in input);

        let path = content.parse()?;

        Ok(DeriveImplTraitPath(path))
    }
}

fn peek_pub_fn(input: ParseStream, vis: &syn::Visibility) -> bool {
    if let syn::Visibility::Inherited = vis {
        // 'private' functions aren't interesting
        return false;
    }
    peek_fn(input)
}

fn peek_fn(input: ParseStream) -> bool {
    if input.peek(syn::token::Fn) {
        return true;
    }

    let fork = input.fork();
    fork.parse::<Option<syn::token::Const>>().is_ok()
        && fork.parse::<Option<syn::token::Async>>().is_ok()
        && fork.parse::<Option<syn::token::Unsafe>>().is_ok()
        && fork.parse::<Option<syn::Abi>>().is_ok()
        && fork.peek(syn::token::Fn)
}

fn verbatim_between<'a>(begin: syn::parse::ParseBuffer<'a>, end: ParseStream<'a>) -> TokenStream {
    let end = end.cursor();
    let mut cursor = begin.cursor();
    let mut tokens = TokenStream::new();
    while cursor != end {
        let (tt, next) = cursor.token_tree().unwrap();
        tokens.extend(std::iter::once(tt));
        cursor = next;
    }
    tokens
}

fn parse_matched_braces_or_ending_semi(input: ParseStream) -> syn::Result<TokenStream> {
    let mut tokens = input.step(|cursor| {
        let mut tokens = TokenStream::new();

        use proc_macro2::Delimiter;
        use proc_macro2::TokenTree;

        let mut rest = *cursor;

        while let Some((tt, next)) = rest.token_tree() {
            match &tt {
                TokenTree::Group(group) => {
                    let is_brace = group.delimiter() == Delimiter::Brace;
                    tokens.extend(std::iter::once(tt));
                    if is_brace {
                        return Ok((tokens, next));
                    }
                }
                TokenTree::Punct(punct) => {
                    let is_semi = punct.as_char() == ';';
                    tokens.extend(std::iter::once(tt));
                    if is_semi {
                        return Ok((tokens, next));
                    }
                }
                _ => {
                    tokens.extend(std::iter::once(tt));
                }
            }
            rest = next;
        }
        Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "Read past the end",
        ))
    })?;

    while input.peek(syn::token::Semi) {
        let semi = input.parse::<syn::token::Semi>()?;
        semi.to_tokens(&mut tokens);
    }

    Ok(tokens)
}

fn disallow_token<T: Spanned>(token: Option<T>) -> syn::Result<()> {
    if let Some(token) = token {
        Err(syn::Error::new(token.span(), "Not allowed here"))
    } else {
        Ok(())
    }
}
