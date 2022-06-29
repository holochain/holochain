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
        .map(|syn::Variant { ident, fields, .. }| {
            let ty = &get_single_tuple_variant(ident, fields).ty;
            quote::quote! {
                vec.extend(#ty::iter().map(Self::#ident));
            }
        })
        .collect();

    let ranges: proc_macro2::TokenStream = variants
        .iter()
        .map(|syn::Variant { ident, fields, .. }| {
            get_single_tuple_variant(ident, fields);
            quote::quote! {
                Self::find_variant(|t| matches!(t, Self::#ident(_)), &range, &zome_types)?,
            }
        })
        .collect();

    let output = quote::quote! {
        #[hdk_to_global_link_types]
        #[hdk_to_local_types(nested = true)]
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
        #input

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

        impl LinkTypesHelper<{ #ident::len() as usize }> for #ident {
            fn range(
                range: impl std::ops::RangeBounds<Self> + 'static + std::fmt::Debug,
            ) -> Box<dyn FnOnce() -> Result<LinkTypeRanges, WasmError>> {
                let zome_types = zome_info().map(|t| t.zome_types);
                let f = move || {
                    let zome_types = zome_types?;

                    let vec = vec![
                        #ranges
                    ];
                    if vec.iter().all(|t| matches!(t, LinkTypeRange::Empty)) {
                        Ok(LinkTypeRanges(vec![LinkTypeRange::Empty]))
                    } else if vec.iter().all(|t| matches!(t, LinkTypeRange::Full)) {
                        Ok(LinkTypeRanges(vec![LinkTypeRange::Full]))
                    } else {
                        Ok(LinkTypeRanges(vec))
                    }
                };
                Box::new(f)
            }

            fn iter() -> core::array::IntoIter<Self, { #ident::len() as usize }> {
                use #ident::*;
                let mut vec = Vec::with_capacity(#ident::len() as usize);

                #iter

                let arr: [_; LinkZomes::len() as usize] = vec
                    .try_into()
                    .expect("This can't fail unless the const generics are wrong");
                arr.into_iter()
            }
        }

        impl TryFrom<LocalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(value: LocalZomeTypeId) -> Result<Self, Self::Error> {
                Self::iter()
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

        impl TryFrom<GlobalZomeTypeId> for #ident {
            type Error = WasmError;

            fn try_from(index: GlobalZomeTypeId) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.links.to_local_scope(index) {
                    Some(local_index) => Self::try_from(local_index),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "global index {:?} does not map to any local scope for this zome",
                        index
                    )))),
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
