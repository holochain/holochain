use proc_macro::TokenStream;

use darling::FromDeriveInput;
use darling::FromVariant;
use syn::parse_macro_input;

#[derive(FromVariant)]
#[darling(attributes(entry_def_name))]
struct VarOpts {
    ident: syn::Ident,
    fields: darling::ast::Fields<darling::util::Ignored>,
    #[darling(default)]
    name: Option<String>,
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
            let inner: proc_macro2::TokenStream = variants
                .into_iter()
                .flat_map(|VarOpts { ident: v_ident, fields, name, .. }| {
                    let enum_style = match fields.style {
                        darling::ast::Style::Struct => quote::quote! {{..}},
                        darling::ast::Style::Unit => quote::quote! {},
                        darling::ast::Style::Tuple => quote::quote! {(_)},
                    };
                    let entry_def_name = crate::util::to_snake_name(name, &v_ident);
                    quote::quote! { #ident::#v_ident #enum_style => AppEntryDefName(#entry_def_name.into()), }
                })
                .collect();
            quote::quote! {
                match self {
                    #inner
                }
            }
        }
        _ => todo!(),
    };

    let output = quote::quote! {
        impl ToAppEntryDefName for #ident {
            fn entry_def_name(&self) -> AppEntryDefName {
                #inner
            }
        }
    };
    // eprintln!("{}", output);
    output.into()
}
