#![feature(extend_one, proc_macro_quote, let_chains)]

extern crate proc_macro;

use proc_macro::{
    TokenStream,
    TokenTree::{Ident, Punct},
};
use proc_macro2::{Punct as Punct2, Spacing as Spacing2, TokenTree as TokenTree2};
use quote::quote;

#[proc_macro]
pub fn cool_macro(stream: TokenStream) -> TokenStream {
    let mut iter = stream.into_iter();
    let method_ident = match iter.next() {
        Some(t) => match t {
            Ident(i) => i,
            other => panic!("expected identifier, found {}", other),
        },
        None => panic!("expected a method name"),
    };
    let method_name = method_ident.to_string();

    let method_ident = proc_macro2::Ident::new(method_name.as_str(), method_ident.span().into());

    let mut routes_stream = proc_macro2::TokenStream::new();

    let mut variables_defined: Vec<String> = Vec::new();
    let mut wildcard_defined = false;

    while let Some(token) = iter.next() {
        if wildcard_defined {
            panic!("wildcard must terminate path");
        }
        match token {
            Punct(punct) => {
                let char = punct.as_char();
                if char != '/' {
                    panic!("expected `/`, found {}", char)
                }
                match iter.next() {
                    Some(next) => match next {
                        Punct(next) => {
                            let char = next.as_char();
                            if char == '*' {
                                wildcard_defined = true;
                                routes_stream
                                    .extend(quote!(::bingus_http::route::RouteToken::WILDCARD));
                            } else if char == ':' {
                                let var_name = match iter.next() {
                                    Some(next) => match next {
                                        Ident(next) => next.to_string(),
                                        _ => panic!("expected identifier, found {}", next),
                                    },
                                    None => panic!("expected identifier"),
                                };
                                if variables_defined.iter().any(|x| *x == var_name) {
                                    panic!("variable with name `{}` already defined", var_name);
                                }
                                variables_defined.push(var_name.to_string());
                                routes_stream
                                    .extend(quote!(::bingus_http::route::RouteToken::PARAMETER(String::from(#var_name).into_boxed_str())));
                            }
                        }
                        Ident(next) => {
                            let path = next.to_string();
                            routes_stream.extend(
                                quote!(::bingus_http::route::RouteToken::PATH(String::from(#path).into_boxed_str())),
                            );
                        }
                        _ => panic!("expected `*`, `:`, or an identifier, found {}", next),
                    },
                    None => routes_stream.extend(quote!(::bingus_http::route::RouteToken::PATH(
                        String::new().into_boxed_str()
                    ))),
                };
            }
            _ => panic!("expected `/`, found {}", token),
        }

        routes_stream.extend_one(TokenTree2::Punct(Punct2::new(',', Spacing2::Alone)));
    }

    let s = quote! {
        ::bingus_http::Route(::bingus_http::Method::#method_ident, Box::new([#routes_stream]))
    };
    s.into()
}
