#![feature(proc_macro_diagnostic)]

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{parse_macro_input, ItemFn, ReturnType};

#[proc_macro_attribute]
pub fn test(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as ItemFn);

    proc_macro::TokenStream::from(generate_test(input))
}

fn generate_test(input: ItemFn) -> TokenStream {
    if let Some(asyncness) = input.sig.asyncness {
        asyncness
            .span()
            .unwrap()
            .error("Tests cannot be `async`")
            .emit();
        return TokenStream::new();
    }

    if !input.sig.inputs.is_empty() {
        input
            .sig
            .inputs
            .span()
            .unwrap()
            .error("Tests cannot take arguments")
            .emit();
        return TokenStream::new();
    }

    let test_name = input.sig.ident;
    let static_name = format_ident!("REGISTER_{}", test_name);
    let impl_name = format_ident!("{}_impl", test_name);

    let test_full_name = quote! {
        concat!(module_path!(), "::", stringify!(#test_name))
    };

    let test_impl = match input.sig.output {
        ReturnType::Default => {
            let body = input.block;
            quote! {
                let _ktest_span = ::ktest::info_span!(stringify!(#test_name)).entered();

                #body

                ::ktest::Outcome::Pass
            }
        }
        ReturnType::Type(_, _) => {
            input
                .sig
                .output
                .span()
                .unwrap()
                .error("Tests cannot return anything")
                .emit();
            return TokenStream::new();
        }
    };

    let expanded = quote! {
        #[::ktest::linkme::distributed_slice(::ktest::TESTS)]
        #[linkme(crate = ::ktest::linkme)]
        #[allow(non_upper_case_globals)]
        static #static_name: ::ktest::Test =
          ::ktest::Test::new(#test_full_name, #impl_name);

        fn #impl_name() -> ::ktest::Outcome {
            #test_impl
        }
    };

    expanded
}
