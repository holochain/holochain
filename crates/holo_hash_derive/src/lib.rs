use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(HashEncoding)]
#[proc_macro_error::proc_macro_error]
pub fn derive_state(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match input.data {
        syn::Data::Struct(s) => {
            let fields = s.fields;
            let fields = match fields {
                syn::Fields::Named(fields) => fields.named,
                syn::Fields::Unnamed(_) => {
                    unimplemented!("HashEncoding cannot be derived for tuple structs")
                }
                syn::Fields::Unit => {
                    unimplemented!("HashEncoding cannot be derived for unit structs")
                }
            };
            let fields = fields.iter().map(|f| f.ident.as_ref().unwrap());
            let name = input.ident;
            let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
            let output = quote::quote! {
                impl #impl_generics holo_hash::HashEncoding for #name #ty_generics #where_clause {
                    fn hash_encoding<H>(&self) -> #name<H> where H: holo_hash::HashType {
                        #name {
                            #(
                                #fields: self.#fields.hash_encoding(),
                            )*
                        }
                    }
                }
            };
            output.into()
        }
        syn::Data::Enum(_) => todo!(),
        syn::Data::Union(_) => unimplemented!("HashEncoding cannot be derived for unions"),
    }
}
