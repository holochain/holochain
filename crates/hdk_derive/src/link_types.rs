use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::AttributeArgs;
use syn::Item;
use syn::ItemEnum;

#[derive(Debug, FromMeta)]
/// Optional attribute for skipping `#[no_mangle].
/// Useful for testing.
pub struct MacroArgs {
    #[darling(default)]
    skip_no_mangle: bool,
}

pub fn build(attrs: TokenStream, input: TokenStream) -> TokenStream {
    // Parse the attributes and input.
    let attr_args = parse_macro_input!(attrs as AttributeArgs);
    let input = parse_macro_input!(input as Item);

    // Extract the enums ident and variants.
    let (ident, variants) = match &input {
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => (ident, variants),
        _ => abort!(input, "hdk_link_types can only be used on Enums"),
    };

    // Get all the variant idents.
    let units: proc_macro2::TokenStream = variants
        .iter()
        .map(|syn::Variant { ident, fields, .. }| {
            if !matches!(fields, syn::Fields::Unit) {
                abort!(ident, "hdk_link_types can only be used on Unit enums.");
            }
            quote::quote! {#ident,}
        })
        .collect();

    // Check no mangle attribute.
    let skip_no_mangle = match MacroArgs::from_list(&attr_args) {
        Ok(a) => a.skip_no_mangle,
        Err(e) => abort!(ident, "{}", e),
    };

    // Generate no mangle if needed.
    let no_mangle = if skip_no_mangle {
        quote::quote! {}
    } else {
        quote::quote! {#[no_mangle]}
    };

    let output = quote::quote! {
        // Add the required derives and attributes.
        #[hdk_to_global_link_types]
        #[hdk_to_local_types]
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
        #input

        // Add the extern function that says how many links this zome has.
        #no_mangle
        pub fn __num_link_types() -> u8 { #ident::len() }

        impl TryFrom<#ident> for LinkType {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Ok(Self(GlobalZomeTypeId::try_from(value)?.0))
            }
        }

        impl TryFrom<&#ident> for LinkType {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Ok(Self(GlobalZomeTypeId::try_from(value)?.0))
            }
        }

        impl TryFrom<#ident> for LinkTypeRange {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                let lt: LinkType = value.try_into()?;
                Ok(lt.into())
            }
        }

        impl TryFrom<&#ident> for LinkTypeRange {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                let lt: LinkType = value.try_into()?;
                Ok(lt.into())
            }
        }

        impl TryFrom<#ident> for LinkTypeRanges {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                let lt: LinkType = value.try_into()?;
                Ok(Self(vec![lt.into()]))
            }
        }

        impl TryFrom<&#ident> for LinkTypeRanges {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                let lt: LinkType = value.try_into()?;
                Ok(Self(vec![lt.into()]))
            }
        }

        // Implement the helper trait.
        impl LinkTypesHelper<{ #ident::len() as usize }> for #ident {
            fn iter() -> core::array::IntoIter<Self, { #ident::len() as usize }> {
                use #ident::*;
                [#units].into_iter()
            }
        }

        impl TryFrom<LocalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(value: LocalZomeTypeId) -> Result<Self, Self::Error> {
                Self::iter()
                    .find(|u| LocalZomeTypeId::from(*u) == value)
                    .ok_or_else(|| {
                        WasmError::Guest(format!(
                            "local index {:?} does not match any variant of {}",
                            value, stringify!(#ident)
                        ))
                    })
            }
        }

        impl TryFrom<&LocalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(value: &LocalZomeTypeId) -> Result<Self, Self::Error> {
                Self::try_from(*value)
            }
        }

        impl TryFrom<GlobalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(index: GlobalZomeTypeId) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.links.to_local_scope(index) {
                    Some(local_index) => Self::try_from(local_index),
                    _ => Err(WasmError::Guest(format!(
                        "global index {:?} does not map to any local scope for this zome",
                        index
                    ))),
                }
            }
        }

        impl TryFrom<&GlobalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(index: &GlobalZomeTypeId) -> Result<Self, Self::Error> {
                Self::try_from(*index)
            }
        }

        impl TryFrom<LinkType> for #ident {
            type Error = WasmError;
            fn try_from(index: LinkType) -> Result<Self, Self::Error> {
                let index: GlobalZomeTypeId = index.into();
                Self::try_from(index)
            }
        }

        impl TryFrom<&LinkType> for #ident {
            type Error = WasmError;
            fn try_from(index: &LinkType) -> Result<Self, Self::Error> {
                Self::try_from(*index)
            }
        }

    };
    output.into()
}
