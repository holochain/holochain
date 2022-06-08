use proc_macro::TokenStream;
use proc_macro_error::abort;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;
use syn::ItemStruct;

/// Type to allow using this macro on entires or types.
enum Category {
    Entries,
    Links,
}

pub fn build_entry(_args: TokenStream, input: TokenStream) -> TokenStream {
    build(Category::Entries, input)
}

pub fn build_link(_args: TokenStream, input: TokenStream) -> TokenStream {
    build(Category::Links, input)
}

fn build(category: Category, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let ident = match &input {
        Item::Enum(ItemEnum { ident, .. }) | Item::Struct(ItemStruct { ident, .. }) => ident,
        r => {
            abort!(
                r,
                "The `to_global_types` macro can only be used on enums or structs."
            )
        }
    };

    // Create the only difference between `hdk_to_global_entry_types`
    // and `hdk_to_global_link_types`.
    let category = match category {
        Category::Entries => quote::quote! {.entries},
        Category::Links => quote::quote! {.links},
    };

    // Implement `TryFrom<Self> for GlobalZOmeTypeId`.
    let output = quote::quote! {
        #input

        impl TryFrom<&#ident> for GlobalZomeTypeId {
            type Error = WasmError;

            fn try_from(value: &#ident) -> Result<Self, Self::Error> {
                // Call zome info to get the types in scope for the calling zome.
                zome_info()?
                    .zome_types
                    // Add `.entries` or `.links`.
                    #category
                    // Convert to global scope or return an error.
                    .to_global_scope(value)
                    .ok_or_else(|| {
                        WasmError::Guest(format!(
                            "Value {:?} does not map to a global entry type for current scope.",
                            value
                        ))
                    })
            }
        }


        // Implement the same for an owned value.
        impl TryFrom<#ident> for GlobalZomeTypeId {
            type Error = WasmError;

            fn try_from(value: #ident) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }
    };
    output.into()
}
