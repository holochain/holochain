use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);

    let (ident, variants) = match &input {
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => (ident, variants),
        _ => todo!(),
    };

    let enum_len = variants.len();

    let entry_def_map: proc_macro2::TokenStream = variants
        .iter()
        .map(|syn::Variant { ident: v_ident, .. }| {
            quote::quote! {(#ident::#v_ident as usize, <#ident as UnitEnum>::Unit::#v_ident as usize),}
        })
        .collect();

    let output = quote::quote! {
        #[derive(EntryDefRegistration, UnitEnum)]
        #input

        impl ToAppEntryDefName for &#ident {
            fn entry_def_name(&self) -> AppEntryDefName {
                #ident::ENTRY_DEFS[self.index()].name.clone()
            }
        }

        impl ToAppEntryDefName for #ident {
            fn entry_def_name(&self) -> AppEntryDefName {
                Self::ENTRY_DEFS[self.index()].name.clone()
            }
        }


        impl #ident {
            pub fn variant_to_index<T>(f: fn(T) -> Self) -> usize {
                thread_local!(static ENTRY_DEF_MAP: [(usize, usize); #enum_len] = {
                    [
                        #entry_def_map
                    ]
                });
                let i = ENTRY_DEF_MAP.with(|m| m.iter().find(|i| i.0 == f as usize).map(|(_, i)| *i));
                match i {
                    Some(i) => i,
                    None => todo!(),
                }
            }
            pub fn variant_to_app_entry_def<T>(f: fn(T) -> Self) -> &'static AppEntryDef {
                &Self::ENTRY_DEFS[Self::variant_to_index(f)]
            }
            pub fn variant_to_app_entry_def_name<T>(f: fn(T) -> Self) -> &'static AppEntryDefName {
                &Self::variant_to_app_entry_def(f).name
            }
            pub fn variant_to_entry_visibility<T>(f: fn(T) -> Self) -> EntryVisibility {
                Self::variant_to_app_entry_def(f).visibility
            }
            pub fn variant_to_required_validations<T>(f: fn(T) -> Self) -> RequiredValidations {
                Self::variant_to_app_entry_def(f).required_validations
            }
            pub fn variant_to_entry_def<T>(f: fn(T) -> Self) -> EntryDef {
                Self::variant_to_app_entry_def(f).clone().into()
            }
            pub fn variant_to_entry_def_id<T>(f: fn(T) -> Self) -> EntryDefId {
                Self::variant_to_app_entry_def(f).name.clone().into()
            }
            pub fn variant_to_entry_def_index<T>(f: fn(T) -> Self) -> EntryDefIndex {
                EntryDefIndex(Self::variant_to_index(f) as u8)
            }
            pub fn all_entry_defs() -> Vec<EntryDef> {
                Self::ENTRY_DEFS
                    .iter()
                    .map(|a| EntryDef::from(a.clone()))
                    .collect()
            }

            pub fn entry_visibility(&self) -> EntryVisibility {
                Self::ENTRY_DEFS[self.index()].visibility
            }

            pub fn entry_def_index(&self) -> EntryDefIndex {
                EntryDefIndex(self.index() as u8)
            }
        }

        #[hdk_extern]
        pub fn entry_defs(_: ()) -> ExternResult<EntryDefsCallbackResult> {
            Ok(EntryDefsCallbackResult::from(#ident::all_entry_defs()))
        }
    };
    // let output = expander::Expander::new("entry_defs_name_registration")
    //     .fmt(expander::Edition::_2021)
    //     .verbose(true)
    //     // common way of gating this, by making it part of the default feature set
    //     .dry(false)
    //     .write_to_out_dir(output.clone()).unwrap_or_else(|e| {
    //         eprintln!("Failed to write to file: {:?}", e);
    //         output
    //     });
    output.into()
}
