use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;
use syn::ItemStruct;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);

    let ident = match &input {
        Item::Enum(ItemEnum { ident, .. }) | Item::Struct(ItemStruct { ident, .. }) => ident,
        _ => todo!(),
    };

    let output = quote::quote! {
        #[derive(Serialize, Deserialize, SerializedBytes, Debug)]
        #input

        holochain_deterministic_integrity::app_entry!(#ident);
    };
    output.into()
}
