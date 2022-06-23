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
                #unit_ident::#v_ident => Ok(Self::#v_ident(entry.try_into()?)),
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
        #[unit_attrs(forward(hdk_to_coordinates(entry = true)))]
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

        impl TryFrom<&#unit_ident> for ScopedEntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &#unit_ident) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.entries.get(value) {
                    Some(t) => Ok(t),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "{:?} does not map to any ZomeId and EntryDefIndex that is in scope for this zome.",
                        value
                    )))),
                }
            }
        }

        impl TryFrom<&#ident> for ScopedEntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                Self::try_from(value.to_unit())
            }
        }

        impl TryFrom<&&#ident> for ScopedEntryDefIndex {
            type Error = WasmError;

            fn try_from(value: &&#ident) -> Result<Self, Self::Error> {
                Self::try_from(value.to_unit())
            }
        }

        impl TryFrom<ScopedEntryDefIndex> for #unit_ident {
            type Error = WasmError;

            fn try_from(value: ScopedEntryDefIndex) -> Result<Self, Self::Error> {
                match zome_info()?.zome_types.entries.find(Self::iter(), value) {
                    Some(t) => Ok(t),
                    _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "{:?} does not map to any link defined by this type.",
                        value
                    )))),
                }
            }
        }

        impl TryFrom<#unit_ident> for ScopedEntryDefIndex {
            type Error = WasmError;

            fn try_from(value: #unit_ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl TryFrom<#ident> for ScopedEntryDefIndex {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl From<&#ident> for ZomeEntryTypesKey {
            fn from(v: &#ident) -> Self {
                v.to_unit().into()
            }
        }

        impl From<#ident> for ZomeEntryTypesKey {
            fn from(v: #ident) -> Self {
                v.to_unit().into()
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

        impl From<#unit_ident> for EntryVisibility {
            fn from(v: #unit_ident) -> Self {
                #ident::ENTRY_DEFS[ZomeEntryTypesKey::from(v).type_index.index()].visibility
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
                let ScopedEntryDefIndex {
                    zome_id,
                    zome_type: id,
                } = value.try_into()?;
                let def: EntryDef = value.into();
                Ok(Self {
                    id,
                    zome_id,
                    visibility: def.visibility,
                })
            }
        }

        impl From<#unit_ident> for EntryDef {
            fn from(v: #unit_ident) -> Self {
                let i = ZomeEntryTypesKey::from(v).type_index;
                #ident::ENTRY_DEFS[i.index()].clone()
            }
        }

        impl TryFrom<(#unit_ident, &Entry)> for #ident {
            type Error = WasmError;

            fn try_from((unit, entry): (#unit_ident, &Entry)) -> Result<Self, Self::Error> {
                match unit {
                    #units_to_full
                }
            }
        }
        impl EntryTypesHelper for #ident {
            fn deserialize_from_type<Z, I>(
                zome_id: Z,
                entry_def_index: I,
                entry: &Entry,
            ) -> Result<Option<Self>, WasmError>
            where
                Z: Into<ZomeId>,
                I: Into<EntryDefIndex>
            {
                let s = ScopedEntryDefIndex{ zome_id: zome_id.into(), zome_type: entry_def_index.into() };
                match zome_info()?.zome_types.entries.find(#unit_ident::iter(), s) {
                    Some(unit) => {
                        Ok(Some((unit, entry).try_into()?))
                    }
                    _ => Ok(None),
                }
            }
        }

        impl EnumLen for #ident {
            const ENUM_LEN: u8 = <#ident as UnitEnum>::Unit::ENUM_LEN;
        }
    };
    output.into()
}
