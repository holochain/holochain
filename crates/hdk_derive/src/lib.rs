#![crate_type = "proc-macro"]
#![allow(clippy::manual_unwrap_or_default)] // Fixing requires a `darling` upgrade

use proc_macro::TokenStream;
use proc_macro_error::abort;
use proc_macro_error::abort_call_site;
use proc_macro_error::proc_macro_error;
use quote::TokenStreamExt;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse::Result;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use util::get_return_type_ident;
use util::is_callback_result;

mod dna_properties;
mod entry_helper;
mod entry_type_registration;
mod entry_types;
mod entry_types_conversions;
mod entry_types_name_registration;
mod entry_zomes;
mod link_types;
mod link_zomes;
mod to_coordinates;
mod unit_enum;
mod util;

struct EntryDef(holochain_integrity_types::entry_def::EntryDef);
struct EntryDefId(holochain_integrity_types::entry_def::EntryDefId);
struct EntryVisibility(holochain_integrity_types::entry_def::EntryVisibility);
struct RequiredValidations(holochain_integrity_types::entry_def::RequiredValidations);

impl Parse for EntryDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut id =
            holochain_integrity_types::entry_def::EntryDefId::App(String::default().into());
        let mut required_validations =
            holochain_integrity_types::entry_def::RequiredValidations::default();
        let mut visibility = holochain_integrity_types::entry_def::EntryVisibility::default();

        let vars = Punctuated::<syn::MetaNameValue, syn::Token![,]>::parse_terminated(input)?;
        for var in vars {
            if let Some(segment) = var.path.segments.first() {
                match segment.ident.to_string().as_str() {
                    "id" => match var.lit {
                        syn::Lit::Str(s) => {
                            id = holochain_integrity_types::entry_def::EntryDefId::App(
                                s.value().to_string().into(),
                            )
                        }
                        _ => unreachable!(),
                    },
                    "required_validations" => match var.lit {
                        syn::Lit::Int(i) => {
                            required_validations =
                                holochain_integrity_types::entry_def::RequiredValidations::from(
                                    i.base10_parse::<u8>()?,
                                )
                        }
                        _ => unreachable!(),
                    },
                    "visibility" => {
                        match var.lit {
                            syn::Lit::Str(s) => visibility = match s.value().as_str() {
                                "public" => {
                                    holochain_integrity_types::entry_def::EntryVisibility::Public
                                }
                                "private" => {
                                    holochain_integrity_types::entry_def::EntryVisibility::Private
                                }
                                _ => unreachable!(),
                            },
                            _ => unreachable!(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Ok(EntryDef(holochain_integrity_types::entry_def::EntryDef {
            id,
            visibility,
            required_validations,
            cache_at_agent_activity: false,
        }))
    }
}

impl quote::ToTokens for EntryDefId {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match &self.0 {
            holochain_integrity_types::entry_def::EntryDefId::App(s) => {
                let string: String = s.0.to_string();
                tokens.append_all(quote::quote! {
                    hdi::prelude::EntryDefId::App(#string.into())
                });
            }
            _ => unreachable!(),
        }
    }
}

impl quote::ToTokens for RequiredValidations {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let u = <u8>::from(self.0);
        tokens.append_all(quote::quote! {
            hdi::prelude::RequiredValidations::from(#u)
        });
    }
}

impl quote::ToTokens for EntryVisibility {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let variant = syn::Ident::new(
            match self.0 {
                holochain_integrity_types::entry_def::EntryVisibility::Public => "Public",
                holochain_integrity_types::entry_def::EntryVisibility::Private => "Private",
            },
            proc_macro2::Span::call_site(),
        );
        tokens.append_all(quote::quote! {
            hdi::prelude::EntryVisibility::#variant
        });
    }
}

impl quote::ToTokens for EntryDef {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let id = EntryDefId(self.0.id.clone());
        let visibility = EntryVisibility(self.0.visibility);
        let required_validations = RequiredValidations(self.0.required_validations);

        tokens.append_all(quote::quote! {
            hdi::prelude::EntryDef {
                id: #id,
                visibility: #visibility,
                required_validations: #required_validations,
            }
        });
    }
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn hdk_extern(attrs: TokenStream, item: TokenStream) -> TokenStream {
    // extern mapping is only valid for functions
    let mut item_fn = syn::parse_macro_input!(item as syn::ItemFn);

    let fn_name = item_fn.sig.ident.to_string();
    let is_infallible = attrs.to_string() == "infallible";

    // Check return type
    if let syn::ReturnType::Type(_, ref ty) = item_fn.sig.output {
        const EXTERN_RESULT: &str = "ExternResult";
        const VALIDATE_CALLBACK_RESULT: &str = "ValidateCallbackResult";
        const INIT_CALLBACK_RESULT: &str = "InitCallbackResult";
        const ENTRY_DEFS_CALLBACK_RESULT: &str = "EntryDefsCallbackResult";

        match (fn_name.as_str(), get_return_type_ident(ty)) {
            ("validate" | "genesis_self_check", Some(return_type)) => {
                if is_infallible && return_type != VALIDATE_CALLBACK_RESULT {
                    abort!(
                        ty.span(),
                        "`{}` must return `{}`",
                        fn_name,
                        VALIDATE_CALLBACK_RESULT
                    );
                } else if !is_infallible && !is_callback_result(ty, VALIDATE_CALLBACK_RESULT) {
                    abort!(
                        ty.span(),
                        "`{}` must return `{}<{}>`",
                        fn_name,
                        EXTERN_RESULT,
                        VALIDATE_CALLBACK_RESULT
                    );
                }
            }
            ("init", Some(return_type)) => {
                if is_infallible && return_type != INIT_CALLBACK_RESULT {
                    abort!(
                        ty.span(),
                        "`{}` must return `{}`",
                        fn_name,
                        INIT_CALLBACK_RESULT
                    );
                } else if !is_infallible && !is_callback_result(ty, INIT_CALLBACK_RESULT) {
                    abort!(
                        ty.span(),
                        "`{}` must return `{}<{}>`",
                        fn_name,
                        EXTERN_RESULT,
                        INIT_CALLBACK_RESULT
                    );
                }
            }
            ("entry_defs", Some(return_type)) => {
                if is_infallible && return_type != ENTRY_DEFS_CALLBACK_RESULT {
                    abort!(
                        ty.span(),
                        "`{}` must return `{}`",
                        fn_name,
                        ENTRY_DEFS_CALLBACK_RESULT
                    );
                } else if !is_infallible && !is_callback_result(ty, ENTRY_DEFS_CALLBACK_RESULT) {
                    abort!(
                        ty.span(),
                        "`{}` must return `{}<{}>`",
                        fn_name,
                        EXTERN_RESULT,
                        ENTRY_DEFS_CALLBACK_RESULT
                    );
                }
            }
            ("post_commit", r) => {
                let type_str = quote::quote!(#ty).to_string();

                if r.is_some() && is_infallible {
                    abort!(
                        ty.span(),
                        "`{}` must not have a return type", fn_name;
                        help = "remove the `{}` return type", type_str
                    );
                } else if !is_callback_result(ty, "()") {
                    abort!(
                        ty.span(),
                        "`{}` must return `{}<{}>`",
                        fn_name,
                        EXTERN_RESULT,
                        "()"
                    );
                }
            }
            (_, Some(return_type)) => {
                let type_str = quote::quote!(#ty).to_string();

                if is_infallible && return_type == EXTERN_RESULT {
                    abort!(
                        ty.span(),
                        "infallible functions should return the inner type directly";
                        help = "consider removing the `{}<{}>` wrapper and return the inner type `{}` instead", EXTERN_RESULT, type_str, type_str
                    );
                } else if !is_infallible && return_type != EXTERN_RESULT {
                    abort!(
                        ty.span(),
                        "functions marked with #[hdk_extern] must return `{}<T>`", EXTERN_RESULT;
                        help = "change the return type to `{}<{}>` or mark the function as infallible if it cannot fail", EXTERN_RESULT, type_str
                    );
                }
            }
            _ => {}
        }
    }

    // extract the ident of the fn
    // this will be exposed as the external facing extern
    let external_fn_ident = item_fn.sig.ident.clone();
    if item_fn.sig.inputs.len() > 1 {
        abort_call_site!("hdk_extern functions must take a single parameter or none");
    }
    let input_type = if let Some(syn::FnArg::Typed(pat_type)) = item_fn.sig.inputs.first() {
        pat_type.ty.clone()
    } else {
        let param_type = syn::Type::Verbatim(quote::quote! { () });
        let param_pat = syn::Pat::Wild(syn::PatWild {
            underscore_token: syn::token::Underscore::default(),
            attrs: Vec::new(),
        });
        let param = syn::FnArg::Typed(syn::PatType {
            attrs: Vec::new(),
            pat: Box::new(param_pat),
            colon_token: syn::token::Colon::default(),
            ty: Box::new(param_type.clone()),
        });
        item_fn.sig.inputs.push(param);
        Box::new(param_type)
    };
    let output_type = if let syn::ReturnType::Type(_, ref ty) = item_fn.sig.output {
        ty.clone()
    } else {
        Box::new(syn::Type::Verbatim(quote::quote! { () }))
    };

    let internal_fn_ident = external_fn_ident.clone();

    if is_infallible {
        (quote::quote! {
            map_extern_infallible!(#external_fn_ident, #internal_fn_ident, #input_type, #output_type);
            #item_fn
        })
        .into()
    } else {
        (quote::quote! {
            map_extern!(#external_fn_ident, #internal_fn_ident, #input_type, #output_type);
            #item_fn
        })
        .into()
    }
}

#[proc_macro_error]
#[proc_macro_derive(EntryDefRegistration, attributes(entry_type))]
pub fn derive_entry_type_registration(input: TokenStream) -> TokenStream {
    entry_type_registration::derive(input)
}

#[proc_macro_error]
#[proc_macro_derive(UnitEnum, attributes(unit_enum, unit_attrs))]
pub fn derive_to_unit_enum(input: TokenStream) -> TokenStream {
    unit_enum::derive(input)
}

/// Declares the integrity zome's entry types.
///
/// # Attributes
/// - `unit_enum(TypeName)`: Defines the unit version of this enum. The resulting enum contains all
/// entry types defined in the integrity zome. It can be used to refer to a type when needed.
/// - `entry_def(name: String, required_validations: u8, visibility: String)`: Defines an entry type.
///   - name: The name of the entry definition (optional).
///     Defaults to the name of the enum variant.
///   - required_validations: The number of validations required before this entry
///     will not be published anymore (optional). Defaults to 5.
///   - visibility: The visibility of this entry. [`public` | `private`].
///     Default is `public`.
///
/// # Examples
/// ```ignore
/// #[hdk_entry_types]
/// #[unit_enum(UnitEntryTypes)]
/// pub enum EntryTypes {
///     Post(Post),
///     #[entry_type(required_validations = 5)]
///     Msg(Msg),
///     #[entry_type(name = "hidden_msg", required_validations = 5, visibility = "private")]
///     PrivMsg(PrivMsg),
/// }
/// ```
#[proc_macro_error]
#[proc_macro_attribute]
pub fn hdk_entry_types(attrs: TokenStream, code: TokenStream) -> TokenStream {
    entry_types::build(attrs, code)
}

/// Implements all the required types needed for a `LinkTypes` enum.
#[proc_macro_error]
#[proc_macro_attribute]
pub fn hdk_link_types(attrs: TokenStream, code: TokenStream) -> TokenStream {
    link_types::build(attrs, code)
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn hdk_to_coordinates(attrs: TokenStream, code: TokenStream) -> TokenStream {
    to_coordinates::build(attrs, code)
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn hdk_entry_types_name_registration(attrs: TokenStream, code: TokenStream) -> TokenStream {
    entry_types_name_registration::build(attrs, code)
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn hdk_entry_types_conversions(attrs: TokenStream, code: TokenStream) -> TokenStream {
    entry_types_conversions::build(attrs, code)
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn hdk_dependent_entry_types(attrs: TokenStream, code: TokenStream) -> TokenStream {
    entry_zomes::build(attrs, code)
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn hdk_dependent_link_types(attrs: TokenStream, code: TokenStream) -> TokenStream {
    link_zomes::build(attrs, code)
}

/// Helper for entry data types.
///
/// # Implements
/// - `#[derive(Serialize, Deserialize, SerializedBytes, Debug)]`
/// - `hdi::app_entry!`
///
/// # Examples
/// ```ignore
/// #[hdk_entry_helper]
/// pub struct Post(pub String);
/// ```
#[proc_macro_error]
#[proc_macro_attribute]
pub fn hdk_entry_helper(attrs: TokenStream, code: TokenStream) -> TokenStream {
    entry_helper::build(attrs, code)
}

/// Helper for decoding DNA Properties into a struct.
///
/// # Implements
/// - [`holochain_integrity_types::TryFromDnaProperties`]
///
/// # Examples
/// ```ignore
/// #[dna_properties]
/// pub struct MyDnaProperties {
///     pub progenitor: String,
///     pub max_length: u16,
/// }
///
/// let my_props = MyDnaProperties::try_from_dna_properties()?;
/// println!("The progenitor is {}", my_props.progenitor);
/// ```
#[proc_macro_error]
#[proc_macro_attribute]
pub fn dna_properties(attrs: TokenStream, code: TokenStream) -> TokenStream {
    dna_properties::build(attrs, code)
}
