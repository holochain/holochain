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

    let variants = match data {
        darling::ast::Data::Enum(variants) => variants,
        _ => todo!(),
    };

    let unit_ident = quote::format_ident!("Unit{}", ident);
    let units: proc_macro2::TokenStream = variants
        .iter()
        .map(|VarOpts { ident, .. }| quote::quote! {#ident,})
        .collect();

    let units_match: proc_macro2::TokenStream = variants
        .iter()
        .map(
            |VarOpts {
                 ident: v_ident,
                 fields,
                 ..
             }| {
                let enum_style = match fields.style {
                    darling::ast::Style::Struct => quote::quote! {{..}},
                    darling::ast::Style::Unit => quote::quote! {},
                    darling::ast::Style::Tuple => quote::quote! {(_)},
                };
                quote::quote! {#ident::#v_ident #enum_style => #unit_ident::#v_ident,}
            },
        )
        .collect();

    let output = quote::quote! {
        impl UnitEnum for #ident {
            type Unit = #unit_ident;

            fn to_unit(&self) -> Self::Unit {
                match self {
                    #units_match
                }
            }
            fn index(&self) -> usize {
                self.to_unit() as usize
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum #unit_ident {
            #units
        }
    };
    output.into()
}
