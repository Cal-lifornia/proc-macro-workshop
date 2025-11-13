use quote::{quote, quote_spanned};
use syn::{parse_quote, spanned::Spanned};

#[allow(dead_code)]
pub struct CustomDebugInput {
    attr: CustomDebugAttribute,
    vis: syn::Visibility,
    struct_token: syn::Token![struct],
    ident: syn::Ident,
    generics: syn::Generics,
    brace_token: syn::token::Brace,
    fields: syn::punctuated::Punctuated<CustomDebugField, syn::Token![,]>,
}

impl syn::parse::Parse for CustomDebugInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attr: CustomDebugAttribute = input.parse()?;
        if attr.debug.is_some() {
            return Err(syn::Error::new(
                attr.span.expect("Some(span)"),
                "direct debug attribute only accepted on fields",
            ));
        }
        let vis: syn::Visibility = input.parse()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::Token![struct]) {
            let content;
            Ok(CustomDebugInput {
                attr,
                vis,
                struct_token: input.parse()?,
                ident: input.parse()?,
                generics: input.parse()?,
                brace_token: syn::braced!(content in input),
                fields: content.parse_terminated(CustomDebugField::parse, syn::Token![,])?,
            })
        } else {
            Err(lookahead.error())
        }
    }
}

struct CustomDebugAttribute {
    debug: Option<syn::LitStr>,
    bound: Option<syn::WherePredicate>,
    span: Option<proc_macro2::Span>,
}

impl syn::parse::Parse for CustomDebugAttribute {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs: Vec<syn::Attribute> = input.call(syn::Attribute::parse_outer)?;
        let debug_attrs: Vec<&syn::Attribute> = attrs
            .iter()
            .filter(|attr| attr.path().is_ident("debug"))
            .collect();
        let mut debug_attribute = CustomDebugAttribute {
            debug: None,
            bound: None,
            span: None,
        };
        if let Some(attr) = debug_attrs.first() {
            debug_attribute.span = Some(attr.span());

            match &attr.meta {
                syn::Meta::Path(_) => {
                    return Err(syn::Error::new(
                        attr.span(),
                        "expected `debug = \" ... \" or debug(\" ... \")`",
                    ))
                }
                syn::Meta::List(ref meta_list) => {
                    meta_list.parse_nested_meta(|meta| {
                        if meta.path.is_ident("bound") {
                            let lit_str: syn::LitStr = meta.value()?.parse()?;
                            debug_attribute.bound = Some(syn::parse_str(&lit_str.value())?);
                            Ok(())
                        } else {
                            Err(syn::Error::new(
                                meta_list.span(),
                                "expected `debug(bound = \" ... \")`",
                            ))
                        }
                    })?;
                }
                syn::Meta::NameValue(meta_name_value) => {
                    if let syn::Expr::Lit(ref lit) = &meta_name_value.value {
                        if let syn::Lit::Str(litstr) = &lit.lit {
                            debug_attribute.debug = Some(litstr.clone());
                        } else {
                            return Err(syn::Error::new(attr.span(), "expected `debug = \"...\"`"));
                        }
                    } else {
                        return Err(syn::Error::new(attr.span(), "expected `debug = \"...\"`"));
                    };
                }
            }
        }
        Ok(debug_attribute)
    }
}

struct CustomDebugField {
    field: syn::Field,
    attr: CustomDebugAttribute,
}

impl syn::parse::Parse for CustomDebugField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attr: CustomDebugAttribute = input.parse()?;
        Ok(CustomDebugField {
            field: syn::Field::parse_named(input)?,
            attr,
        })
    }
}

struct GenericBoundsResult<'a> {
    skip: Vec<&'a syn::Ident>,
    where_clause: proc_macro2::TokenStream,
}

impl CustomDebugInput {
    pub fn debug_impl(&self) -> proc_macro2::TokenStream {
        // let vis = &self.vis;
        let name = &self.ident;

        let impl_decl = if let Some(bound) = self.attr.bound.as_ref() {
            let (impl_generics, ty_generics, _) = self.generics.split_for_impl();
            let where_clause = quote! {
                where #bound,
            };
            quote! {
                impl #impl_generics std::fmt::Debug for #name #ty_generics #where_clause
            }
        } else {
            let generic_bounds = self.find_generic_bounds();
            let where_clause = &generic_bounds.where_clause;
            let generics = self.add_trait_bounds(&generic_bounds);
            let (impl_generics, ty_generics, _) = generics.split_for_impl();
            quote! {
                impl #impl_generics std::fmt::Debug for #name #ty_generics #where_clause
            }
        };

        let recurse = self.fields.pairs().map(|pair| {
            let f = pair.value();
            let f_name = &f.field.ident.as_ref().expect("field ident");
            let f_name_str = f_name.to_string();

            if let Some(ref debug_fmt) = &f.attr.debug {
                quote! {
                    .field(#f_name_str, &format_args!(#debug_fmt, self.#f_name))
                }
            } else {
                quote! {
                    .field(#f_name_str, &self.#f_name)
                }
            }
        });
        let name_str = name.to_string();
        quote_spanned! { self.ident.span() =>
            #impl_decl {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.debug_struct(#name_str)
                        #(#recurse)*
                        .finish()
                }
            }
        }
    }

    // Adds trait bounds to the generics on self
    fn add_trait_bounds(&self, generic_bounds: &GenericBoundsResult) -> syn::Generics {
        let mut generics = self.generics.clone();
        for param in &mut generics.params {
            if let syn::GenericParam::Type(ref mut type_param) = *param {
                if !generic_bounds.skip.contains(&&type_param.ident) {
                    type_param.bounds.push(parse_quote!(std::fmt::Debug));
                }
            }
        }
        generics
    }

    fn find_generic_bounds(&self) -> GenericBoundsResult<'_> {
        let generic_type_idents: Vec<&syn::Ident> = self
            .generics
            .type_params()
            .map(|type_param| &type_param.ident)
            .collect();
        let mut skip = vec![];
        let mut recurse = vec![];

        for pair in self.fields.pairs() {
            let f = pair.value();
            if let Some(bound_str) = f.attr.bound.as_ref() {
                recurse.push(quote_spanned! {f.field.span() =>
                    #bound_str
                });
                continue;
            }
            if let syn::Type::Path(path) = &f.field.ty {
                for segment in &path.path.segments {
                    if let syn::PathArguments::AngleBracketed(angle_brackets) = &segment.arguments {
                        for arg in &angle_brackets.args {
                            if let syn::GenericArgument::Type(syn::Type::Path(ref segments, ..)) =
                                arg
                            {
                                // It's an associated type if the first path segment matches
                                // the generic type param in the collected struct
                                if segments.path.segments.first().is_some_and(|segment| {
                                    generic_type_idents.contains(&&segment.ident)
                                }) && segments.path.segments.len() > 1
                                {
                                    let path = &segments.path;
                                    recurse.push(quote_spanned! {arg.span() =>
                                        #path: std::fmt::Debug
                                    });
                                }
                                for s in &segments.path.segments {
                                    skip.push(&s.ident);
                                }
                            }
                        }
                    }
                }
            }
        }

        let where_clause = quote! {
            where #(#recurse),*
        };
        GenericBoundsResult { skip, where_clause }
    }
}
