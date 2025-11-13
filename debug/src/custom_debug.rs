use quote::quote;

#[allow(dead_code)]
pub struct CustomDebugInput {
    vis: syn::Visibility,
    struct_token: syn::Token![struct],
    ident: syn::Ident,
    brace_token: syn::token::Brace,
    fields: syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
}

impl syn::parse::Parse for CustomDebugInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let vis: syn::Visibility = input.parse()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::Token![struct]) {
            let content;
            Ok(CustomDebugInput {
                vis,
                struct_token: input.parse()?,
                ident: input.parse()?,
                brace_token: syn::braced!(content in input),
                fields: content.parse_terminated(syn::Field::parse_named, syn::Token![,])?,
            })
        } else {
            Err(lookahead.error())
        }
    }
}

impl CustomDebugInput {
    pub fn debug_impl(&self) -> proc_macro2::TokenStream {
        // let vis = &self.vis;
        let name = &self.ident;
        let recurse = self.fields.pairs().map(|pair| {
            let f = pair.value();
            let f_name = &f.ident.as_ref().expect("field ident");
            let f_name_str = f_name.to_string();
            quote! {
                .field(#f_name_str, &self.#f_name)
            }
        });
        let name_str = name.to_string();
        quote! {
            impl std::fmt::Debug for #name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.debug_struct(#name_str)
                        #(#recurse)*
                        .finish()
                }
            }
        }
    }
}
