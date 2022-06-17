use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::AttributeArgs;
use syn::Item;
use syn::ItemEnum;

use crate::util::get_unit_ident;

#[derive(Debug, FromMeta)]
pub struct MacroArgs {
    #[darling(default)]
    skip_hdk_extern: bool,
}

pub fn build(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let attr_args = parse_macro_input!(attrs as AttributeArgs);

    let (ident, variants, attrs) = match &input {
        Item::Enum(ItemEnum {
            ident,
            variants,
            attrs,
            ..
        }) => (ident, variants, attrs),
        _ => abort!(
            input,
            "hdk_entry_defs_name_registration can only be used on Enums"
        ),
    };

    let unit_ident = get_unit_ident(attrs);

    let units_to_full: proc_macro2::TokenStream = variants
        .iter()
        .map(|syn::Variant { ident: v_ident, .. }| {
            quote::quote! {
                #unit_ident::#v_ident => Ok(Some(Self::#v_ident(entry.try_into()?))),
            }
        })
        .collect();

    let skip_hdk_extern = match MacroArgs::from_list(&attr_args) {
        Ok(a) => a.skip_hdk_extern,
        Err(e) => abort!(ident, "{}", e),
    };

    let hdk_extern = if skip_hdk_extern {
        quote::quote! {}
    } else {
        quote::quote! {#[hdk_extern]}
    };

    let no_mangle = if skip_hdk_extern {
        quote::quote! {}
    } else {
        quote::quote! {#[no_mangle]}
    };

    let output = quote::quote! {
        #[derive(EntryDefRegistration, UnitEnum)]
        #[unit_attrs(forward(hdk_to_local_types, hdk_to_global_entry_types))]
        #input

        #hdk_extern
        pub fn entry_defs(_: ()) -> ExternResult<EntryDefsCallbackResult> {
            let defs: Vec<EntryDef> = #ident::ENTRY_DEFS
                    .iter()
                    .map(|a| EntryDef::from(a.clone()))
                    .collect();
            Ok(EntryDefsCallbackResult::from(defs))
        }

        #no_mangle
        pub fn __num_entry_types() -> u8 { #unit_ident::len() }

        impl TryFrom<&#ident> for GlobalZomeTypeId {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Self::try_from(value.to_unit())
            }
        }

        impl TryFrom<#ident> for GlobalZomeTypeId {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl TryFrom<&#ident> for EntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Ok(Self(GlobalZomeTypeId::try_from(value.to_unit())?.0))
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

        impl From<&#ident> for LocalZomeTypeId {
            fn from(t: &#ident) -> Self {
                Self::from(t.to_unit())
            }
        }

        impl From<#ident> for LocalZomeTypeId {
            fn from(t: #ident) -> Self {
                Self::from(&t)
            }
        }

        impl TryFrom<&#ident> for EntryType {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                value.to_unit().try_into()
            }
        }

        impl TryFrom<#ident> for EntryType {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl From<&#ident> for EntryVisibility {
            fn from(v: &#ident) -> Self {
                Self::from(v.to_unit())
            }
        }

        impl From<&&#ident> for EntryVisibility {
            fn from(v: &&#ident) -> Self {
                Self::from(v.to_unit())
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

        impl TryFrom<&#ident> for RecordBuilder {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Ok(RecordBuilder::App(value.try_into()?))
            }
        }

        impl TryFrom<#ident> for RecordBuilder {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl From<#unit_ident> for EntryVisibility {
            fn from(v: #unit_ident) -> Self {
                #ident::ENTRY_DEFS[LocalZomeTypeId::from(v).0 as usize].visibility
            }
        }

        impl TryFrom<#unit_ident> for EntryType {
            type Error = WasmError;

            fn try_from(value: #unit_ident) -> Result<Self, Self::Error> {
                Ok(EntryType::App(AppEntryType::try_from(value)?))
            }
        }

        impl TryFrom<#unit_ident> for AppEntryType {
            type Error = WasmError;

            fn try_from(value: #unit_ident) -> Result<Self, Self::Error> {
                let id = value.try_into()?;
                let def: EntryDef = value.into();
                Ok(Self {
                    id,
                    visibility: def.visibility,
                })
            }
        }

        impl From<#unit_ident> for EntryDef {
            fn from(v: #unit_ident) -> Self {
                let i: LocalZomeTypeId = v.into();
                #ident::ENTRY_DEFS[i.0 as usize].clone()
            }
        }

        impl TryFrom<LocalZomeTypeId> for #unit_ident {
            type Error = WasmError;

            fn try_from(value: LocalZomeTypeId) -> Result<Self, Self::Error> {
                Self::iter()
                    .find(|u| LocalZomeTypeId::from(*u) == value)
                    .ok_or_else(|| {
                        wasm_error!(WasmErrorInner::Guest(format!(
                            "local index {} does not match any variant of {}",
                            value.0, stringify!(#unit_ident)
                        )))
                    })
            }
        }

        impl TryFrom<&#unit_ident> for EntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &#unit_ident) -> Result<Self, Self::Error> {
                Ok(Self(GlobalZomeTypeId::try_from(value)?.0))
            }
        }

        impl TryFrom<#unit_ident> for EntryDefIndex {
            type Error = WasmError;

            fn try_from(value: #unit_ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl TryFrom<&EntryDefIndex> for #unit_ident {
            type Error = WasmError;

            fn try_from(index: &EntryDefIndex) -> Result<Self, Self::Error> {
                let index: GlobalZomeTypeId = (*index).into();
                Self::try_from(index)
            }
        }

        impl TryFrom<EntryDefIndex> for #unit_ident {
            type Error = WasmError;

            fn try_from(index: EntryDefIndex) -> Result<Self, Self::Error> {
                Self::try_from(&index)
            }
        }

        impl TryFrom<GlobalZomeTypeId> for #unit_ident {
            type Error = WasmError;

            fn try_from(index: GlobalZomeTypeId) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.entries.to_local_scope(index) {
                    Some(local_index) => Self::try_from(local_index),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "global index {:?} does not map to any local scope for this zome",
                        index
                    )))),
                }
            }
        }

        impl EntryTypesHelper for #ident {
            fn try_from_local_type<I>(type_index: I, entry: &Entry) -> Result<Option<Self>, WasmError>
            where
                LocalZomeTypeId: From<I>,
            {
                <#ident as UnitEnum>::Unit::try_from(LocalZomeTypeId::from(type_index))
                    .ok()
                    .map_or(Ok(None), |unit| match unit {
                        #units_to_full
                    })
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

        impl EnumLen for #ident {
            const ENUM_LEN: u8 = <#ident as UnitEnum>::Unit::ENUM_LEN;
        }
    };
    output.into()
}
