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

    let index_to_variant: proc_macro2::TokenStream = variants
        .iter()
        .enumerate()
        .map(|(index, syn::Variant { ident: v_ident, .. })| {
            quote::quote! {#index => Ok(#ident::#v_ident(entry.try_into()?)),}
        })
        .collect();

    let output = quote::quote! {
        #[derive(EntryDefRegistration, UnitEnum)]
        #input



        #[hdk_extern]
        pub fn entry_defs(_: ()) -> ExternResult<EntryDefsCallbackResult> {
            let defs: Vec<EntryDef> = #ident::ENTRY_DEFS
                    .iter()
                    .map(|a| EntryDef::from(a.clone()))
                    .collect();
            Ok(EntryDefsCallbackResult::from(defs))
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
