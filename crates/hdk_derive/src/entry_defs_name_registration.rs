use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;

pub fn build(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let ident = match &input {
        Item::Enum(ItemEnum { ident, .. }) => ident,
        _ => todo!(),
    };

    let unit = match &input {
        Item::Enum(ItemEnum { variants, .. }) => {
            let unit_ident = quote::format_ident!("{}Unit", ident);
            let units: proc_macro2::TokenStream = variants
                .iter()
                .map(|syn::Variant { ident, .. }| quote::quote! {#ident,})
                .collect();
            let units_match: proc_macro2::TokenStream = variants
                .iter()
                .map(
                    |syn::Variant {
                         ident: v_ident,
                         fields,
                         ..
                     }| {
                        let enum_style = match fields {
                            syn::Fields::Named(_) => quote::quote! {{..}},
                            syn::Fields::Unit => quote::quote! {},
                            syn::Fields::Unnamed(_) => quote::quote! {(_)},
                        };
                        quote::quote! {#ident::#v_ident #enum_style => #unit_ident::#v_ident,}
                    },
                )
                .collect();
            quote::quote! {
                #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
                pub enum #unit_ident {
                    #units
                }
                impl #ident {
                    pub fn unit(&self) -> #unit_ident {
                        match self {
                            #units_match
                        }
                    }
                    pub fn index(&self) -> usize {
                        self.unit() as usize
                    }
                }
            }
        }
        _ => todo! {},
    };

    let output = quote::quote! {
        #[derive(EntryDefRegistration)]
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

        #unit
    };
    output.into()
}
