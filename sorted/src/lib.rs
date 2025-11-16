use quote::quote;
use syn::{spanned::Spanned, visit_mut::VisitMut};

struct Sorted(syn::ItemEnum);

impl std::ops::Deref for Sorted {
    type Target = syn::ItemEnum;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl syn::parse::Parse for Sorted {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let fork = input.fork();
        fork.parse::<syn::Visibility>()?;
        let lookahead = fork.lookahead1();
        if lookahead.peek(syn::Token![enum]) {
            Ok(Self(input.parse()?))
        } else {
            Err(lookahead.error())
        }
    }
}

impl Sorted {
    fn build(&mut self) -> proc_macro2::TokenStream {
        if let Err(err) = self.check_sorted_variants() {
            let out = &self.0;
            let err = err.into_compile_error();
            quote! {
                #err
                #out
            }
        } else {
            let out = &self.0;
            quote! {
                #out
            }
        }
    }
    fn check_sorted_variants(&mut self) -> syn::Result<()> {
        for (idx, current_pair) in self.variants.pairs().enumerate() {
            let current = &current_pair.value().ident;
            for next_pair in self.variants.pairs().skip(idx + 1) {
                let next = &next_pair.value().ident;

                if current > next {
                    return Err(syn::Error::new(
                        next.span(),
                        format!("{} should sort before {}", &next, &current),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[proc_macro_attribute]
pub fn sorted(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    eprintln!("INPUT: {input}");
    let sorted = syn::parse_macro_input!(input as Sorted).build();
    let _ = args;

    let output = quote! {
        #sorted
    };
    eprintln!("OUTPUT: {output}");
    output.into()
}

#[derive(Default)]
struct CheckFn {
    err: Option<syn::Error>,
}

#[proc_macro_attribute]
pub fn check(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    // eprintln!("INPUT: {input}");

    let mut item_fn: syn::ItemFn = syn::parse_macro_input!(input);

    let mut check_fn = CheckFn::default();
    check_fn.visit_item_fn_mut(&mut item_fn);

    let err = if let Some(ref err) = check_fn.err {
        let out_err = err.to_compile_error();
        quote! {
            #out_err
        }
    } else {
        quote! {}
    };

    quote! {
        #err
        #item_fn
    }
    .into()
}

impl syn::visit_mut::VisitMut for CheckFn {
    fn visit_expr_match_mut(&mut self, i: &mut syn::ExprMatch) {
        let mut sort = None::<usize>;
        for (idx, attr) in i.attrs.iter().enumerate() {
            if attr.path().is_ident("sorted") {
                sort = Some(idx)
            }
        }

        if let Some(attr_idx) = sort {
            let pats: Vec<&syn::Pat> = i.arms.iter().map(|arm| &arm.pat).collect();
            match pat_to_arm_ident(&pats) {
                Ok(idents) => {
                    for (idx, (current, _span)) in idents.iter().enumerate() {
                        for (next, span) in idents.iter().skip(idx + 1) {
                            if current > next {
                                self.err = Some(syn::Error::new(
                                    *span,
                                    format!("{} should sort before {}", &next, &current),
                                ));
                                break;
                            }
                        }
                    }
                }
                Err(err) => self.err = Some(err),
            }
            i.attrs.remove(attr_idx);
        }
    }
}

fn pat_to_arm_ident(pats: &[&syn::Pat]) -> syn::Result<Vec<(String, proc_macro2::Span)>> {
    let mut out: Vec<(String, proc_macro2::Span)> = vec![];

    for pat in pats {
        use syn::Pat::*;
        match pat {
            Ident(ref ident) => out.push((ident.ident.to_string(), ident.span())),
            Or(pat_or) => {
                let pats: Vec<&syn::Pat> =
                    pat_or.cases.pairs().map(|pair| pair.into_value()).collect();
                out.append(&mut pat_to_arm_ident(&pats)?);
            }
            Path(ref path) => {
                out.push((create_string_from_path(&path.path), path.span()));
            }
            TupleStruct(ref tuple) => {
                out.push((create_string_from_path(&tuple.path), tuple.path.span()));
            }
            Wild(ref wild) => {
                out.push(("_".to_string(), wild.span()));
            }
            _ => return Err(syn::Error::new(pat.span(), "unsupported by #[sorted]")),
        }
    }

    Ok(out)
}

fn create_string_from_path(path: &syn::Path) -> String {
    let mut out = String::new();
    path.segments.pairs().for_each(|pair| {
        if !out.is_empty() {
            out.push_str("::");
        }
        out.push_str(&pair.value().ident.to_string());
    });
    out
}
