use quote::quote;

use crate::custom_debug::CustomDebugInput;

mod custom_debug;
mod derive_trait;

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as CustomDebugInput);
    let debug_impl = input.debug_impl();
    let stream = quote! {
        #debug_impl
    };
    // eprintln!("OUT: {stream}");
    stream.into()
}
