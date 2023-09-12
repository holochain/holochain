//! Macro to do a lot of the legwork of implementing `stef::State`
//! and interfacing with that implementation.
//!
//! Normally, to implement `State`, you must:
//! - define an Action type
//! - define an Effect type
//! - implement the `State::transition` function, taking an Action and returning an Effect
//!
//! This is uncomfortable to work with though: rather than calling functions on your state,
//! you always have to do an explicit `transition`, using an enum instead of normal function args.
//!
//! This macro lets you define your Actions as individual functions returning Effects, rather than
//! needing to define a single transition function to handle each Action. The macro generates an
//! Action type for you based on the functions you define, and the functions get rewritten such that:
//!
//! - Each input signature corresponds to an Action variant
//! - The function maps its inputs into the correct Action variant and passes that to `State::transition`
//! - The `State::transition` function is written for you automatically
//! - Optionally, each function can specify a "matches" pattern, for situations where a particular
//!     action can only ever produce a subset of possible effects. This allows the functions to return
//!     something other than the Effect type, which makes calling the function more ergonomic, so that
//!     you don't have to re-map the Effect at the call site. (Under the hood, every Action still
//!     produces the same Effect type.)
//!
//! ## Implementation details
//!
//! To help with following along with what this macro is doing, the rewriting is done roughly as follows:
//!
//! - The `type Action` and `type Effect` statements are parsed to learn those types
//! - The function definitions are collected
//! - The Action enum is built up, with each variant defined according to the function inputs
//! - The original `impl<_> State` block is gutted and replaced with a `transition` fn, completing the State implementation
//! - A new `impl` block is created containing the original function bodies, but with the function names prefixed by `_stef_impl_`
//!     and private visibility (these should never be leaked or otherwise called directly, it would defeat the entire purpose!)
//! - Another new `impl` block is created with the original function names, but with bodies that simply call the `transition`
//!     function with the `Action` corresponding to this function. If a `matches` directive was provided, the pattern is
//!     applied to the output to map the return type
//!
//! TODO: the matches() attr is very magical and needs more explanation. But for now:
//! Both sides of the `<=>` are intepreted as both a Pattern, and an Expression. A function can return a type other than
//! the Effect, if a matches() or map_with() attr is provided. The left side of the matches() represents the effect type,
//! and the right side represents the return type. Through this bidirectional mapping (partial isomorphism), we can freely
//! convert between effect and function return types, so that the users of the function don't have to match on the effect
//! (usually an enum) to get the value they want

use std::collections::HashMap;

use heck::ToPascalCase;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::abort;
use quote::{quote, ToTokens};
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{parse_macro_input, Expr, Ident, Pat, Token, Type};

#[proc_macro_attribute]
#[proc_macro_error::proc_macro_error]
pub fn state(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    if let Some(proc_macro::TokenTree::Ident(i)) = item.clone().into_iter().next() {
        if &i.to_string() == "impl" {
            return state_impl(attr, item);
        }
    }
    item
}

#[derive(Default)]
struct Options {
    _parameterized: Option<syn::Path>,
    gen_paths: HashMap<syn::Type, Vec<syn::Path>>,
}

impl syn::parse::Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // let mut this = Self {
        //     _parameterized: Default::default(),
        //     share_type: Default::default(),
        //     gen_paths: Default::default(),
        // };
        let mut this = Self::default();

        while !input.is_empty() {
            let key: syn::Path = input.parse()?;
            if key.is_ident("gen") {
                let content;
                syn::parenthesized!(content in input);
                let _: syn::Token![struct] = content.parse()?;
                let struct_name: syn::Type = content.parse()?;
                let _: syn::Token![=] = content.parse()?;
                let mut items: Vec<syn::Path> = vec![];
                while !content.is_empty() {
                    items.push(content.parse()?);
                }
                this.gen_paths.insert(struct_name, items);
            }
            let _: syn::Result<syn::Token![,]> = input.parse();
        }

        Ok(this)
    }
}

struct MatchPat {
    forward_pat: Pat,
    forward_expr: Expr,
    backward_pat: Pat,
    backward_expr: Expr,
}

type MapWith = syn::Path;

impl syn::parse::Parse for MatchPat {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let forward_pat = Pat::parse_single(input)?;
        let backward_expr = syn::parse(forward_pat.to_token_stream().into())?;

        #[allow(clippy::nonminimal_bool)]
        if true
            && input.parse::<Token!(<)>().is_ok()
            && input.parse::<Token!(=)>().is_ok()
            && input.parse::<Token!(>)>().is_ok()
        {
            let backward_pat = Pat::parse_single(input)?;
            let forward_expr = syn::parse(backward_pat.to_token_stream().into())?;
            Ok(Self {
                forward_pat,
                forward_expr,
                backward_pat,
                backward_expr,
            })
        } else {
            Err(syn::Error::new(
                forward_pat.span(),
                "Expected <=> in `matches()`",
            ))
        }
    }
}

