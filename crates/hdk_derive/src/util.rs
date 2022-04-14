use heck::ToSnakeCase;

pub fn to_snake_name(name: Option<String>, v_ident: &syn::Ident) -> String {
    match name {
        Some(s) => s,
        None => v_ident.to_string().to_snake_case(),
    }
}
