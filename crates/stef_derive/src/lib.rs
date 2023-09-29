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
//! The matches() attr is very magical and needs more explanation. But for now:
//! Both sides of the `<=>` are intepreted as both a Pattern, and an Expression. A function can return a type other than
//! the Effect, if a matches() or map_with() attr is provided. The left side of the matches() represents the effect type,
//! and the right side represents the return type. Through this bidirectional mapping (partial isomorphism), we can freely
//! convert between effect and function return types, so that the users of the function don't have to match on the effect
//! (usually an enum) to get the value they want. TODO: write more
//!
//! gen() is also pretty magical. It lets you define new structs with the same functions as the State,
//! but which add extra functionality. For instance:
//!
//!     #[stef:state(gen(struct Quux = Bar Baz Bat))]
//!     impl stef::State<'static> for Foo { ... }
//!
//! This will not only create the usual implementation of State for `Foo`, along with all the provided
//! methods, but will also create a new struct `struct Quux(Bar<Baz<Bat<Foo>>>)`, with a constructor
//! `Quux::new(foo)`, such that `Quux` is also a `State` with the same action and effect types as `Foo`.
//! Bar, Baz, and Bat are wrapper structs which provide extra functionality when processing actions and
//! effects, or granting shared access in a specific way. The `stef::combinators` module contains some
//! built-in wrappers. TODO: write more

use proc_macro2::TokenStream;
use proc_macro_error::{abort, abort_call_site};
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{parse_macro_input, Ident, Pat, Token, Type, Visibility};

mod attr_parsers;
use attr_parsers::*;

mod func;
use func::*;

#[proc_macro_derive(State)]
pub fn derive_state(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut strukt: syn::ItemStruct =
        syn::parse(item).expect("State can only be derived for a struct");

    if strukt.fields.len() != 1 {
        proc_macro_error::abort_call_site!(
            "State can only be derived for a struct with a single tuple field"
        )
    }

    let field = strukt.fields.into_iter().next().unwrap();
    let ty = field.ty;
    let name = strukt.ident;

    let predicate: syn::WherePredicate = syn::parse_quote! {
        #ty: stef::State<'static>
    };
    strukt
        .generics
        .make_where_clause()
        .predicates
        .push(predicate);

    let (igen, tgen, where_clause) = strukt.generics.split_for_impl();

    let mut state_impl: syn::ItemImpl = syn::parse_quote! {
        impl #igen stef::State<'static> for #name #tgen #where_clause {
            type Action = <#ty as stef::State<'static>>::Action;
            type Effect = <#ty as stef::State<'static>>::Effect;

            fn transition(&mut self, action: Self::Action) -> Self::Effect {
                self.0.transition(action)
            }
        }
    };
    state_impl.generics = strukt.generics.clone();

    proc_macro::TokenStream::from(quote! {
        #state_impl
    })
}

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

