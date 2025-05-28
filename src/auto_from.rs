use darling::{util::SpannedValue, FromAttributes, FromDeriveInput, ToTokens};
use itertools::Itertools;
use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::{quote, quote_spanned};
use std::collections::HashMap;

use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    token, Attribute, Data, DataStruct, DeriveInput, Expr, Field, Fields, FieldsNamed,
    GenericArgument, Path, Token, Type, TypePath,
};

#[derive(Debug, Default, FromDeriveInput)]
#[darling(default, attributes(convert))]
struct MetaOpts {
    default: bool,
    #[darling(multiple)]
    into: Vec<Path>,
    #[darling(multiple)]
    from: Vec<Path>,
    #[darling(multiple)]
    from_on: Vec<Path>,
}

#[derive(Debug, Default, Clone, FromAttributes)]
#[darling(default, attributes(convert_field))]
struct FiledOpts {
    rename: String,
    custom_fn: SpannedValue<String>,
    from: String,
    into: String,
    ignore: bool,
    wrap: bool,
    unwrap: bool,
    option: bool,
    to_string: bool,
    nested_field: String,
    nested_type: String,
}

#[derive(Clone, Debug)]
struct Fd {
    name: Ident,
    default_opts: FiledOpts,
    custom_opts: Vec<FiledOpts>,
    optional: bool,
    is_vec: bool,
}
/// 把一个 Field 转换成 Fd
impl From<Field> for Fd {
    fn from(f: Field) -> Self {
        let (optional, is_vec, _) = get_option_inner(&f.ty);
        let multi_opts = parse_attrs(&f.attrs);
        let opts = multi_opts
            .iter()
            .find(|f| f.from.is_empty() && f.into.is_empty())
            .map(Clone::clone)
            .unwrap_or_default();
        Self {
            // 此时，我们拿到的是 NamedFields，所以 ident 必然存在
            name: f.ident.unwrap(),
            optional,
            is_vec,
            default_opts: opts,
            custom_opts: multi_opts,
        }
    }
}

#[derive(Debug, Clone)]
enum FieldClass {
    From(String),
    Into(String),
}

impl Fd {
    fn get_by_name(&self, field_class: FieldClass) -> Option<FiledOpts> {
        match field_class.clone() {
            FieldClass::From(name) => {
                if let Some(opt) = self.custom_opts.iter().find(|o| {
                    o.from
                        .split_whitespace()
                        .collect::<String>()
                        .eq(&name.split_whitespace().collect::<String>())
                }) {
                    return Some(opt.clone());
                }
            }
            FieldClass::Into(name) => {
                if let Some(opt) = self.custom_opts.iter().find(|o| {
                    o.into
                        .split_whitespace()
                        .collect::<String>()
                        .eq(&name.split_whitespace().collect::<String>())
                }) {
                    return Some(opt.clone());
                }
            }
        };

        None
    }
}

fn parse_attrs(attrs: &[Attribute]) -> Vec<FiledOpts> {
    let mut result = vec![];
    for attr in attrs
        .iter()
        .filter(|attr| attr.path.is_ident("convert_field"))
    {
        match FiledOpts::from_attributes(&[attr.clone()]) {
            Ok(f) => {
                result.push(f);
            }
            Err(e) => {
                panic!("{:?}", e)
            }
        }
    }
    result
}

#[derive(Debug)]
pub struct DeriveIntoContext {
    name: Ident,
    attrs: MetaOpts,
    fields: Vec<Fd>,
}

