#![feature(proc_macro_diagnostic)]
extern crate proc_macro;

use self::proc_macro::TokenStream;
use heck::{CamelCase, ShoutySnekCase};
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, Ident, ItemFn, ReturnType};
use syn::spanned::Spanned;

#[proc_macro_attribute]
pub fn kernel_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input: ItemFn = parse_macro_input!(item as ItemFn);

    if input.asyncness.is_some() {
        input.span().unwrap().error("Test cases cannot be async").emit();
        return TokenStream::new();
    }

    if !input.decl.generics.params.is_empty() {
        input.decl.generics.span().unwrap().error("Test cases cannot be generic").emit();
        return TokenStream::new();
    }

    if !input.decl.inputs.is_empty() {
        input.decl.inputs.span().unwrap().error("Test cases cannot take arguments").emit();
        return TokenStream::new();
    }

    match input.decl.output {
        ReturnType::Default => (),
        _ => {
            input.decl.output.span().unwrap().error("Test cases cannot return values").emit();
            return TokenStream::new();
        }
    }

    let block = input.block;
    let body = if input.unsafety.is_some() {
        quote! { unsafe #block }
    } else {
        quote! { #block }
    };

    let test_name = input.ident.to_string();
    let struct_name = format!("TestCase{}", test_name.to_camel_case());
    let struct_ident = Ident::new(&struct_name, Span::call_site());

    let global_ident = Ident::new(&test_name.TO_SHOUTY_SNEK_CASE(), Span::call_site());

    let out = quote! {
        pub struct #struct_ident;

        impl ::platypos_test::TestCase for #struct_ident {
            fn name(&self) -> &'static str {
                #test_name
            }

            fn run(&self) {
                #body
            }
        }

        #[test_case]
        pub static #global_ident: #struct_ident = #struct_ident;
    };

    out.into()
}