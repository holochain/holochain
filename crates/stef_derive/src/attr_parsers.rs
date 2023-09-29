use quote::ToTokens;
use syn::{parse::ParseStream, spanned::Spanned, Expr, Pat, Token};

#[derive(Default)]
pub struct Options {
    pub _parameterized: Option<syn::Path>,
    pub wrappers: Vec<syn::Type>,
    pub fuzzing: bool,
    pub recording: bool,
}

impl syn::parse::Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut this = Self::default();

        while !input.is_empty() {
            let key: syn::Path = input.parse()?;
            if key.is_ident("wrapper") {
                let content;
                syn::parenthesized!(content in input);
                let struct_name: syn::Type = content.parse()?;
                this.wrappers.push(struct_name);
            } else if key.is_ident("fuzzing") {
                this.fuzzing = true;
            } else if key.is_ident("recording") {
                this.recording = true;
            }
            let _: syn::Result<syn::Token![,]> = input.parse();
        }

        Ok(this)
    }
}

pub struct MatchPat {
    pub forward_pat: Pat,
    pub forward_expr: Expr,
    pub backward_pat: Pat,
    pub backward_expr: Expr,
}

impl syn::parse::Parse for MatchPat {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let forward_pat = Pat::parse_single(input)?;
        let backward_expr = syn::parse(forward_pat.to_token_stream().into())?;

        #[allow(clippy::nonminimal_bool)]
        if true
            && input.parse::<Token!(<)>().is_ok()
            && input.parse::<Token!(=)>().is_ok()
            && input.parse::<Token!(>)>().is_ok()
        {
            let backward_pat = Pat::parse_single(input)?;
            let forward_expr = syn::parse(backward_pat.to_token_stream().into())?;
            Ok(Self {
                forward_pat,
                forward_expr,
                backward_pat,
                backward_expr,
            })
        } else {
            Err(syn::Error::new(
                forward_pat.span(),
                "Expected <=> in `matches()`",
            ))
        }
    }
}
