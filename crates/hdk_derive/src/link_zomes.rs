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

    let into_link_type: proc_macro2::TokenStream = variants
        .into_iter()
        .map(
            |syn::Variant {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                match fields {
                    syn::Fields::Named(_) => todo!(),
                    syn::Fields::Unit => todo!(),
                    syn::Fields::Unnamed(_) => {
                        // TODO: Error if fields is longer then one.
                        quote::quote! {#ident::#v_ident (v) => v.into(),}
                    }
                }
            },
        )
        .collect();

    let output = quote::quote! {
        #[derive(ToZomeName, ToLinkTypeQuery, Clone, Copy)]
        #input

        impl From<#ident> for LinkType {
            fn from(lt: #ident) -> Self {
                match lt {
                    #into_link_type
                }
            }
        }

        impl From<#ident> for Option<Box<dyn ToLinkTypeQuery>> {
            fn from(l: #ident) -> Self {
                Some(Box::new(l))
            }
        }

        impl From<&#ident> for Option<Box<dyn ToLinkTypeQuery>> {
            fn from(l: &#ident) -> Self {
                l.clone().into()
            }
        }

        impl From<#ident> for Option<LinkTypeQuery> {
            fn from(l: #ident) -> Self {
                Some(l.into())
            }
        }

        impl From<&#ident> for Option<LinkTypeQuery> {
            fn from(l: &#ident) -> Self {
                Some(l.into())
            }
        }
    };
    // let output = expander::Expander::new("link_zomes")
    //     .fmt(expander::Edition::_2021)
    //     .verbose(true)
    //     // common way of gating this, by making it part of the default feature set
    //     .dry(false)
    //     .write_to_out_dir(output.clone())
    //     .unwrap_or_else(|e| {
    //         eprintln!("Failed to write to file: {:?}", e);
    //         output
    //     });
    output.into()
}
