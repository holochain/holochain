use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;

use crate::util::get_single_tuple_variant;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    // Parse the input.
    let input = parse_macro_input!(input as Item);
    let (ident, variants) = match &input {
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => (ident, variants),
        _ => abort!(input, "hdk_link_types can only be used on Enums"),
    };
    let iter: proc_macro2::TokenStream = variants
        .iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                let ty = &get_single_tuple_variant(v_ident, fields).ty;
                quote::quote! {
                    vec.extend(#ty::iter().map(#ident::#v_ident));
                }
            },
        )
        .collect();

    let into_link_type: proc_macro2::TokenStream = variants
        .into_iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                get_single_tuple_variant(v_ident, fields);
                quote::quote! {#ident::#v_ident (v) => LinkType::from(v),}
            },
        )
        .collect();

    let output = quote::quote! {
        #[hdk_to_local_types(nested = true)]
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
        #input

        impl From<&#ident> for LinkType {
            fn from(value: &#ident) -> Self {
                match value {
                    #into_link_type
                }
            }
        }

        impl From<#ident> for LinkType {

            fn from(value: #ident) -> Self {
                Self::from(&value)
            }
        }

        impl TryFrom<#ident> for LinkTypeFilter {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                let z: ZomeId = value.try_into()?;
                let lt: LinkType = value.into();
                Ok(LinkTypeFilter::single_type(z, lt))
            }
        }

        impl TryFrom<&#ident> for LinkTypeFilter {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                let z: ZomeId = value.try_into()?;
                let lt: LinkType = value.into();
                Ok(LinkTypeFilter::single_type(z, lt))
            }
        }

        impl LinkTypeFilterExt for #ident {
            fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
                self.try_into()
            }
        }

        fn iter() -> core::array::IntoIter<#ident, { #ident::len() as usize }> {
            use #ident::*;
            let mut vec = Vec::with_capacity(#ident::len() as usize);

            #iter

            let arr: [_; LinkZomes::len() as usize] = vec
                .try_into()
                .expect("This can't fail unless the const generics are wrong");
            arr.into_iter()
        }

        impl TryFrom<LocalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(value: LocalZomeTypeId) -> Result<Self, Self::Error> {
                iter()
                    .find(|u| LocalZomeTypeId::from(*u) == value)
                    .ok_or_else(|| {
                        wasm_error!(WasmErrorInner::Guest(format!(
                            "local index {:?} does not match any variant of {}",
                            value, stringify!(#ident)
                        )))
                    })
            }
        }

        impl TryFrom<&LocalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(value: &LocalZomeTypeId) -> Result<Self, Self::Error> {
                Self::try_from(*value)
            }
        }

        impl TryFrom<(ZomeId, LinkType)> for #ident {
            type Error = WasmError;

            fn try_from((zome_id, index): (ZomeId, LinkType)) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.links.offset(zome_id, index) {
                    Some(t) => Self::try_from(t),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "LinkType {:?} {:?} does not map to any local scope for this zome",
                        zome_id,
                        index
                    )))),
                }
            }
        }

        impl TryFrom<&#ident> for ZomeId {
            type Error = WasmError;

            fn try_from(index: &#ident) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.links.zome_id(LocalZomeTypeId::from(index)) {
                    Some(z) => Ok(z),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "ZomeId not found for {:?}",
                        index
                    )))),
                }
            }
        }

        impl TryFrom<#ident> for ZomeId {
            type Error = WasmError;

            fn try_from(index: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&index)
            }
        }

    };
    output.into()
}
