use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemStruct;
use std::any::type_name;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let (ident) = match &input {
        Item::Struct(ItemStruct { ident, .. }) => (ident),
        _ => abort!(
            input,
            "hdk_entry_def_conversions can only be used on Structs"
        ),
    };

    let output = quote::quote! {
        #input

        impl TryFromDnaProperties<#ident> for #ident {
          pub fn try_from_dna_properties<T>() -> ExternResult<T>
          where
            T: Sized + TryFrom<SerializedBytes>
          {
              T::try_from(dna_info()?.modifiers.properties)
                  .map_err(|_| wasm_error!(WasmErrorInner::Guest(format!("Failed to deserialize dna properties into {:}", type_name::<T>()))))
          }
        }
    };
    output.into()
}
