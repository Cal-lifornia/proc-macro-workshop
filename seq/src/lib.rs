use quote::quote;
use syn::spanned::Spanned;

#[allow(dead_code)]
struct Sequence {
    number: syn::Ident,
    in_token: syn::Token![in],
    range: syn::Expr,
    brace: syn::token::Brace,
    expr: proc_macro2::TokenStream,
}

impl syn::parse::Parse for Sequence {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let n: syn::Ident = input.parse()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::Token![in]) {
            let content;
            Ok(Sequence {
                number: n,
                in_token: input.parse()?,
                range: input.call(syn::Expr::parse_without_eager_brace)?,
                brace: syn::braced!(content in input),
                expr: content.parse()?,
            })
        } else {
            Err(lookahead.error())
        }
    }
}

#[proc_macro]
pub fn seq(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let sequence: Sequence = syn::parse_macro_input!(input);

    if let syn::Expr::Range(range) = sequence.range {
        let number = &sequence.number;
        let expr = &sequence.expr;
        let stream = quote! {
            #expr
        };
        eprintln!("OUTPUT: {stream}");
        stream.into()
    } else {
        syn::Error::new(sequence.range.span(), "expected range expression")
            .into_compile_error()
            .into()
    }
}
