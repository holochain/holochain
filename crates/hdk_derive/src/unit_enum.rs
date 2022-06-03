use darling::util::PathList;
use darling::FromMeta;
use proc_macro::TokenStream;

use darling::FromDeriveInput;
use darling::FromVariant;
use proc_macro_error::abort;
use syn::parse_macro_input;

use crate::util::get_unit_ident;

#[derive(FromVariant)]
struct VarOpts {
    ident: syn::Ident,
    fields: darling::ast::Fields<darling::util::Ignored>,
}

#[derive(FromMeta)]
struct EnumName(syn::Ident);

#[derive(FromDeriveInput)]
#[darling(attributes(unit_attrs), forward_attrs(unit_enum))]
struct Opts {
    ident: syn::Ident,
    attrs: Vec<syn::Attribute>,
    data: darling::ast::Data<VarOpts, darling::util::Ignored>,
    #[darling(default)]
    forward: PathList,
}

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    let opts = match Opts::from_derive_input(&input) {
        Ok(o) => o,
        Err(e) => abort!(e.span(), e),
    };
    let Opts {
        ident,
        attrs,
        data,
        forward,
    } = opts;

    let variants = match data {
        darling::ast::Data::Enum(variants) => variants,
        _ => abort!(ident, "UnitEnum can only be derived on Enums"),
    };

    let unit_ident = get_unit_ident(&attrs);

    let units: proc_macro2::TokenStream = variants
        .iter()
        .map(|VarOpts { ident, .. }| quote::quote! {#ident,})
        .collect();

    let units_match: proc_macro2::TokenStream = variants
        .iter()
        .map(
            |VarOpts {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                let enum_style = match fields.style {
                    darling::ast::Style::Struct => quote::quote! {{..}},
                    darling::ast::Style::Unit => quote::quote! {},
                    darling::ast::Style::Tuple => quote::quote! {(_)},
                };
                quote::quote! {#ident::#v_ident #enum_style => #unit_ident::#v_ident,}
            },
        )
        .collect();

    let unit_attrs: proc_macro2::TokenStream = forward
        .iter()
        .cloned()
        .map(|a| quote::quote! {#[#a] })
        .collect();

    let output = quote::quote! {
        impl UnitEnum for #ident {
            type Unit = #unit_ident;

            fn to_unit(&self) -> Self::Unit {
                match self {
                    #units_match
                }
            }
        }

        #unit_attrs
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum #unit_ident {
            #units
        }

        impl #unit_ident {
            pub fn iter() -> impl Iterator<Item = Self> {
                use #unit_ident::*;
                [#units].into_iter()
            }
        }
    };
    output.into()
}
