use holochain_integrity_types::EntryVisibility;
use holochain_integrity_types::RequiredValidations;
use proc_macro::TokenStream;

use darling::FromDeriveInput;
use darling::FromVariant;
use proc_macro_error::abort;
use syn::parse_macro_input;

#[derive(FromVariant)]
#[darling(attributes(entry_def, entry_name))]
struct VarOpts {
    ident: syn::Ident,
    #[darling(default)]
    name: Option<String>,
    #[darling(default)]
    visibility: Option<String>,
    #[darling(default)]
    required_validations: Option<u8>,
}

#[derive(FromDeriveInput)]
struct Opts {
    ident: syn::Ident,
    data: darling::ast::Data<VarOpts, darling::util::Ignored>,
}

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    let opts = match Opts::from_derive_input(&input) {
        Ok(o) => o,
        Err(e) => abort!(e.span(), e),
    };
    let Opts { ident, data } = opts;

    let inner: proc_macro2::TokenStream = match data {
        darling::ast::Data::Enum(variants) => variants
            .into_iter()
            .flat_map(
                |VarOpts {
                     ident: v_ident,
                     name,
                     visibility,
                     required_validations,
                     ..
                 }| {
                    let id = crate::util::to_snake_name(name, &v_ident);
                    let visibility = parse_visibility(&v_ident, visibility);
                    let required_validations =
                        required_validations.unwrap_or_else(|| RequiredValidations::default().0);
                    quote::quote! {
                        EntryDef {
                            id: EntryDefId::App(AppEntryDefName::from_str(#id)),
                            visibility: #visibility,
                            required_validations: RequiredValidations(#required_validations),
                        },
                    }
                },
            )
            .collect(),
        _ => abort!(ident, "EntryDefRegistration can only be derived on Enums"),
    };

    let output = quote::quote! {
        impl EntryDefRegistration for #ident {
            const ENTRY_DEFS: &'static [EntryDef] = &[#inner];
        }
        impl EntryDefRegistration for &#ident {
            const ENTRY_DEFS: &'static [EntryDef] = &#ident::ENTRY_DEFS;
        }
    };
    output.into()
}

fn parse_visibility(ident: &syn::Ident, variant: Option<String>) -> proc_macro2::TokenStream {
    let variant = match variant {
        Some(v) => v,
        None => return default_visibility(),
    };
    match variant.as_str() {
        "public" => quote::quote! {EntryVisibility::Public},
        "private" => quote::quote! {EntryVisibility::Private},
        _ => abort!(ident, "EntryVisibility can only be `public` or `private`"),
    }
}

fn default_visibility() -> proc_macro2::TokenStream {
    match EntryVisibility::default() {
        EntryVisibility::Public => quote::quote! {EntryVisibility::Public},
        EntryVisibility::Private => quote::quote! {EntryVisibility::Private},
    }
}
