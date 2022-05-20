use heck::ToSnakeCase;
use syn::Fields;
use syn::Token;

pub fn to_snake_name(name: Option<String>, v_ident: &syn::Ident) -> String {
    match name {
        Some(s) => s,
        None => v_ident.to_string().to_snake_case(),
    }
}

pub fn ignore_enum_data(fields: &Fields) -> proc_macro2::TokenStream {
    match fields {
        syn::Fields::Named(_) => quote::quote! {{..}},
        syn::Fields::Unit => quote::quote! {},
        syn::Fields::Unnamed(_) => quote::quote! {(_)},
    }
}
