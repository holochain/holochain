use holochain_integrity_types::EntryVisibility;
use holochain_integrity_types::RequiredValidations;
use proc_macro::TokenStream;

use darling::FromDeriveInput;
use darling::FromVariant;
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
    let opts = Opts::from_derive_input(&input).expect("Wrong options");
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
                    let visibility =
                        parse_visibility(visibility).expect("Failed to parse visibility");
                    let required_validations =
                        required_validations.unwrap_or_else(|| RequiredValidations::default().0);
                    quote::quote! {
                        AppEntryDef {
                            name: AppEntryDefName::from_str(#id),
                            visibility: #visibility,
                            required_validations: RequiredValidations(#required_validations),
                        },
                    }
                },
            )
            .collect(),
        _ => todo!("Make real error"),
    };

    let output = quote::quote! {
        impl EntryDefRegistration for #ident {
            const ENTRY_DEFS: &'static [AppEntryDef] = &[#inner];
        }
        impl EntryDefRegistration for &#ident {
            const ENTRY_DEFS: &'static [AppEntryDef] = &#ident::ENTRY_DEFS;
        }
    };
    // eprintln!("{}", output);
    output.into()
}

fn parse_visibility(variant: Option<String>) -> darling::Result<proc_macro2::TokenStream> {
    let variant = match variant {
        Some(v) => v,
        None => return Ok(default_visibility()),
    };
    match variant.as_str() {
        "public" => Ok(quote::quote! {EntryVisibility::Public}),
        "private" => Ok(quote::quote! {EntryVisibility::Private}),
        _ => Err(darling::Error::custom(
            "EntryVisibility can only be `public` or `private`",
        )),
    }
}

fn default_visibility() -> proc_macro2::TokenStream {
    match EntryVisibility::default() {
        EntryVisibility::Public => quote::quote! {EntryVisibility::Public},
        EntryVisibility::Private => quote::quote! {EntryVisibility::Private},
    }
}
