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

    let output = quote::quote! {
        #[hdk_to_coordinates(nested = true)]
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
        #input

        impl TryFrom<&#ident> for ScopedLinkType {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.links.get(value) {
                    Some(t) => Ok(t),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "{:?} does not map to any ZomeIndex and LinkType that is in scope for this zome.",
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

        fn iter() -> core::array::IntoIter<#ident, { #ident::len() as usize }> {
            use #ident::*;
            let mut vec = Vec::with_capacity(#ident::len() as usize);

            #iter

            let arr: [_; LinkZomes::len() as usize] = vec
                .try_into()
                .expect("This can't fail unless the const generics are wrong");
            arr.into_iter()
        }

        impl TryFrom<ScopedLinkType> for #ident {
            type Error = WasmError;

            fn try_from(value: ScopedLinkType) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.links.find(iter(), value) {
                    Some(t) => Ok(t),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "{:?} does not map to any link defined by this type.",
                        value
                    )))),
                }
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
                    zome_index,
                    zome_type,
                } = value.try_into()?;
                Ok(LinkTypeFilter::single_type(zome_index, zome_type))
            }
        }

        impl LinkTypeFilterExt for #ident {
            fn try_into_filter(self) -> Result<LinkTypeFilter, WasmError> {
                self.try_into()
            }
        }

        impl LinkTypesHelper for #ident {
            type Error = WasmError;

            fn from_type<Z, I>(zome_index: Z, link_type: I) -> Result<Option<Self>, Self::Error>
            where
                Z: Into<ZomeIndex>,
                I: Into<LinkType>
            {
                let link_type = ScopedLinkType {
                    zome_index: zome_index.into(),
                    zome_type: link_type.into(),
                };
                let links = zome_info()?.zome_types.links;
                match links.find(iter(), link_type) {
                    Some(l) => Ok(Some(l)),
                    None => if links.dependencies().any(|z| z == link_type.zome_index) {
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
