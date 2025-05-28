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

#[derive(Debug, Clone)]
struct NestedFieldPath {
    field_name: String,
    type_name: String,
}

#[derive(Debug)]
struct NestedStructure {
    fields: HashMap<String, NestedNode>,
}

#[derive(Debug)]
struct NestedNode {
    type_name: String,
    direct_fields: Vec<(Ident, TokenStream)>,
    nested_children: HashMap<String, NestedNode>,
}

impl NestedStructure {
    fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    fn add_field(&mut self, path: Vec<NestedFieldPath>, target_name: Ident, assignment: TokenStream) {
        if path.is_empty() {
            return;
        }

        let first = &path[0];
        let node = self.fields
            .entry(first.field_name.clone())
            .or_insert_with(|| NestedNode {
                type_name: first.type_name.clone(),
                direct_fields: Vec::new(),
                nested_children: HashMap::new(),
            });

        if path.len() == 1 {
            // This is a direct field of the current level
            node.direct_fields.push((target_name, assignment));
        } else {
            // This needs to go deeper
            node.add_nested_field(&path[1..], target_name, assignment);
        }
    }

    fn generate_assignments(&self) -> Vec<TokenStream> {
        let mut assignments = Vec::new();
        
        // Sort by field name for deterministic output
        let mut sorted_fields: Vec<_> = self.fields.iter().collect();
        sorted_fields.sort_by(|a, b| a.0.cmp(b.0));

        for (field_name, node) in sorted_fields {
            let field_ident = Ident::new(field_name, Span::call_site());
            let type_ident = Ident::new(&node.type_name, Span::call_site());
            
            let field_assignments = node.generate_field_assignments();
            
            assignments.push(quote! {
                #field_ident: #type_ident {
                    #(#field_assignments,)*
                    ..Default::default()
                },
            });
        }
        
        assignments
    }
}

impl NestedNode {
    fn add_nested_field(&mut self, path: &[NestedFieldPath], target_name: Ident, assignment: TokenStream) {
        if path.is_empty() {
            return;
        }

        let first = &path[0];
        let child = self.nested_children
            .entry(first.field_name.clone())
            .or_insert_with(|| NestedNode {
                type_name: first.type_name.clone(),
                direct_fields: Vec::new(),
                nested_children: HashMap::new(),
            });

        if path.len() == 1 {
            child.direct_fields.push((target_name, assignment));
        } else {
            child.add_nested_field(&path[1..], target_name, assignment);
        }
    }

    fn generate_field_assignments(&self) -> Vec<TokenStream> {
        let mut assignments = Vec::new();
        
        // Add direct field assignments
        for (field_name, assignment) in &self.direct_fields {
            assignments.push(quote! {
                #field_name: #assignment
            });
        }
        
        // Add nested struct assignments
        let mut sorted_children: Vec<_> = self.nested_children.iter().collect();
        sorted_children.sort_by(|a, b| a.0.cmp(b.0));
        
        for (child_field_name, child_node) in sorted_children {
            let child_field_ident = Ident::new(child_field_name, Span::call_site());
            let child_type_ident = Ident::new(&child_node.type_name, Span::call_site());
            
            let child_assignments = child_node.generate_field_assignments();
            
            assignments.push(quote! {
                #child_field_ident: #child_type_ident {
                    #(#child_assignments,)*
                    ..Default::default()
                }
            });
        }
        
        assignments
    }
}

fn parse_nested_field_path(nested_field: &str, nested_type: &str) -> Vec<NestedFieldPath> {
    // Handle dot-separated paths like "level1.level2.level3"
    if nested_field.contains('.') {
        let parts: Vec<&str> = nested_field.split('.').collect();
        let mut path = Vec::new();
        
        for (i, part) in parts.iter().enumerate() {
            let (field_name, type_name) = if part.contains(':') {
                let type_parts: Vec<&str> = part.splitn(2, ':').collect();
                (type_parts[0].to_string(), type_parts[1].to_string())
            } else if i == 0 && !nested_type.is_empty() {
                // Use nested_type for the first level if provided
                (part.to_string(), nested_type.to_string())
            } else {
                // Fallback to capitalization heuristic
                let mut chars: Vec<char> = part.chars().collect();
                if !chars.is_empty() {
                    chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
                }
                let inferred_type = chars.into_iter().collect::<String>();
                (part.to_string(), inferred_type)
            };
            
            path.push(NestedFieldPath { field_name, type_name });
        }
        
        path
    } else {
        // Handle single level (existing behavior)
        let (field_name, type_name) = if nested_field.contains(':') {
            let parts: Vec<&str> = nested_field.splitn(2, ':').collect();
            (parts[0].to_string(), parts[1].to_string())
        } else if !nested_type.is_empty() {
            (nested_field.to_string(), nested_type.to_string())
        } else {
            // Fallback to simple capitalization heuristic
            let mut chars: Vec<char> = nested_field.chars().collect();
            if !chars.is_empty() {
                chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
            }
            let inferred_type = chars.into_iter().collect::<String>();
            (nested_field.to_string(), inferred_type)
        };
        
        vec![NestedFieldPath { field_name, type_name }]
    }
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
        let mut nested_structure = NestedStructure::new();

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
                // Parse nested_field path and validate depth
                let nested_path = parse_nested_field_path(&opts.nested_field, &opts.nested_type);
                if nested_path.len() > 3 {
                    panic!("Nested field depth cannot exceed 3 levels. Found {} levels in '{}'", 
                           nested_path.len(), opts.nested_field);
                }
                
                nested_structure.add_field(nested_path, target_name, field_assignment);
            } else {
                // Regular field
                regular_fields.push(quote! {
                    #target_name: #field_assignment,
                });
            }
        }

        // Second pass: generate nested struct initializations
        regular_fields.extend(nested_structure.generate_assignments());
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
