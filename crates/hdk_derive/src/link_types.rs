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

    let output = quote::quote! {
        #[derive(Clone, Copy)]
        #[repr(u8)]
        #input

        impl From<&#ident> for LinkType {
            fn from(lt: &#ident) -> Self {
                LinkType(*lt as u8)
            }
        }

        impl From<#ident> for LinkType {
            fn from(lt: #ident) -> Self {
                LinkType(lt as u8)
            }
        }
    };
    output.into()
}
