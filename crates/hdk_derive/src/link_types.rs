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
        #[hdk_to_coordinates(entry = false)]
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
        #input

        // Add the extern function that says how many links this zome has.
        #no_mangle
        pub fn __num_link_types() -> u8 { #ident::len() }

        impl TryFrom<&#ident> for ScopedLinkType {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.links.get(value) {
                    Some(t) => Ok(t),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "{:?} does not map to any ZomeId and LinkType that is in scope for this zome.",
                        value
                    )))),
                }
            }
        }

        impl TryFrom<ScopedLinkType> for #ident {
            type Error = WasmError;

            fn try_from(value: ScopedLinkType) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.links.find(Self::iter(), value) {
                    Some(t) => Ok(t),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "{:?} does not map to any link defined by this type.",
                        value
                    )))),
                }
            }
        }

        impl TryFrom<#ident> for ScopedLinkType {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl TryFrom<#ident> for LinkTypeFilter {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl TryFrom<&#ident> for LinkTypeFilter {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                let ScopedLinkType {
                    zome_id,
                    zome_type,
                } = value.try_into()?;
                Ok(LinkTypeFilter::single_type(zome_id, zome_type))
            }
        }

        impl LinkTypeFilterExt for #ident {
            fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
                self.try_into()
            }
        }

        impl #ident {
            pub fn iter() -> impl Iterator<Item = Self> {
                use #ident::*;
                [#units].into_iter()
            }
        }

        impl LinkTypesHelper for #ident {
            type Error = WasmError;

            fn from_type<Z, I>(zome_id: Z, link_type: I) -> Result<Option<Self>, Self::Error>
            where
                Z: Into<ZomeId>,
                I: Into<LinkType>
            {
                let link_type = ScopedLinkType {
                    zome_id: zome_id.into(),
                    zome_type: link_type.into(),
                };
                let links = zome_info()?.zome_types.links;
                match links.find(#ident::iter(), link_type) {
                    Some(l) => Ok(Some(l)),
                    None => if links.dependencies().any(|z| z == link_type.zome_id) {
                        Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "Link type: {:?} is out of range for this zome.",
                            link_type
                        ))))
                    } else {
                        Ok(None)
                    }
                }
            }
        }
    };
    output.into()
}
