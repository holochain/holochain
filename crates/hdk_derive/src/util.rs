use heck::ToSnakeCase;
use proc_macro_error::abort;
use proc_macro_error::abort_call_site;
use syn::Fields;

pub fn to_snake_case(name: Option<String>, v_ident: &syn::Ident) -> String {
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

pub fn get_unit_ident(attrs: &[syn::Attribute]) -> syn::Ident {
    attrs
        .iter()
        .find(|a| {
            a.path
                .segments
                .last()
                .map_or(false, |s| s.ident == "unit_enum")
        })
        .and_then(|a| darling::util::parse_attribute_to_meta_list(a).ok())
        .and_then(|syn::MetaList { path, nested, .. }| {
            nested
                .first()
                .filter(|_| path.is_ident("unit_enum"))
                .and_then(|f| match f {
                    syn::NestedMeta::Meta(syn::Meta::Path(path)) => path.get_ident().cloned(),
                    _ => None,
                })
        })
        .unwrap_or_else(|| {
            abort_call_site!("macro requires attribute `unit_enum`."; 
                help = "Add attribute like `unit_enum(UnitEnumName)`")
        })
}

pub fn index_to_u8(index: usize) -> u8 {
    match u8::try_from(index) {
        Ok(i) => i,
        Err(_) => abort_call_site!("Can only have a maximum of 255 enum variants"),
    }
}

pub fn get_single_tuple_variant<'a>(ident: &syn::Ident, fields: &'a syn::Fields) -> &'a syn::Field {
    match fields {
        syn::Fields::Named(_) | syn::Fields::Unit => abort!(
            ident,
            "hdk_entry_types_conversions only works for tuple enums"
        ),
        syn::Fields::Unnamed(syn::FieldsUnnamed { unnamed, .. }) => unnamed
            .first()
            .filter(|_| unnamed.len() == 1)
            .unwrap_or_else(|| {
                abort!(
                    unnamed,
                    "hdk_entry_types_conversions must only have a single enum tuple"
                );
            }),
    }
}

pub fn get_return_type_ident(ty: &syn::Type) -> Option<syn::Ident> {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return Some(segment.ident.clone());
        }
    }
    None
}

pub fn is_callback_result(ty: &syn::Type, callback_result: &str) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "ExternResult" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    // check if T in `_Result<T>` is a unit type
                    if let Some(syn::GenericArgument::Type(syn::Type::Tuple(t))) = args.args.first()
                    {
                        return t.elems.is_empty() && callback_result == "()";
                    }
                    // check if T in `_Result<T>` ident matches callback_result
                    if let Some(syn::GenericArgument::Type(syn::Type::Path(inner_path))) =
                        args.args.first()
                    {
                        if let Some(inner_segment) = inner_path.path.segments.last() {
                            return inner_segment.ident == callback_result;
                        }
                    }
                }
            }
        }
    }
    false
}
