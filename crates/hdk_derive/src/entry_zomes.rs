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
                if ((<#ident as EnumVariantLen<#i>>::ENUM_VARIANT_START)..(<#ident as EnumVariantLen<#i>>::ENUM_VARIANT_LEN)).contains(&value) {
                    return Ok(#ty::try_from_local_type::<LocalZomeTypeId>(LocalZomeTypeId(value), entry)?.map(Self::#v_ident));
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

    let try_into_app_bytes: proc_macro2::TokenStream = variants
        .into_iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                get_single_tuple_variant(v_ident, fields);
                quote::quote! {#ident::#v_ident (v) => AppEntryBytes::try_from(v),}
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
        #[hdk_to_global_entry_types]
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

        impl TryFrom<&#ident> for AppEntryBytes {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                match value {
                    #try_into_app_bytes
                }
            }
        }

        impl TryFrom<#ident> for AppEntryBytes {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl TryFrom<&#ident> for EntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Ok(Self(GlobalZomeTypeId::try_from(value)?.0))
            }
        }

        impl TryFrom<#ident> for EntryDefIndex {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl TryFrom<&&#ident> for EntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &&#ident) -> Result<Self, Self::Error> {
                Self::try_from(*value)
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

        impl TryFrom<&#ident> for AppEntry {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Ok(
                    AppEntry {
                        entry_def_index: (&value).try_into()?,
                        visibility: (&value).try_into()?,
                        entry: value.try_into()?,
                    }
                )
            }
        }

        impl TryFrom<#ident> for AppEntry {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl TryFrom<&#ident> for ElementBuilder {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Ok(ElementBuilder::App(value.try_into()?))
            }
        }

        impl TryFrom<#ident> for ElementBuilder {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }
        impl EntryTypesHelper for #ident {
            fn try_from_local_type<I>(type_index: I, entry: &Entry) -> Result<Option<Self>, WasmError>
            where
                LocalZomeTypeId: From<I>,
            {
                let value = LocalZomeTypeId::from(type_index).0;
                #index_to_variant

                Err(wasm_error!(WasmErrorInner::Guest(format!(
                    "local type index {} does not map to any the entry types for this zome",
                    value
                ))))
            }
            fn try_from_global_type<I>(type_index: I, entry: &Entry) -> Result<Option<Self>, WasmError>
            where
                GlobalZomeTypeId: From<I>,
            {
                let index: GlobalZomeTypeId = type_index.into();
                match zome_info()?.zome_types.entries.to_local_scope(index) {
                    Some(local_index) => Self::try_from_local_type(local_index, &entry),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "global index {} does not map to any local scope for this zome",
                        index.0
                    )))),
                }
            }
        }

    };
    output.into()
}
