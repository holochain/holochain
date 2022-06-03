use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;
use syn::AttributeArgs;
use syn::Item;
use syn::ItemEnum;
use syn::Variant;

use crate::util::get_single_tuple_variant;
use crate::util::ignore_enum_data;
use crate::util::index_to_u8;

#[derive(Debug, FromMeta)]
pub struct MacroArgs {
    #[darling(default)]
    nested: bool,
}

pub fn build(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let attr_args = parse_macro_input!(args as AttributeArgs);
    let (ident, variants) = match &input {
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => (ident, variants),
        r => {
            abort!(r, "The `to_local_types` macro can only be used on enums."; help = "Make this an enum.";)
        }
    };

    // Handle case for nested
    let nested = match MacroArgs::from_list(&attr_args) {
        Ok(a) => a.nested,
        Err(e) => abort!(e.span(), "{}", e),
    };

    let variant_to_index = if nested {
        nesting(ident, variants)
    } else {
        no_nesting(ident, variants)
    };

    let output = quote::quote! {
        #input

        #variant_to_index

        impl From<#ident> for LocalZomeTypeId {
            fn from(t: #ident) -> Self {
                Self::from(&t)
            }
        }

    };
    output.into()
}

fn no_nesting(
    ident: &syn::Ident,
    variants: &Punctuated<Variant, syn::token::Comma>,
) -> proc_macro2::TokenStream {
    let variant_len = index_to_u8(variants.len());

    let variant_to_index: proc_macro2::TokenStream = variants
        .iter()
        .enumerate()
        .map(
            |(
                index,
                syn::Variant {
                    ident: v_ident,
                    fields,
                    ..
                },
            )| {
                let index = index_to_u8(index);
                let ignore = ignore_enum_data(fields);
                quote::quote! {#ident::#v_ident #ignore => LocalZomeTypeId(#index),}
            },
        )
        .collect();

    quote::quote! {
        impl From<&#ident> for LocalZomeTypeId {
            fn from(t: &#ident) -> Self {
                match t {
                    #variant_to_index
                }
            }
        }

        impl EnumLen<#variant_len> for #ident {}

        impl #ident {
            pub const fn len() -> u8 {
                <Self as EnumLen<#variant_len>>::ENUM_LEN
            }
        }
    }
}

fn nesting(
    ident: &syn::Ident,
    variants: &Punctuated<Variant, syn::token::Comma>,
) -> proc_macro2::TokenStream {
    let inner_from: proc_macro2::TokenStream = variants
        .iter()
        .enumerate()
        .map(
            |(
                enum_index,
                syn::Variant {
                    ident: v_ident,
                    fields,
                    ..
                },
            )| {
                let enum_index = index_to_u8(enum_index);
                match fields {
                    syn::Fields::Named(syn::FieldsNamed { named, .. }) =>
                    match named.iter().next().and_then(|syn::Field{ident, ..}| ident.as_ref())
                    {
                        Some(inner_ident) => {
                            quote::quote! {
                                #ident::#v_ident { #inner_ident, ..}  => {
                                    Self(<#ident as EnumVariantLen<#enum_index>>::ENUM_VARIANT_START + Self::from(#inner_ident).0)
                                }
                            }
                        }
                        None => abort!(v_ident, "Struct style enum needs at least one field."),
                    }
                    syn::Fields::Unnamed(syn::FieldsUnnamed { .. }) => {
                        get_single_tuple_variant(v_ident, fields);
                        quote::quote! {
                            #ident::#v_ident (inner_ident)  => {
                                Self(<#ident as EnumVariantLen<#enum_index>>::ENUM_VARIANT_START + Self::from(inner_ident).0)
                            }
                        }
                    }
                    syn::Fields::Unit => {
                        quote::quote! {
                            #ident::#v_ident  => {
                                Self(<#ident as EnumVariantLen<#enum_index>>::ENUM_VARIANT_START)
                            }
                        }
                    },
                }
            },
        )
        .collect();
    let enum_variant_len: proc_macro2::TokenStream = variants
        .iter()
        .enumerate()
        .map(|(enum_index, syn::Variant { fields, .. })| {
            let enum_index = index_to_u8(enum_index);
            let nested_field = match fields {
                syn::Fields::Named(syn::FieldsNamed { named, .. }) => named.iter().next(),
                syn::Fields::Unnamed(syn::FieldsUnnamed { unnamed, .. }) => unnamed.iter().next(),
                syn::Fields::Unit => None,
            };
            let start = if enum_index == 0 {
                quote::quote! {0;}
            } else {
                quote::quote! {<Self as EnumVariantLen<{#enum_index - 1}>>::ENUM_VARIANT_LEN;}
            };
            let inner_len = match nested_field {
                Some(syn::Field {
                    ty: syn::Type::Path(syn::TypePath { path, .. }),
                    ..
                }) => {
                    quote::quote! {
                            const ENUM_VARIANT_INNER_LEN: u8 = #path::ENUM_LEN;
                    }
                }
                None => quote::quote! {
                            const ENUM_VARIANT_INNER_LEN: u8 = 1;
                },
                _ => abort!(
                    nested_field,
                    "The field for this enum has an invalid inner type for this macro."
                ),
            };
            quote::quote! {
                impl EnumVariantLen<#enum_index> for #ident {
                    const ENUM_VARIANT_START: u8 = #start
                    #inner_len
                }
            }
        })
        .collect();

    let i = variants
        .len()
        .checked_sub(1)
        .unwrap_or_else(|| abort!(ident, "Enum must have at least one variant"));
    let last_variant_index = index_to_u8(i);

    quote::quote! {
        impl EnumLen<{<#ident as EnumVariantLen<#last_variant_index>>::ENUM_VARIANT_LEN}> for #ident {}

        #enum_variant_len

        impl From<&#ident> for LocalZomeTypeId {
            fn from(n: &#ident) -> Self {
                match n {
                    #inner_from
                }
            }
        }

        impl #ident {
            pub const fn len() -> u8 {
                <Self as EnumLen<{<#ident as EnumVariantLen<#last_variant_index>>::ENUM_VARIANT_LEN}>>::ENUM_LEN
            }
        }
    }
}
