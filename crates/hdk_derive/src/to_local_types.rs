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
/// Type for parsing the `#[hdk_to_local_types(nested = true)]`
/// attribute into. Defaults to false.
pub struct MacroArgs {
    #[darling(default)]
    nested: bool,
}

pub fn build(args: TokenStream, input: TokenStream) -> TokenStream {
    // Parse input and attributes.
    let input = parse_macro_input!(input as Item);
    let attr_args = parse_macro_input!(args as AttributeArgs);

    // Check the input is an enum.
    let (ident, variants) = match &input {
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => (ident, variants),
        r => {
            abort!(r, "The `to_local_types` macro can only be used on enums."; help = "Make this an enum.";)
        }
    };

    // Check if this is the nested version or not.
    let nested = match MacroArgs::from_list(&attr_args) {
        Ok(a) => a.nested,
        Err(e) => abort!(e.span(), "{}", e),
    };

    // Generate the output for mapping between variants
    // and local types.
    let variant_to_index = if nested {
        nesting(ident, variants)
    } else {
        no_nesting(ident, variants)
    };

    let output = quote::quote! {
        #input

        #variant_to_index

        // Add the owned from.
        impl From<#ident> for LocalZomeTypeId {
            fn from(t: #ident) -> Self {
                Self::from(&t)
            }
        }

    };
    output.into()
}

/// Generates output for an enum while ignoring nested enums.
fn no_nesting(
    ident: &syn::Ident,
    variants: &Punctuated<Variant, syn::token::Comma>,
) -> proc_macro2::TokenStream {
    // Get the total number of variants for this enum.
    let variant_len = index_to_u8(variants.len());

    // Create match branches for each variant that map to the `LocalZomeTypeId`.
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
                // Get the identifier of this variant as a u8.
                let index = index_to_u8(index);
                // Generate output that ignores any nested data.
                let ignore = ignore_enum_data(fields);
                // Generate the match branch.
                quote::quote! {#ident::#v_ident #ignore => LocalZomeTypeId(#index),}
            },
        )
        .collect();

    quote::quote! {
        impl From<&#ident> for LocalZomeTypeId {
            fn from(t: &#ident) -> Self {
                match t {
                    // Use the generated match branches here.
                    #variant_to_index
                }
            }
        }

        // Implement the `EnumLen` trait that sets the const based on the
        // number of variants.
        impl EnumLen for #ident {
            const ENUM_LEN: u8 = #variant_len;
        }

        // Implement a const len function to
        // give this enum a length.
        impl #ident {
            pub const fn len() -> u8 {
                <Self as EnumLen>::ENUM_LEN
            }
        }
    }
}

/// Generates output for an enum while ignoring nested enums.
fn nesting(
    ident: &syn::Ident,
    variants: &Punctuated<Variant, syn::token::Comma>,
) -> proc_macro2::TokenStream {
    // Generate inner match arms for `impl From<&Self> for LocalZomeTypeId`
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
                // Get this variants index as u8.
                let enum_index = index_to_u8(enum_index);

                // Map inner fields to a `LocalZomeTypeId`.
                match fields {
                    syn::Fields::Named(syn::FieldsNamed { named, .. }) =>
                    // Get the first fields identifier.
                    match named.iter().next().and_then(|syn::Field{ident, ..}| ident.as_ref())
                    {
                        Some(inner_ident) => {
                            // This arms `LocalZomeTypeId` is the starting point of this variant
                            // plus the fields `LocalZomeTypeId`.
                            quote::quote! {
                                #ident::#v_ident { #inner_ident, ..}  => {
                                    Self(<#ident as EnumVariantLen<#enum_index>>::ENUM_VARIANT_START + Self::from(#inner_ident).0)
                                }
                            }
                        }
                        None => abort!(v_ident, "Struct style enum needs at least one field."),
                    }
                    syn::Fields::Unnamed(syn::FieldsUnnamed { .. }) => {
                        // Check there is only a single tuple variant.
                        get_single_tuple_variant(v_ident, fields);
                        // This arms `LocalZomeTypeId` is the starting point of this variant
                        // plus the fields `LocalZomeTypeId`.
                        quote::quote! {
                            #ident::#v_ident (inner_ident)  => {
                                Self(<#ident as EnumVariantLen<#enum_index>>::ENUM_VARIANT_START + Self::from(inner_ident).0)
                            }
                        }
                    }
                    syn::Fields::Unit => {
                        // A unit variant is simply the starting variant point.
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

    // Implement the `EnumVariantLen` trait for each variant.
    let enum_variant_len: proc_macro2::TokenStream = variants
        .iter()
        .enumerate()
        .map(|(enum_index, syn::Variant { fields, .. })| {
            // Get this variants index as a u8.
            let enum_index = index_to_u8(enum_index);

            // Get the nested field if there is one.
            let nested_field = match fields {
                syn::Fields::Named(syn::FieldsNamed { named, .. }) => named.iter().next(),
                syn::Fields::Unnamed(syn::FieldsUnnamed { unnamed, .. }) => unnamed.iter().next(),
                syn::Fields::Unit => None,
            };

            // Set the start of this variants length.
            let start = if enum_index == 0 {
                quote::quote! {0;}
            } else {
                // The start for 1.. variants is the length as of n - 1.
                quote::quote! {<Self as EnumVariantLen<{#enum_index - 1}>>::ENUM_VARIANT_LEN;}
            };
            let inner_len = match nested_field {
                Some(syn::Field {
                    ty: syn::Type::Path(syn::TypePath { path, .. }),
                    ..
                }) => {
                    // For a nested field the inner length is the inner fields
                    // EnumLen::ENUM_LEN.
                    quote::quote! {
                            const ENUM_VARIANT_INNER_LEN: u8 = #path::ENUM_LEN;
                    }
                }
                // Unit enums have an inner length of one.
                None => quote::quote! {
                            const ENUM_VARIANT_INNER_LEN: u8 = 1;
                },
                _ => abort!(
                    nested_field,
                    "The field for this enum has an invalid inner type for this macro."
                ),
            };

            // Note that `ENUM_VARIANT_LEN` is the total length up to this variant
            // where as `ENUM_VARIANT_INNER_LEN` is the actual length of the inner field.

            // Implement the `EnumVariantLen` for this variant.
            quote::quote! {
                impl EnumVariantLen<#enum_index> for #ident {
                    const ENUM_VARIANT_START: u8 = #start
                    #inner_len
                }
            }
        })
        .collect();

    // Get the index of the last variant (len - 1) as a u8.
    let i = variants
        .len()
        .checked_sub(1)
        .unwrap_or_else(|| abort!(ident, "Enum must have at least one variant"));
    let last_variant_index = index_to_u8(i);

    quote::quote! {
        // The overall length of this enum is the variant length
        // as of the last variant.
        impl EnumLen for #ident {
            const ENUM_LEN: u8 = <#ident as EnumVariantLen<#last_variant_index>>::ENUM_VARIANT_LEN;
        }

        #enum_variant_len

        impl From<&#ident> for LocalZomeTypeId {
            fn from(n: &#ident) -> Self {
                match n {
                    #inner_from
                }
            }
        }

        // Impl simple const len helper.
        impl #ident {
            pub const fn len() -> u8 {
                <Self as EnumLen>::ENUM_LEN
            }
        }
    }
}
