use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

use crate::builder_input::BuilderInput;

mod builder_input;

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    // eprintln!("INPUT: {}", input);
    let input = parse_macro_input!(input as BuilderInput);

    let struct_name = &input.ident;

    let builder_name = &input.builder_struct_ident();

    let builder_struct = input.generate_builder_struct();

    let builder_method = input.generate_builder_method();

    let builder_setter_methods = input.generate_setter_methods();

    let builder_final_method = input.generate_final_build_method();

    let derive_impl = quote! {
        impl #struct_name {
            #builder_method
        }

        impl #builder_name {
            #builder_setter_methods
            #builder_final_method
        }
    };

    let stream = quote! {
        #builder_struct
        #derive_impl
    };
    // eprintln!("STREAM: {}", stream);

    proc_macro::TokenStream::from(stream)
}