fn state_impl(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    // Parse top-level attrs
    let Options {
        _parameterized: _,
        wrappers,
        fuzzing,
        recording,
    } = parse_macro_input!(attr as Options);

    let mut action_name = None;
    let mut effect_name = None;

    let item = parse_macro_input!(input as syn::ItemImpl);

    // The path of the type for which State is being impl'd
    let struct_path = match &*item.self_ty {
        Type::Path(path) => path,
        _ => abort!(item.self_ty.span(), "This impl is too fancy"),
    };

    let mut fns = vec![];

    // Pick apart the impl
    for item in item.items {
        match item {
            syn::ImplItem::Type(ty) => {
                if ty.ident == "Action" {
                    // - type Action = Foo
                    action_name = Some(ty.ty.clone());
                } else if ty.ident == "Effect" {
                    // - type Effect = Foo
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
                        // #[stef::state]
                        attr.parse_nested_meta(|meta| {
                            if meta.path.is_ident("matches") {
                                // #[stef::state(matches(_ <=> _))]
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
                                // #[stef::state(map_with(foo))]
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
                            abort_call_site!("couldn't parse function attrs {}", err);
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

    let action_name = action_name.unwrap_or_else(|| abort_call_site!("`type Action` must be set"));
    let effect_name = effect_name.unwrap_or_else(|| abort_call_site!("`type Effect` must be set"));

    // - define `pub enum FooAction`
    let mut define_action_enum: syn::ItemEnum = {
        let variants = delim::<_, Token!(,)>(fns.iter().map(|f| {
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

        let doc = "The Action type for the State (autogenerated by stef::state macro)".to_string();
        syn::parse_quote! {
            #[doc = #doc]
            #[derive(Debug)]
            pub enum #action_name {
                #variants
            }
        }
    };
    define_action_enum.generics = item.generics.clone();

    if recording && cfg!(feature = "recording") {
        define_action_enum.attrs.push(syn::parse_quote!(
            #[derive(::stef::dependencies::serde::Serialize, ::stef::dependencies::serde::Deserialize)]
        ));
    }

    if fuzzing {
        define_action_enum.attrs.push(syn::parse_quote!(
            #[derive(::proptest_derive::Arbitrary)]
        ));
    }

    // - define impl with `_stef_impl_*` functions containing the real logic used to
    //   implement `State::transition`
    let mut define_hidden_fns: syn::ItemImpl = {
        let inner = ss_flatten(fns.iter().map(|f| {
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
        syn::parse(
            quote! {
                impl #struct_path {
                    #inner
                }
            }
            .into(),
        )
        .expect("problem 4!")
    };
    define_hidden_fns.generics = item.generics.clone();

    // - define impl with public functions which perform state transitions, which in turn call the
    //   corresponding hidden functions
    let mut define_public_fns: syn::ItemImpl = {
        let inner = ss_flatten(fns.iter().map(|f| {
            let mut original_func = f.f.clone();
            let variant_name = f.variant_name();

            let pats = delim::<_, Token!(,)>(f.inputs().into_iter().map(|(pat, _)| pat));
            let arg = match pats.is_empty() {
                true => quote! { <Self as stef::State>::Action::#variant_name },
                false => quote! { <Self as stef::State>::Action::#variant_name(#pats) },
            };

            let new_block = f.mapped_block(quote! {
                self.transition(#arg)
            });

            let ts = proc_macro::TokenStream::from(new_block.into_token_stream());
            original_func.block = syn::parse(ts).expect("problem 3!");
            original_func.vis = Visibility::Public(Default::default());
            original_func.to_token_stream()
        }));

        syn::parse_quote! {
            impl #struct_path {
                #inner
            }
        }
    };
    define_public_fns.generics = item.generics.clone();

    // - generated by #[stef::share(wrapper(Foo))]
    let define_wrapper_impls = ss_flatten(wrappers.into_iter().map(|name| {
        let mut define_wrapper_impl: syn::ItemImpl = {
            let inner = ss_flatten(fns.iter().map(|f| {
                let mut original_func = f.f.clone();
                let variant_name = f.variant_name();
                let args = delim::<_, Token!(,)>(f.inputs().into_iter().map(|(id, _)| id));
                let transition = if args.is_empty() {
                    quote! { <Self as stef::State>::Action::#variant_name }
                } else {
                    quote! { <Self as stef::State>::Action::#variant_name(#args) }
                };

                original_func.block = syn::parse(
                    f.mapped_block(quote! {
                        self.transition(#transition)
                    })
                    .into_token_stream()
                    .into(),
                )
                .expect("problem xy8n88!");
                original_func.vis = Visibility::Public(Default::default());
                original_func.to_token_stream()
            }));

            syn::parse(
                quote! {
                    impl #name {
                        #inner
                    }
                }
                .into(),
            )
            .expect("problem xyzzy72!")
        };
        define_wrapper_impl.generics = item.generics.clone();
        define_wrapper_impl.into_token_stream()
    }));

    let (_, item_trait, item_for_token) =
        item.trait_.expect("must use `impl stef::State<_> for ...`");

    // - impl State for StructName
    let mut define_state_impl: syn::ItemImpl = {
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

        let define_match: syn::ExprMatch = syn::parse_quote! {
            match action {
                #define_transitions
            }
        };

        syn::parse_quote! {
            impl #item_trait #item_for_token #struct_path {
                type Action = #action_name;
                type Effect = #effect_name;

                fn transition(&mut self, action: Self::Action) -> Self::Effect {
                    #define_match
                }
            }
        }
    };
    define_state_impl.generics = item.generics;

    let expanded = quote! {
        #define_action_enum
        #define_public_fns
        #define_hidden_fns
        #define_state_impl
        #define_wrapper_impls
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
