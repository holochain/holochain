use darling::FromMeta;
use darling::ToTokens;
use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;
use syn::visit;
use syn::visit::Visit;
use syn::AttributeArgs;
use syn::Item;
use syn::ItemEnum;
use syn::Variant;

use crate::util::ignore_enum_data;

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

    // TODO: Handle case for nested?
    let nested = match MacroArgs::from_list(&attr_args) {
        Ok(a) => a.nested,
        Err(e) => abort!(ident, "not sure {:?}", e),
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
    // FIXME: Check this overflow
    let variant_len = variants.len() as u8;

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
                let index = match u8::try_from(index) {
                    Ok(i) => i,
                    Err(_) => abort!(
                        ident,
                        "Enum cannot be longer then 256 variants.";
                        help = "Make your enum with less then 256 variants"
                    ),
                };
                let ignore = ignore_enum_data(&fields);
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

        impl #ident {
            pub fn len() -> u8 {
                #variant_len
            }
        }
    }
}

fn nesting(
    ident: &syn::Ident,
    variants: &Punctuated<Variant, syn::token::Comma>,
) -> proc_macro2::TokenStream {
    variants
        .iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                let nested_field_lens: proc_macro2::TokenStream = match fields {
                    syn::Fields::Named(syn::FieldsNamed { named, .. }) => named.iter()
                     .map(|syn::Field{ty, ..}| quote::quote! {#ty::len() + }).collect(),
                    syn::Fields::Unnamed(syn::FieldsUnnamed { unnamed, .. }) => unnamed.iter()
                     .map(|syn::Field{ty, ..}| quote::quote! {#ty::len() + }).collect(),
                    syn::Fields::Unit => quote::quote! {1u8 + },
                };
                // dbg!(&nested_field);
                // let inner_variants: proc_macro2::TokenStream = match nested_field {
                //     Some(syn::Field {
                //         ident: inner_ident, ty, ..
                //     }) => match inner_ident {
                //         Some(inner_ident) => {
                //             quote::quote! {#ty::len() => {
                //                 let i = count + LocalZomeTypeId::from(#inner_ident).0;
                //                 LocalZomeTypeId(i)
                //             },}
                //         }
                //         None => todo!(),
                //     },
                //     None => todo!(),
                // };
                // if inner_variants.is_empty() {
                //     let i = inc_index(ident, &mut index);
                //     let ignore = ignore_enum_data(&fields);
                //     quote::quote! {#ident::#v_ident #ignore => LocalZomeTypeId(#i),}
                // } else {
                //     inner_variants.into_iter().collect()
                // }
                nested_field_lens
            },
        )
        .collect()
}

fn inc_index(ident: &syn::Ident, index: &mut u8, amount: u8) -> u8 {
    let i = *index;
    match index.checked_add(amount) {
        Some(next) => {
            *index = next;
        }
        None => abort!(
            ident,
            "Enum cannot be longer then 256 variants.";
            help = "Make your enum with less then 256 variants"
        ),
    };
    i
}
