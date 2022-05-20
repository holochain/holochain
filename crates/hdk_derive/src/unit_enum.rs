use darling::FromMeta;
use proc_macro::TokenStream;

use darling::FromDeriveInput;
use darling::FromVariant;
use syn::parse_macro_input;

#[derive(FromVariant)]
struct VarOpts {
    ident: syn::Ident,
    fields: darling::ast::Fields<darling::util::Ignored>,
}

#[derive(FromMeta)]
struct EnumName(syn::Ident);

#[derive(FromDeriveInput)]
#[darling(forward_attrs(unit_enum))]
struct Opts {
    ident: syn::Ident,
    attrs: Vec<syn::Attribute>,
    data: darling::ast::Data<VarOpts, darling::util::Ignored>,
}

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    let opts = Opts::from_derive_input(&input).expect("Wrong options");
    let Opts { ident, attrs, data } = opts;

    let unit_ident = match darling::util::parse_attribute_to_meta_list(
        attrs.first().expect("Must have 'unit_enum' attribute"),
    ) {
        Ok(syn::MetaList { path, nested, .. }) if path.is_ident("unit_enum") => {
            match nested.first() {
                Some(syn::NestedMeta::Meta(syn::Meta::Path(path))) => path
                    .get_ident()
                    .expect("Failed to parse meta to ident")
                    .clone(),
                _ => todo!(),
            }
        }
        _ => todo!(),
    };

    let variants = match data {
        darling::ast::Data::Enum(variants) => variants,
        _ => todo!(),
    };

    // let unit_ident = quote::format_ident!("Unit{}", ident);
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
