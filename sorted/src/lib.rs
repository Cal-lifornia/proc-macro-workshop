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
                    for (idx, current) in idents.iter().enumerate() {
                        for next in idents.iter().skip(idx + 1) {
                            if current > next {
                                self.err = Some(syn::Error::new(
                                    next.span(),
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

fn pat_to_arm_ident(pats: &[&syn::Pat]) -> syn::Result<Vec<syn::Ident>> {
    let mut out: Vec<syn::Ident> = vec![];

    for pat in pats {
        use syn::Pat::*;
        match pat {
            Ident(ref ident) => out.push(ident.ident.clone()),
            Or(pat_or) => {
                let pats: Vec<&syn::Pat> =
                    pat_or.cases.pairs().map(|pair| pair.into_value()).collect();
                out.append(&mut pat_to_arm_ident(&pats)?);
            }
            Path(path) => {
                out.push(path.path.segments.last().unwrap().ident.clone());
            }
            TupleStruct(tuple) => tuple.path,
            _ => {
                return Err(syn::Error::new(
                    pat.span(),
                    "match pattern type not suppported in [sorted]",
                ))
            }
        }
    }

    Ok(out)
}
