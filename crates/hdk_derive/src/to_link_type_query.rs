use proc_macro::TokenStream;

use darling::FromDeriveInput;
use darling::FromVariant;
use syn::parse_macro_input;

#[derive(FromVariant)]
struct VarOpts {
    ident: syn::Ident,
    fields: darling::ast::Fields<darling::util::Ignored>,
}

#[derive(FromDeriveInput)]
struct Opts {
    ident: syn::Ident,
    data: darling::ast::Data<VarOpts, darling::util::Ignored>,
}

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    let opts = Opts::from_derive_input(&input).expect("Wrong options");
    let Opts { ident, data } = opts;

    let inner: proc_macro2::TokenStream = match data {
        darling::ast::Data::Enum(variants) => {
            variants
                .into_iter()
                .map(
                    |VarOpts {
                         ident: v_ident,
                         fields,
                         ..
                     }| {
                        assert!(matches!(fields.style, darling::ast::Style::Tuple));
                        quote::quote! {#ident::#v_ident (v) => LinkTypeQuery::SingleType(zome_name, v.into()), }
                    },
                )
                .collect()
        }
        _ => todo!(),
    };

    let output = quote::quote! {
        impl ToLinkTypeQuery for #ident {
            fn link_type(&self) -> LinkTypeQuery {
                let zome_name = self.zome_name();
                match self {
                    #inner
                }
            }
        }
        impl From<#ident> for LinkTypeQuery {
            fn from(l: #ident) -> Self {
                l.link_type()
            }
        }
        impl From<&#ident> for LinkTypeQuery {
            fn from(l: &#ident) -> Self {
                l.link_type()
            }
        }
    };
    // let output = Expander::new("baz")
    //     .add_comment("This is generated code!".to_owned())
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