fn state_impl(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let Options {
        _parameterized: _,
        gen_paths,
    } = parse_macro_input!(attr as Options);

    let mut action_name = None;
    let mut effect_name = None;

    let item = parse_macro_input!(input as syn::ItemImpl);

    // let struct_ty = item.self_ty.clone();
    let struct_path = match &*item.self_ty {
        Type::Path(path) => path,
        _ => abort!(item.self_ty.span(), "This impl is too fancy"),
    };

    struct F {
        f: syn::ImplItemFn,
        match_pats: Vec<MatchPat>,
        map_with: Option<MapWith>,
    }

    impl F {
        fn _name(&self) -> Ident {
            syn::Ident::new(&self.f.sig.ident.to_string(), self.f.span())
        }
        fn impl_name(&self) -> Ident {
            syn::Ident::new(&format!("_stef_impl_{}", self.f.sig.ident), self.f.span())
        }
        fn variant_name(&self) -> Ident {
            syn::Ident::new(
                &self.f.sig.ident.to_string().to_pascal_case(),
                self.f.span(),
            )
        }
        fn inputs(&self) -> Vec<(Box<Pat>, Box<Type>)> {
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
    }

    let mut fns = vec![];

    for item in item.items {
        match item {
            syn::ImplItem::Type(ty) => {
                if ty.ident == "Action" {
                    action_name = Some(ty.ty.clone());
                } else if ty.ident == "Effect" {
                    effect_name = Some(ty.ty.clone());
                }
            }
            syn::ImplItem::Fn(f) => {
                let span = f.span();

                let mut match_pats = vec![];
                let mut map_with = None;

                for attr in f.attrs.iter() {
                    if attr.path().segments.last().map(|s| s.ident.to_string())
                        == Some("state".to_string())
                    {
                        attr.parse_nested_meta(|meta| {
                            if meta.path.is_ident("matches") {
                                let content;
                                syn::parenthesized!(content in meta.input);
                                while !content.is_empty() {
                                    let mp: MatchPat =
                                        content.parse().map_err(|e| syn::Error::new(span, e))?;
                                    match_pats.push(mp);
                                    let _: syn::Result<syn::Token![,]> = content.parse();
                                }
                                return Ok(());
                            } else if meta.path.is_ident("map_with") {
                                let content;
                                syn::parenthesized!(content in meta.input);
                                let mw: MapWith =
                                    content.parse().map_err(|e| syn::Error::new(span, e))?;
                                map_with = Some(mw);
                                return Ok(());
                            }
                            Ok(())
                        })
                        .unwrap_or_else(|err| {
                            abort!("blah {}", err);
                        })
                    }
                }

                fns.push(F {
                    f,
                    match_pats,
                    map_with,
                });
            }
            _ => {}
        }
    }

    let action_name =
        action_name.unwrap_or_else(|| abort!(Span::call_site(), "`type Action` must be set"));
    let effect_name =
        effect_name.unwrap_or_else(|| abort!(Span::call_site(), "`type Effect` must be set"));

    let define_action_enum_variants = delim::<_, Token!(,)>(fns.iter().map(|f| {
        let params = delim::<_, Token!(,)>(f.inputs().into_iter().map(|(_, ty)| ty));
        let variant_name = f.variant_name();
        let doc = ss_flatten(
            f.f.attrs
                .iter()
                .filter(|a| a.path().is_ident("doc"))
                .cloned()
                .map(|a| a.into_token_stream()),
        );
        if params.is_empty() {
            quote! {
                #doc
                #variant_name
            }
            .to_token_stream()
        } else {
            quote! {
                #doc
                #variant_name(#params)
            }
            .to_token_stream()
        }
    }));

    // if define_action_enum_variants.is_empty() {
    //     abort!(
    //         Span::call_site(),
    //         "at least one function must be provided to create the Action enum"
    //     )
    // }

    let doc = "The Action type for the State (autogenerated by stef::state macro)".to_string();
    let mut define_action_enum: syn::ItemEnum = syn::parse(
        quote! {
            #[doc = #doc]
            #[derive(Debug)]
            // TODO: don't depend on this feature being available in the consuming crate
            #[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
            pub enum #action_name {
                #define_action_enum_variants
            }
        }
        .into(),
    )
    .expect("problem 2!");
    define_action_enum.generics = item.generics.clone();

    let define_hidden_fns_inner = ss_flatten(fns.iter().map(|f| {
        let mut original_func = f.f.clone();

        original_func.sig.ident = syn::Ident::new(
            &format!("_stef_impl_{}", original_func.sig.ident),
            f.f.span(),
        );

        // let (rarr, output_type) = match &mut original_func.sig.output {
        //     syn::ReturnType::Default => abort!(f.f.span(), "functions must return a type"),
        //     syn::ReturnType::Type(rarr, t) => {
        //         let actual = t.clone();

        //         // *t = Box::new(effect_name.clone());

        //         (rarr, actual)
        //     }
        // };
        // original_func.sig.output = syn::ReturnType::Type(*rarr, output_type);

        // let impl_name = f.impl_name();
        // let block = f.f.block;
        // let args = delim::<_, Token!(,)>(f.inputs().iter().map(|(pat, ty)| quote! { #pat: #ty }));
        original_func.to_token_stream()
        // quote! {
        //    fn #impl_name(&mut self, #args) -> #output_type {
        //        #block
        //    }
        // }
    }));

    let define_public_fns_inner = ss_flatten(fns.iter().map(|f| {
        let mut original_func = f.f.clone();
        let variant_name = f.variant_name();

        let pats = delim::<_, Token!(,)>(f.inputs().into_iter().map(|(pat, _)| pat));
        let arg = match pats.is_empty() {
            true => quote! { <Self as stef::State>::Action::#variant_name },
            false => quote! { <Self as stef::State>::Action::#variant_name(#pats) },
        };

        let new_block = match (f.map_with.as_ref(), f.match_pats.is_empty()) {
            (Some(mw), _) => {
                quote! {{
                    use stef::State;
                    let eff = self.transition(#arg);
                    #mw(eff)
                }}
            }
            (None, false) => {
                let pats = delim::<_, Token!(,)>(f.match_pats.iter().map(
                    |MatchPat {
                         forward_pat,
                         forward_expr,
                         ..
                     }| quote!(#forward_pat => #forward_expr),
                ));
                quote! {{
                    use stef::State;
                    let eff = self.transition(#arg);

                    match eff {
                        #pats,
                        _ => unreachable!("stef::state is using some invalid logic in its matches() attr")
                    }
                }}
            }
            (None, true) => {
                quote! {{
                    use stef::State;
                    self.transition(#arg)
                }}
            }
        };

        let ts = proc_macro::TokenStream::from(new_block.into_token_stream());
        original_func.block = syn::parse(ts).expect("problem 3!");
        original_func.to_token_stream()
    }));

    let mut define_hidden_fns: syn::ItemImpl = syn::parse(
        quote! {
            impl #struct_path {
                #define_hidden_fns_inner
            }
        }
        .into(),
    )
    .expect("problem 4!");
    define_hidden_fns.generics = item.generics.clone();

    let mut define_public_fns: syn::ItemImpl = syn::parse(
        quote! {
            impl #struct_path {
                #define_public_fns_inner
            }
        }
        .into(),
    )
    .expect("problem 5!");
    define_public_fns.generics = item.generics.clone();

    let (_, item_trait, item_for_token) = item
        .trait_
        .clone()
        .expect("must use `impl stef::State<_> for ...`");

    let define_gen_impls = ss_flatten(gen_paths.into_iter().map(|(name, paths)| {
        let mut inner = struct_path.to_token_stream();
        for path in paths.iter() {
            inner = quote! { #path<#inner>};
        }

        let mut construction = struct_path.to_token_stream();
        for path in paths {
            construction = quote! { #path::new(#construction)};
        }

        let define_gen_fns_inner = ss_flatten(fns.iter().map(|f| {
            let mut original_func = f.f.clone();
            let variant_name = f.variant_name();
            let args = delim::<_, Token!(,)>(f.inputs().into_iter().map(|(id, _)| id));
            let transition = if args.is_empty() {
                quote! { <Self as stef::State>::Action::#variant_name }
            } else {
                quote! { <Self as stef::State>::Action::#variant_name(#args) }
            };
            match original_func
                .sig
                .inputs
                .first_mut()
                .expect("problem xyzzy9283!")
            {
                syn::FnArg::Receiver(ref mut r) => {
                    r.mutability = None;
                    match r.ty.as_mut() {
                        Type::Reference(r) => r.mutability = None,
                        _ => unreachable!(),
                    }
                    // r.ty.as_mut().mutability = None;
                    // panic!("{:#?}", r.ty);
                }
                syn::FnArg::Typed(_) => unreachable!(),
            }
            original_func.block = syn::parse(
                quote! {{
                    self.0.transition(#transition)
                }}
                .into_token_stream()
                .into(),
            )
            .expect("problem xyzzy48!");
            original_func.to_token_stream()
        }));

        let mut define_gen_struct: syn::ItemStruct = {
            syn::parse(
                quote! {
                    /// Newtype for shared access to a `stef::State`, autogenerated
                    /// by the `#[stef::state]` attribute macro.
                    #[derive(Clone, Debug)]
                    pub struct #name(#inner);
                }
                .into(),
            )
            .expect("problem xyzzy29!")
        };
        define_gen_struct.generics = item.generics.clone();

        let mut define_gen_impl: syn::ItemImpl = syn::parse(
            quote! {
                impl #name {

                    /// Constructor
                    pub fn new(data: #struct_path) -> Self {
                        Self(stef::Share::new(data))
                    }

                    #define_gen_fns_inner
                }
            }
            .into(),
        )
        .expect("problem xyzzy72!");
        define_gen_impl.generics = item.generics.clone();

        let mut define_deref_impl: syn::ItemImpl = syn::parse(
            quote! {
                impl std::ops::Deref for #name {
                    type Target = #inner;

                    fn deref(&self) -> &Self::Target {
                        &self.0
                    }
                }
            }
            .into(),
        )
        .expect("problem 7232!");
        define_deref_impl.generics = item.generics.clone();

        let mut define_gen_state_impl: syn::ItemImpl = syn::parse(
            quote! {
                impl #item_trait #item_for_token #name {
                    type Action = #action_name;
                    type Effect = #effect_name;

                    fn transition(&mut self, action: Self::Action) -> Self::Effect {
                        self.0.transition(action)
                    }
                }
            }
            .into(),
        )
        .expect("problem 84842n!");
        define_gen_state_impl.generics = item.generics.clone();

        quote! {
            #define_gen_struct
            #define_gen_impl
            #define_gen_state_impl
            #define_deref_impl
        }
    }));

    // let action_name_generic = match action_name.clone() {
    //     Type::Path(mut path) => {
    //         path.path.segments.last_mut().expect("problem 6!").arguments = struct_path
    //             .path
    //             .segments
    //             .last()
    //             .expect("problem 7!")
    //             .arguments
    //             .clone();
    //         path
    //     }
    //     _ => todo!(),
    // };
    let _action_name_nogeneric = match action_name.clone() {
        Type::Path(mut path) => {
            path.path.segments.last_mut().expect("problem 6!").arguments = Default::default();
            path
        }
        _ => todo!(),
    };

    let define_transitions = delim::<_, Token!(,)>(fns.iter().map(|f| {
        let args = delim::<_, Token!(,)>(f.inputs().into_iter().map(|(pat, _)| pat));
        let variant_name = f.variant_name();
        let impl_name = f.impl_name();
        let pats = delim::<_, Token!(,)>(f.match_pats.iter().map(
            |MatchPat {
                 backward_pat,
                 backward_expr,
                 ..
             }| quote!(#backward_pat => #backward_expr),
        ));

        let (pat, expr) = if args.is_empty() {
            (quote!(Self::Action::#variant_name), quote!(self.#impl_name()))
        } else {
            (quote!(Self::Action::#variant_name(#args)), quote!(self.#impl_name(#args)))
        };

        if pats.is_empty() {
            quote! {
                #pat => #expr.into()
            }
        } else {
            quote! {
                #pat => match #expr {
                    #pats,
                    _ => unreachable!("stef::state is using some invalid logic in its matches() attr")
                }
            }
        }
    }));

    let define_match: syn::ExprMatch = syn::parse(
        quote! {
            match action {
                #define_transitions
            }
        }
        .into(),
    )
    .expect("problem 8sd");

    let (_, item_trait, item_for_token) =
        item.trait_.expect("must use `impl stef::State<_> for ...`");

    let mut define_state_impl: syn::ItemImpl = syn::parse(
        quote! {
            impl #item_trait #item_for_token #struct_path {
                type Action = #action_name;
                type Effect = #effect_name;

                fn transition(&mut self, action: Self::Action) -> Self::Effect {
                    #define_match
                }
            }
        }
        .into(),
    )
    .expect("problem 8!");

    define_state_impl.generics = item.generics;

    let expanded = quote! {
        #define_action_enum
        #define_public_fns
        #define_hidden_fns
        #define_state_impl
        #define_gen_impls
    };

    proc_macro::TokenStream::from(expanded)
}

fn delim<T: ToTokens, P: ToTokens + Default>(ss: impl Iterator<Item = T>) -> Punctuated<T, P> {
    let mut items = Punctuated::<T, P>::new();
    items.extend(ss);
    items
}

fn ss_flatten(ss: impl Iterator<Item = TokenStream>) -> TokenStream {
    ss.fold(TokenStream::new(), |mut ss, s| {
        ss.extend(s);
        ss
    })
}