impl DeriveIntoContext {
    pub fn render(&self) -> TokenStream {
        let name = &self.name;
        let is_from = !self.attrs.from.is_empty();
        let is_into = !self.attrs.into.is_empty();
        let is_from_on = !self.attrs.from_on.is_empty();

        let from_code = if is_from {
            TokenStream::from_iter(self.attrs.from.iter().map(|from| {
                let struct_name = Ident::new(&format!("{}", name), name.span());
                let source_name = from;
                let assigns = self.gen_from_assigns(from.to_token_stream().to_string());

                let default_code = if self.attrs.default {
                    quote! {..#struct_name::default()}
                } else {
                    quote!()
                };
                quote! {
                        impl std::convert::From<#source_name> for #struct_name {
                            fn from(this: #source_name) -> Self {
                                #struct_name {
                                #(#assigns)*

                                #default_code
                            }
                        }
                    }
                }
            }))
        } else {
            quote!()
        };
        let into_code = if is_into {
            TokenStream::from_iter(self.attrs.into.iter().map(|into| {
                let struct_name = Ident::new(&format!("{}", name), name.span());
                let target_name = into;
                let assigns = self.gen_into_assigns(into.to_token_stream().to_string());

                let default_code = if self.attrs.default {
                    quote! {..#target_name::default()}
                } else {
                    quote!()
                };

                quote! {
                    impl std::convert::Into<#target_name> for #struct_name {
                        fn into(self) -> #target_name {
                            let this = self;
                            #target_name {
                                #(#assigns)*

                                #default_code
                            }
                        }
                    }
                }
            }))
        } else {
            quote!()
        };
        let from_on_code = if is_from_on {
            TokenStream::from_iter(self.attrs.from_on.iter().map(|remote| {
                let struct_name = Ident::new(&format!("{}", name), name.span());
                let target_name = remote;
                let assigns = self.gen_into_assigns(remote.to_token_stream().to_string());

                let default_code = if self.attrs.default {
                    quote! {..#target_name::default()}
                } else {
                    quote!()
                };

                quote! {
                    impl std::convert::From<#struct_name> for #target_name {
                        fn from(value: #struct_name) -> #target_name {
                            let this = value;
                            #target_name {
                                #(#assigns)*

                                #default_code
                            }
                        }
                    }
                }
            }))
        } else {
            quote!()
        };
        quote!(
            #from_code
            #into_code
            #from_on_code
        )
    }

    fn gen_from_assigns(&self, struct_name: String) -> Vec<TokenStream> {
        self.fields
            .clone()
            .into_iter()
            .sorted_by_key(|fd| {
                fd.get_by_name(FieldClass::From(struct_name.clone()))
                    .unwrap_or(fd.default_opts.clone())
                    .custom_fn
                    .is_empty()
            })
            .map(|fd| {
                let Fd {
                    name,
                    optional,
                    is_vec,
                    ..
                } = fd.clone();

                let opts = fd
                    .get_by_name(FieldClass::From(struct_name.clone()))
                    .unwrap_or(fd.default_opts);

                let source_name: Ident = if opts.rename.is_empty() {
                    name.clone()
                } else {
                    Ident::new(opts.rename.as_str(), name.span())
                };

                if !opts.custom_fn.is_empty() {
                    let custom_fn = parse_custom_fn_to_token_stream(
                        name.clone(),
                        opts.custom_fn.as_str(),
                        opts.custom_fn.span(),
                    );
                    return quote! {
                        #name: #custom_fn,
                    };
                }

                if self.attrs.default && opts.ignore {
                    return quote!();
                }

                if optional && opts.ignore {
                    return quote! {
                        #name: None,
                    };
                }

                if opts.unwrap {
                    return quote! {
                        #name: this.#source_name.unwrap_or_default(),
                    };
                }

                if optional && opts.wrap {
                    return quote! {
                        #name: Some(this.#source_name),
                    };
                }

                if optional {
                    return quote! {
                        #name: this.#source_name.map(Into::into),
                    };
                }

                if opts.to_string {
                    return quote! {
                        #name: this.#source_name.to_string(),
                    };
                }

                if is_vec {
                    return quote! {
                        #name: this.#source_name.into_iter().map(|a| a.into()).collect(),
                    };
                }

                quote! {
                    #name: this.#source_name.into(),
                }
            })
            .collect()
    }

    // 比如：#field_name: self.#field_name.take().ok_or(" xxx need to be set!")
    fn gen_into_assigns(&self, struct_name: String) -> Vec<TokenStream> {
        let mut regular_fields = Vec::new();
        let mut nested_fields: HashMap<String, (String, Vec<(Ident, TokenStream)>)> =
            HashMap::new();

        // First pass: collect all fields and separate regular from nested
        for fd in &self.fields {
            let Fd {
                name,
                optional,
                is_vec,
                default_opts: _opts,
                ..
            } = fd.clone();

            let opts = fd
                .get_by_name(FieldClass::Into(struct_name.clone()))
                .unwrap_or(fd.default_opts.clone());

            if opts.ignore {
                continue;
            }

            let target_name: Ident = if opts.rename.is_empty() {
                name.clone()
            } else {
                Ident::new(opts.rename.as_str(), name.span())
            };

            let field_assignment = if !opts.custom_fn.is_empty() {
                let custom_fn = parse_custom_fn_to_token_stream(
                    name.clone(),
                    opts.custom_fn.as_str(),
                    opts.custom_fn.span(),
                );
                custom_fn
            } else if optional && opts.unwrap {
                quote! { this.#name.unwrap_or_default() }
            } else if opts.option {
                if optional {
                    quote! { this.#name }
                } else {
                    quote! { Some(this.#name) }
                }
            } else if optional {
                quote! { this.#name.map(Into::into) }
            } else if opts.to_string {
                quote! { this.#name.to_string() }
            } else if is_vec {
                quote! { this.#name.into_iter().map(|a| a.into()).collect() }
            } else {
                quote! { this.#name.into() }
            };

            if !opts.nested_field.is_empty() {
                // Parse nested_field to extract field name and optional type
                let (field_name, type_name) = if opts.nested_field.contains(':') {
                    let parts: Vec<&str> = opts.nested_field.splitn(2, ':').collect();
                    (parts[0].to_string(), parts[1].to_string())
                } else if !opts.nested_type.is_empty() {
                    (opts.nested_field.clone(), opts.nested_type.clone())
                } else {
                    // Fallback to simple capitalization heuristic
                    let mut chars: Vec<char> = opts.nested_field.chars().collect();
                    if !chars.is_empty() {
                        chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
                    }
                    let inferred_type = chars.into_iter().collect::<String>();
                    (opts.nested_field.clone(), inferred_type)
                };

                // This field should be nested
                nested_fields
                    .entry(field_name.clone())
                    .or_insert_with(|| (type_name, Vec::new()))
                    .1
                    .push((target_name, field_assignment));
            } else {
                // Regular field
                regular_fields.push(quote! {
                    #target_name: #field_assignment,
                });
            }
        }

        // Second pass: generate nested struct initializations
        // Sort the nested fields by name to ensure deterministic output
        let mut sorted_nested_fields: Vec<_> = nested_fields.into_iter().collect();
        sorted_nested_fields.sort_by(|a, b| a.0.cmp(&b.0));

        for (nested_struct_name, (type_name, fields)) in sorted_nested_fields {
            let nested_ident = Ident::new(&nested_struct_name, Span::call_site());
            let struct_type_ident = Ident::new(&type_name, Span::call_site());

            // Create field assignments for struct literal
            let field_assignments: Vec<TokenStream> = fields
                .into_iter()
                .map(|(field_name, assignment)| {
                    quote! {
                        #field_name: #assignment
                    }
                })
                .collect();

            // Use struct literal syntax with the specified or inferred type
            regular_fields.push(quote! {
                #nested_ident: #struct_type_ident {
                    #(#field_assignments,)*
                    ..Default::default()
                },
            });
        }

        regular_fields
    }
}

impl From<DeriveInput> for DeriveIntoContext {
    fn from(input: DeriveInput) -> Self {
        let attrs = match MetaOpts::from_derive_input(&input) {
            Ok(v) => v,
            Err(_e) => {
                panic!("not args");
            }
        };
        let name = input.ident;

        let fields = if let Data::Struct(DataStruct {
            fields: Fields::Named(FieldsNamed { named, .. }),
            ..
        }) = input.data
        {
            named
        } else {
            panic!("Unsupported data type");
        };

        let fds = fields.into_iter().map(Fd::from).collect();
        Self {
            name,
            fields: fds,
            attrs,
        }
    }
}

// 如果是 T = Option<Inner>，返回 (true, Inner)；否则返回 (false, T)
fn get_option_inner(ty: &Type) -> (bool, bool, &Type) {
    // 首先模式匹配出 segments
    if let Type::Path(TypePath {
        path: Path { segments, .. },
        ..
    }) = ty
    {
        if let Some(v) = segments.iter().next() {
            if v.ident == "Option" {
                // 如果 PathSegment 第一个是 Option，那么它内部应该是 AngleBracketed，比如 <T>
                // 获取其第一个值，如果是 GenericArgument::Type，则返回
                let t = match &v.arguments {
                    syn::PathArguments::AngleBracketed(a) => match a.args.iter().next() {
                        Some(GenericArgument::Type(t)) => t,
                        _ => panic!("Not sure what to do with other GenericArgument"),
                    },
                    _ => panic!("Not sure what to do with other PathArguments"),
                };
                return (true, false, t);
            }
            if v.ident == "Vec" {
                // 如果 PathSegment 第一个是 Option，那么它内部应该是 AngleBracketed，比如 <T>
                // 获取其第一个值，如果是 GenericArgument::Type，则返回
                let t = match &v.arguments {
                    syn::PathArguments::AngleBracketed(a) => match a.args.iter().next() {
                        Some(GenericArgument::Type(t)) => t,
                        _ => panic!("Not sure what to do with other GenericArgument"),
                    },
                    _ => panic!("Not sure what to do with other PathArguments"),
                };
                return (false, true, t);
            }
        }
    }
    (false, false, ty)
}

fn parse_custom_fn_to_token_stream(field_name: Ident, custom_fn: &str, span: Span) -> TokenStream {
    let ident = syn::parse_str::<Ident>(custom_fn);
    if let Ok(_fn_) = ident {
        return quote_spanned! { span => #_fn_(&this) };
    }

    let expr = syn::parse_str::<CustomFnExpr>(&format!("{};{}", field_name, custom_fn));
    match expr {
        Ok(CustomFnExpr(_expr_)) => {
            quote_spanned! { span => #_expr_ }
        }
        Err(e) => {
            let e = e.to_string();
            quote_spanned! { span => compile_error!(#e) }
        }
    }
}

struct CustomFnExpr(Expr);

impl Parse for CustomFnExpr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let field_name = input.parse()?;
        input.parse::<Token![;]>()?;
        let tt = parse_custom_fn_expr(field_name, input)?;
        let expr = syn::parse2(tt)?;
        Ok(Self(expr))
    }
}

fn parse_custom_fn_expr(field_name: Ident, input: ParseStream) -> syn::Result<TokenStream> {
    let mut begin_expr = true;
    let mut tokens = Vec::new();
    while !input.is_empty() {
        if begin_expr && input.peek(Token![.]) && input.peek2(syn::Ident) {
            input.parse::<Token![.]>()?;
            tokens.push(TokenTree::Ident(Ident::new("this", field_name.span())));
            tokens.push(TokenTree::Punct(Punct::new('.', Spacing::Alone)));
            tokens.push(TokenTree::Ident(field_name.clone()));
            tokens.push(TokenTree::Punct(Punct::new('.', Spacing::Alone)));
            begin_expr = false;
            continue;
        }

        begin_expr = input.peek(Token![break])
            || input.peek(Token![continue])
            || input.peek(Token![if])
            || input.peek(Token![in])
            || input.peek(Token![match])
            || input.peek(Token![mut])
            || input.peek(Token![return])
            || input.peek(Token![while])
            || input.peek(Token![+])
            || input.peek(Token![&])
            || input.peek(Token![!])
            || input.peek(Token![^])
            || input.peek(Token![,])
            || input.peek(Token![/])
            || input.peek(Token![=])
            || input.peek(Token![>])
            || input.peek(Token![<])
            || input.peek(Token![|])
            || input.peek(Token![%])
            || input.peek(Token![;])
            || input.peek(Token![*])
            || input.peek(Token![-]);

        let token: TokenTree = if input.peek(token::Paren) {
            let content;
            let delimiter = parenthesized!(content in input);
            let nested = parse_custom_fn_expr(field_name.clone(), &content)?;
            let mut group = Group::new(Delimiter::Parenthesis, nested);
            group.set_span(delimiter.span);
            TokenTree::Group(group)
        } else if input.peek(token::Brace) {
            let content;
            let delimiter = braced!(content in input);
            let nested = parse_custom_fn_expr(field_name.clone(), &content)?;
            let mut group = Group::new(Delimiter::Brace, nested);
            group.set_span(delimiter.span);
            TokenTree::Group(group)
        } else if input.peek(token::Bracket) {
            let content;
            let delimiter = bracketed!(content in input);
            let nested = parse_custom_fn_expr(field_name.clone(), &content)?;
            let mut group = Group::new(Delimiter::Bracket, nested);
            group.set_span(delimiter.span);
            TokenTree::Group(group)
        } else {
            input.parse()?
        };
        tokens.push(token);
    }
    Ok(TokenStream::from_iter(tokens))
}
