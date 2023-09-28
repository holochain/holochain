use heck::CamelCase;

use super::*;

pub type MapWith = syn::Path;

/// A parsed function in an impl trait body
pub struct F {
    /// The original function
    pub f: syn::ImplItemFn,
    /// The matches() attr
    pub match_pats: Vec<MatchPat>,
    /// The map_with() attr
    pub map_with: Option<MapWith>,
}

impl F {
    pub fn _name(&self) -> Ident {
        syn::Ident::new(&self.f.sig.ident.to_string(), self.f.span())
    }
    pub fn impl_name(&self) -> Ident {
        syn::Ident::new(&format!("_stef_impl_{}", self.f.sig.ident), self.f.span())
    }
    pub fn variant_name(&self) -> Ident {
        syn::Ident::new(&self.f.sig.ident.to_string().to_camel_case(), self.f.span())
    }
    pub fn inputs(&self) -> Vec<(Box<Pat>, Box<Type>)> {
        let mut f_inputs = self.f.sig.inputs.iter();
        match f_inputs.next() {
            Some(syn::FnArg::Receiver(r)) if r.mutability.is_some() => (),
            o => {
                abort!(
                    o.span(),
                    "#[stef::state] must take &mut self as first argument",
                )
            }
        }

        f_inputs
            .map(|i| match i {
                syn::FnArg::Typed(arg) => (arg.pat.clone(), arg.ty.clone()),
                _ => unreachable!(),
            })
            .collect()
    }

    pub fn mapped_block(&self, val: TokenStream) -> TokenStream {
        match (self.map_with.as_ref(), self.match_pats.is_empty()) {
            (Some(mw), _) => {
                quote! {{
                    use stef::State;
                    let eff = #val;
                    #mw(eff)
                }}
            }
            (None, false) => {
                let pats = delim::<_, Token!(,)>(self.match_pats.iter().map(
                    |MatchPat {
                         forward_pat,
                         forward_expr,
                         ..
                     }| quote!(#forward_pat => #forward_expr),
                ));
                quote! {{
                    use stef::State;
                    let eff = #val;

                    match eff {
                        #pats,
                        _ => unreachable!("stef::state is using some invalid logic in its matches() attr")
                    }
                }}
            }
            (None, true) => {
                quote! {{
                    use stef::State;
                    #val
                }}
            }
        }
    }
}
