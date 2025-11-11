use syn::{braced, spanned::Spanned, Attribute, Field, Token};

use quote::{quote, quote_spanned};
#[allow(dead_code)]
pub struct BuilderInput {
    pub vis: syn::Visibility,
    pub struct_token: Token![struct],
    pub ident: syn::Ident,
    pub brace_token: syn::token::Brace,
    fields: syn::punctuated::Punctuated<BuilderInputField, Token![,]>,
}

impl syn::parse::Parse for BuilderInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let vis: syn::Visibility = input.parse()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![struct]) {
            let content;
            Ok(BuilderInput {
                vis,
                struct_token: input.parse()?,
                ident: input.parse()?,
                brace_token: braced!(content in input),
                fields: content.parse_terminated(BuilderInputField::parse, Token![,])?,
            })
        } else {
            Err(lookahead.error())
        }
    }
}

struct BuilderInputField {
    field: syn::Field,
    optional: Option<syn::Type>,
    each: Option<BuilderEachAttribute>,
}

struct BuilderEachAttribute {
    val: String,
    ty: syn::Type,
}

impl syn::parse::Parse for BuilderInputField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs: Vec<syn::Attribute> = input.call(Attribute::parse_outer)?;
        let builder_attrs: Vec<&Attribute> = attrs
            .iter()
            .filter(|attr| attr.path().is_ident("builder"))
            .collect();
        // TODO: Get Vec type if each is set as it's needed for the set method
        let each_attr: Option<syn::LitStr> = if let Some(attr) = builder_attrs.first() {
            let mut each: Option<syn::LitStr> = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("each") {
                    each = Some(meta.value()?.parse()?);
                    Ok(())
                } else {
                    Err(syn::Error::new(
                        attr.meta.span(),
                        "expected `builder(each = \"...\")`",
                    ))
                }
            })?;
            each
        } else {
            None
        };

        let field = Field::parse_named(input)?;
        let optional = type_confirm(vec!["Option:", "std:core:Option"].into_iter(), &field);

        let mut each: Option<BuilderEachAttribute> = None;

        if let Some(each_str) = each_attr {
            let Some(each_ty) = type_confirm(vec!["Vec:", "std:vec:Vec"].into_iter(), &field)
            else {
                return Err(syn::Error::new(field.span(), "Type should be a Vec"));
            };

            each = Some(BuilderEachAttribute {
                val: each_str.value(),
                ty: each_ty,
            });
        }
        Ok(Self {
            field,
            optional,
            each,
        })
    }
}

impl BuilderInput {
    pub fn builder_struct_ident(&self) -> syn::Ident {
        syn::Ident::new(
            &format!("{}Builder", self.ident),
            proc_macro2::Span::call_site(),
        )
    }

    pub fn generate_builder_struct(&self) -> proc_macro2::TokenStream {
        // The declaration of the struct, ie. it's visibility, type and identifier.
        // Made a variable as it's used in the different struct types

        let vis = &self.vis;
        let builder_name = self.builder_struct_ident();
        let builder_decl = quote! {#vis struct #builder_name};

        let recurse = self.fields.pairs().map(|pair| {
            let f = pair.value();
            let name = &f.field.ident;
            let ty = &f.field.ty;
            if f.optional.is_some() || f.each.is_some() {
                quote! {
                    #name: #ty
                }
            } else {
                quote! {
                    #name: std::option::Option<#ty>
                }
            }
        });

        quote! {
            #builder_decl {
                #(#recurse),*
            }
        }
    }
    pub fn generate_builder_method(&self) -> proc_macro2::TokenStream {
        let vis = &self.vis;
        let builder_name = self.builder_struct_ident();
        let recurse = self.fields.pairs().map(|pair| {
            let f = pair.value();
            let name = &f.field.ident;
            if f.each.is_some() {
                quote_spanned! { f.field.span() =>
                    #name: std::vec::Vec::new()
                }
            } else {
                quote! {
                    #name: None
                }
            }
        });
        quote! {
            #vis fn builder() -> #builder_name {
                #builder_name {#(#recurse),*}
            }
        }
    }
    pub fn generate_setter_methods(&self) -> proc_macro2::TokenStream {
        let recurse = self.fields.pairs().map(|pair| {
            let f = pair.value();
            if let Some(each) = &f.each {
                let field_name = &f.field.ident;
                let name = syn::Ident::new(&each.val, f.field.span());
                let ty = &each.ty;
                quote_spanned! { f.field.span() =>
                    fn #name(&mut self, #name: #ty) -> &mut Self {
                        self.#field_name.push(#name);
                        self
                    }
                }
            } else {
                let name = &f.field.ident;
                let ty = if f.optional.is_some() {
                    f.optional.as_ref().unwrap()
                } else {
                    &f.field.ty
                };
                quote! {
                    fn #name(&mut self, #name: #ty) -> &mut Self {
                        self.#name = Some(#name);
                        self
                    }
                }
            }
        });
        quote! {
            #(#recurse)*
        }
    }

    pub fn generate_final_build_method(&self) -> proc_macro2::TokenStream {
        let field_check = {
            let recurse = self.fields.pairs().map(|pair| {
                let f = pair.value();
                let field = &f.field.ident;
                if f.optional.is_some() || f.each.is_some() {
                    quote! {
                        let #field = Clone::clone(&self.#field);
                    }
                } else {
                    quote! {
                        let Some(#field) = Clone::clone(&self.#field) else {
                            return Err(String::from("missing #field_name value").into());
                        };
                    }
                }
            });
            quote! {
                #(#recurse)*
            }
        };
        let field_set = {
            let recurse = self.fields.pairs().map(|pair| {
                let f = pair.value();
                let field = &f.field.ident;
                quote_spanned! {f.field.span() =>
                    #field: Clone::clone(&#field)
                }
            });
            quote! {
                #(#recurse),*
            }
        };
        let struct_name = &self.ident;
        quote! {
            pub fn build(&mut self) -> core::result::Result<#struct_name, std::boxed::Box<dyn std::error::Error>> {
                #field_check

                Ok(#struct_name {
                    #field_set
                })
            }
        }
    }
}
// Checks if a field is a container against an iter of possible path types with only one colon separator,
// like "Option:", "std:option:Option" or "core:option:Option".
// Then will return a Some(syn::Type), syn::Type=the contained type
fn type_confirm(
    mut pattern: impl Iterator<Item = &'static str>,
    field: &Field,
) -> Option<syn::Type> {
    let ty = &field.ty;
    let opt = match ty {
        syn::Type::Path(typepath) if typepath.qself.is_none() => Some(typepath.path.clone()),
        _ => None,
    };

    if let Some(path) = opt {
        let idents_of_path = path.segments.iter().fold(String::new(), |mut acc, v| {
            acc.push_str(&v.ident.to_string());
            acc.push(':');
            acc
        });
        if let Some(segment) = pattern
            .find(|s| idents_of_path == *s)
            .and_then(|_| path.segments.last())
        {
            if let syn::PathArguments::AngleBracketed(ref args) = &segment.arguments {
                if let Some(syn::GenericArgument::Type(ty)) = args.args.first() {
                    Some(ty.clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}
