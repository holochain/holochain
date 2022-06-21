use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;

use crate::util::get_single_tuple_variant;
use crate::util::index_to_u8;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);

    let (ident, variants) = match &input {
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => (ident, variants),
        _ => abort!(input, "hdk_dependent_entry_types can only be used on Enums"),
    };

    let index_to_variant: proc_macro2::TokenStream = variants
        .iter()
        .enumerate()
        .map(|(i, v)|(index_to_u8(i), v))
        .map(|(i, syn::Variant { ident: v_ident, fields, .. })| {
            let ty = &get_single_tuple_variant(v_ident, fields).ty;
            quote::quote! {
                if ((<#ident as EnumVariantLen<#i>>::ENUM_VARIANT_START)..(<#ident as EnumVariantLen<#i>>::ENUM_VARIANT_LEN)).contains(&offset.0) {
                    return Ok(Some(Self::#v_ident(#ty::try_from((type_index, entry))?)));
                }
            }
        })
        .collect();

    let try_into_entry: proc_macro2::TokenStream = variants
        .into_iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                get_single_tuple_variant(v_ident, fields);
                quote::quote! {#ident::#v_ident (v) => Entry::try_from(v),}
            },
        )
        .collect();

    let into_entry_def_index: proc_macro2::TokenStream = variants
        .into_iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                get_single_tuple_variant(v_ident, fields);
                quote::quote! {#ident::#v_ident (v) => EntryDefIndex::from(v),}
            },
        )
        .collect();

    let into_visibility: proc_macro2::TokenStream = variants
        .into_iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                get_single_tuple_variant(v_ident, fields);
                quote::quote! {#ident::#v_ident (v) => EntryVisibility::from(v),}
            },
        )
        .collect();

    let output = quote::quote! {
        #[hdk_to_local_types(nested = true)]
        #[derive(Debug)]
        #input

        impl TryFrom<&#ident> for Entry {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                match value {
                    #try_into_entry
                }
            }
        }

        impl TryFrom<#ident> for Entry {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Entry::try_from(&value)
            }
        }

        impl From<&#ident> for EntryDefIndex {
            fn from(value: &#ident) -> Self {
                match value {
                    #into_entry_def_index
                }
            }
        }

        impl From<#ident> for EntryDefIndex {
            fn from(value: #ident) -> Self {
                Self::from(&value)
            }
        }

        impl From<&&#ident> for EntryDefIndex {
            fn from(value: &&#ident) -> Self {
                Self::from(*value)
            }
        }

        impl From<&#ident> for EntryVisibility {
            fn from(v: &#ident) -> Self {
                match v {
                    #into_visibility
                }
            }
        }

        impl From<&&#ident> for EntryVisibility {
            fn from(v: &&#ident) -> Self {
                Self::from(*v)
            }
        }

        impl TryFrom<&#ident> for ZomeId {
            type Error = WasmError;

            fn try_from(v: &#ident) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.entries.zome_id(LocalZomeTypeId::from(v)) {
                    Some(z) => Ok(z),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "ZomeId not found for {:?}",
                        v
                    )))),
                }
            }
        }

        impl TryFrom<#ident> for ZomeId {
            type Error = WasmError;

            fn try_from(v: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&v)
            }
        }

        impl EntryTypesHelper for #ident {
            fn deserialize_from_type<Z, I>(
                zome_id: Z,
                type_index: I,
                entry: &Entry,
            ) -> Result<Option<Self>, WasmError>
            where
                Z: Into<ZomeId>,
                I: Into<LocalZomeTypeId>
            {
                let zome_id = zome_id.into();
                let type_index = type_index.into();
                match zome_info()?.zome_types.entries.offset(zome_id, type_index) {
                    Some(offset) => {
                        #index_to_variant

                        Ok(None)
                    }
                    _ => Ok(None),
                }
            }
        }

    };
    output.into()
}
