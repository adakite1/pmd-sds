// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 adakite1
//
// Licensed under either of Apache License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0)
// or the MIT license (http://opensource.org) at your option.

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote, quote_spanned};
use syn::{
    Attribute, Error, Field, Ident, LitInt, LitStr, Meta, Result, Token, Visibility, braced,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
};

#[proc_macro]
pub fn many_spans(input: TokenStream) -> TokenStream {
    let def = parse_macro_input!(input as SpanStructDef);
    match expand(def) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

enum Entry {
    Start {
        attrs: Vec<Attribute>,
        instantiable: bool,
        label: Ident,
    },
    Stop {
        label: Ident,
    },
    Field(Field),
}

struct SpanStructDef {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
    entries: Vec<Entry>,
}

impl Parse for SpanStructDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        input.parse::<Token![struct]>()?;
        let ident: Ident = input.parse()?;
        if input.peek(Token![<]) || input.peek(Token![where]) {
            return Err(input.error("structs with generics are not supported"));
        }

        let content;
        braced!(content in input);
        let mut entries = Vec::new();
        while !content.is_empty() {
            let attrs = content.call(Attribute::parse_outer)?;

            if content.peek(Token![+]) {
                content.parse::<Token![+]>()?;
                let label: Ident = content.parse()?;
                let star: Option<Token![*]> = content.parse()?;
                content.parse::<Option<Token![,]>>()?;
                entries.push(Entry::Start {
                    attrs,
                    instantiable: star.is_some(),
                    label,
                });
                continue;
            }

            if peek_stop(&content) {
                let label: Ident = content.parse()?;
                if let Some(a) = attrs.first() {
                    return Err(Error::new(
                        a.span(),
                        format!("attributes are not allowed on span stops (`{label}`)"),
                    ));
                }
                if let Some(star) = content.parse::<Option<Token![*]>>()? {
                    return Err(Error::new(
                        star.span(),
                        format!("`*` is only allowed on span starts (`+{label}*`)"),
                    ));
                }
                content.parse::<Option<Token![,]>>()?;
                entries.push(Entry::Stop { label });
                continue;
            }

            let mut field = content.call(Field::parse_named)?;
            field.attrs = attrs;
            entries.push(Entry::Field(field));
            if content.is_empty() {
                break;
            }
            content.parse::<Token![,]>()?;
        }

        input.parse::<Option<Token![;]>>()?;
        Ok(Self {
            attrs,
            vis,
            ident,
            entries,
        })
    }
}

fn peek_stop(input: ParseStream) -> bool {
    let fork = input.fork();
    fork.parse::<Ident>().is_ok() && !fork.peek(Token![:])
}

struct SpanDef {
    ident: Ident,
    docs: Vec<Attribute>,
    inst_attrs: Vec<Attribute>,
    instantiable: bool,
    first: usize,
    end: Option<usize>,
}

#[derive(Default)]
struct Errors(Option<Error>);

impl Errors {
    fn push(&mut self, span: impl Spanned, msg: impl std::fmt::Display) {
        let e = Error::new(span.span(), msg);
        match &mut self.0 {
            Some(acc) => acc.combine(e),
            None => self.0 = Some(e),
        }
    }
    fn finish(self) -> Result<()> {
        self.0.map_or(Ok(()), Err)
    }
}

struct OutAttr {
    conditions: Vec<Meta>,
    attr: Meta,
}

impl OutAttr {
    fn from_attrs(attrs: &[Attribute]) -> Vec<OutAttr> {
        attrs.iter().flat_map(|a| Self::unwrap(&a.meta)).collect()
    }

    fn unwrap(meta: &Meta) -> Vec<OutAttr> {
        let Some((condition, inner)) = cfg_attr_parts(meta) else {
            return vec![OutAttr {
                conditions: Vec::new(),
                attr: meta.clone(),
            }];
        };
        inner
            .iter()
            .flat_map(Self::unwrap)
            .map(|mut a| {
                a.conditions.insert(0, condition.clone());
                a
            })
            .collect()
    }

    fn tokens(&self) -> TokenStream2 {
        self.wrap(&self.attr)
    }

    fn wrap(&self, attr: impl ToTokens) -> TokenStream2 {
        match self.conditions.as_slice() {
            [] => quote!(#[#attr]),
            [condition] => quote!(#[cfg_attr(#condition, #attr)]),
            conditions => quote!(#[cfg_attr(all(#(#conditions),*), #attr)]),
        }
    }
}

fn cfg_attr_parts(meta: &Meta) -> Option<(Meta, Vec<Meta>)> {
    let Meta::List(list) = meta else {
        return None;
    };
    if !list.path.is_ident("cfg_attr") {
        return None;
    }
    let mut args = list
        .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        .ok()?
        .into_iter();
    let condition = args.next()?;
    Some((condition, args.collect()))
}

fn repr_items(meta: &Meta) -> Vec<Meta> {
    match meta {
        Meta::List(list) if list.path.is_ident("repr") => list
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .map(|items| items.into_iter().collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn has_repr_c(attrs: &[OutAttr]) -> bool {
    attrs
        .iter()
        .filter(|a| a.conditions.is_empty())
        .flat_map(|a| repr_items(&a.attr))
        .any(|m| m.path().is_ident("C"))
}

#[derive(Default)]
struct Passthrough {
    cfgs: Vec<TokenStream2>,
    shared: Vec<TokenStream2>,
    reprs: Vec<TokenStream2>,
}

impl Passthrough {
    fn new(attrs: &[OutAttr]) -> Self {
        let mut out = Self::default();
        for a in attrs {
            let name = a.attr.path().get_ident().map(|i| i.to_string());
            match name.as_deref() {
                Some("cfg") => out.cfgs.push(a.tokens()),
                Some("derive" | "allow" | "warn" | "deny" | "forbid") => {
                    out.shared.push(a.tokens())
                },
                Some("expect") => {
                    if let Meta::List(list) = &a.attr {
                        let lints = &list.tokens;
                        out.shared.push(a.wrap(quote!(allow(#lints))));
                    }
                },
                Some("repr") => {
                    let kept: Vec<Meta> = repr_items(&a.attr)
                        .into_iter()
                        .filter(|m| !m.path().is_ident("align"))
                        .collect();
                    if !kept.is_empty() {
                        out.reprs.push(a.wrap(quote!(repr(#(#kept),*))));
                    }
                },
                _ => {},
            }
        }
        out
    }
}

fn validate(
    attrs: &[OutAttr],
    ident: &Ident,
    entries: Vec<Entry>,
) -> Result<(Vec<Field>, Vec<SpanDef>)> {
    let mut errs = Errors::default();

    if !has_repr_c(attrs) {
        errs.push(ident, "struct must be `#[repr(C)]`");
    }

    let mut fields: Vec<Field> = Vec::new();
    let mut span_defs: Vec<SpanDef> = Vec::new();

    let mut rejected: Vec<Ident> = Vec::new();

    for entry in entries {
        match entry {
            Entry::Field(f) => {
                for a in &f.attrs {
                    if a.path().is_ident("cfg") || a.path().is_ident("cfg_attr") {
                        errs.push(a, "`cfg` on fields is not supported (the macro can't evaluate `cfg`s to determine which fields will be enabled)");
                    }
                }
                fields.push(f);
            },
            Entry::Start {
                attrs,
                instantiable,
                label,
            } => {
                let (docs, inst_attrs): (Vec<_>, Vec<_>) =
                    attrs.into_iter().partition(|a| a.path().is_ident("doc"));
                if !instantiable {
                    for a in &inst_attrs {
                        errs.push(
                            a,
                            format!("only doc comments are allowed on `+{label}`; attributes are only real when the span is instantiable (`+{label}*`)"),
                        );
                    }
                }
                match span_defs.iter().find(|s| s.ident == label) {
                    Some(prev) if prev.end.is_some() => {
                        errs.push(&label, format!("span `{label}` was already defined above"));
                        errs.push(&prev.ident, format!("span `{label}` first defined here"));
                        rejected.push(label);
                    },
                    Some(prev) => {
                        errs.push(&label, format!("span `{label}` is already started"));
                        errs.push(&prev.ident, format!("span `{label}` first started here"));
                    },
                    None => span_defs.push(SpanDef {
                        ident: label,
                        docs,
                        inst_attrs,
                        instantiable,
                        first: fields.len(),
                        end: None,
                    }),
                }
            },
            Entry::Stop { label } => {
                if let Some(i) = rejected.iter().position(|r| *r == label) {
                    rejected.remove(i);
                    continue;
                }
                match span_defs.iter_mut().find(|s| s.ident == label) {
                    None => errs.push(
                        &label,
                        format!("`{label}` stops an undefined span (start a span with `+{label}`)"),
                    ),
                    Some(s) if s.end.is_some() => {
                        errs.push(&label, format!("span `{label}` is already stopped"))
                    },
                    Some(s) => {
                        if s.first == fields.len() {
                            errs.push(&label, format!("span `{label}` contains no fields"));
                        }
                        s.end = Some(fields.len());
                    },
                }
            },
        }
    }
    for s in span_defs.iter().filter(|s| s.end.is_none()) {
        errs.push(&s.ident, format!("span `{}` was never stopped", s.ident));
    }
    errs.finish()?;
    Ok((fields, span_defs))
}

fn fnv1a_128(bytes: &[u8]) -> u128 {
    let mut hash: u128 = 0x6c62272e07bb014262b821756295c58d;
    for &b in bytes {
        hash ^= b as u128;
        hash = hash.wrapping_mul(0x0000000001000000000000000000013b);
    }
    hash
}

fn type_text(ty: &syn::Type) -> String {
    let mut text = quote!(#ty).to_string();
    for (from, to) in [
        (" :: ", "::"),
        (":: ", "::"),
        (" ;", ";"),
        (" ,", ","),
        ("< ", "<"),
        (" <", "<"),
        (" >", ">"),
        ("& ", "&"),
        ("[ ", "["),
        (" ]", "]"),
        ("( ", "("),
        (" )", ")"),
        ("* const ", "*const "),
        ("* mut ", "*mut "),
        ("fn (", "fn("),
    ] {
        text = text.replace(from, to);
    }
    text
}

fn krate() -> TokenStream2 {
    match crate_name("many-spans") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let name = Ident::new(&name, proc_macro2::Span::call_site());
            quote!(::#name)
        },
        Err(_) => quote!(::many_spans),
    }
}

struct OutDef<'a> {
    krate: TokenStream2,
    ident: &'a Ident,
    vis: &'a Visibility,
    fields: &'a [Field],
    field_idents: Vec<&'a Ident>,
    passthrough: Passthrough,
}

fn expand(def: SpanStructDef) -> Result<TokenStream2> {
    let SpanStructDef {
        attrs,
        vis,
        ident,
        entries,
    } = def;
    let out_attrs = OutAttr::from_attrs(&attrs);
    let (fields, span_defs) = validate(&out_attrs, &ident, entries)?;
    let krate = krate();

    let out_def = OutDef {
        krate: krate.clone(),
        ident: &ident,
        vis: &vis,
        fields: &fields,
        field_idents: fields.iter().map(|f| f.ident.as_ref().unwrap()).collect(),
        passthrough: Passthrough::new(&out_attrs),
    };
    let cfgs = &out_def.passthrough.cfgs;
    let spans = span_defs
        .iter()
        .enumerate()
        .map(|(id, s)| expand_span(&out_def, id, s));
    let label_idents = span_defs.iter().map(|s| &s.ident);
    let label_names = span_defs
        .iter()
        .map(|s| LitStr::new(&s.ident.unraw().to_string(), s.ident.span()));
    let span_types = expand_span_types(&out_def, &span_defs);

    Ok(quote! {
        #(#attrs)*
        #vis struct #ident { #(#fields),* }

        #(#cfgs)*
        unsafe impl #krate::SpanTable for #ident {
            const SPANS: &'static [#krate::Span] =
                &[#(<#ident as #krate::HasSpan<{ #ident::#label_idents }>>::SPAN),*];
            const NAMES: &'static [&'static str] = &[#(#label_names),*];
        }

        #(#spans)*

        #span_types
    })
}

fn expand_span_types(out_def: &OutDef, span_defs: &[SpanDef]) -> TokenStream2 {
    let OutDef {
        krate,
        ident,
        vis,
        passthrough: Passthrough { cfgs, .. },
        ..
    } = out_def;
    let struct_name = ident.unraw().to_string();
    let macro_name = format_ident!("__{}{}_span_types", struct_name.len(), struct_name);
    let reexport_vis = match vis {
        Visibility::Inherited => quote!(),
        _ => quote!(pub(crate)),
    };

    let arms = span_defs.iter().map(|s| {
        let span_name_ident = &s.ident;
        let instantiation = if s.instantiable {
            quote!(#krate::InstantiationOf<#ident, { #ident::#span_name_ident }>)
        } else {
            let span_name = span_name_ident.unraw().to_string();
            let msg = LitStr::new(
                &format!(
                    "span `{span_name}` can't be instantiated; make it instantiable with `+{span_name}*` or use `{struct_name}!({span_name} view)`"
                ),
                span_name_ident.span(),
            );
            quote_spanned!(span_name_ident.span()=> ::core::compile_error!(#msg))
        };
        quote! {
            (#span_name_ident) => { #instantiation };
            (#span_name_ident view) => { #krate::ViewOf<#ident, { #ident::#span_name_ident }> };
        }
    });

    quote! {
        #(#cfgs)*
        #[allow(unused_macros)]
        macro_rules! #macro_name {
            #(#arms)*
        }

        #(#cfgs)*
        #[allow(unused_imports)]
        #reexport_vis use #macro_name as #ident;
    }
}

fn expand_span(out_def: &OutDef, id: usize, s: &SpanDef) -> TokenStream2 {
    let OutDef {
        krate,
        ident,
        vis,
        passthrough:
            Passthrough {
                cfgs,
                shared,
                reprs,
            },
        ..
    } = out_def;
    let (first, end) = (s.first, s.end.unwrap());
    let span_name_ident = &s.ident;
    let span_name = span_name_ident.unraw().to_string();
    let struct_name = ident.unraw().to_string();
    let inst = format_ident!("__{}{}I_{}", struct_name.len(), struct_name, span_name);
    let view = format_ident!("__{}{}V_{}", struct_name.len(), struct_name, span_name);
    let span_fields = &out_def.fields[first..end];
    let span_field_idents = &out_def.field_idents[first..end];
    let first_field = span_field_idents[0];
    let (docs, inst_attrs) = (&s.docs, &s.inst_attrs);
    let id_hash = fnv1a_128(format!("{struct_name}::{span_name}").as_bytes());
    let span_id = LitInt::new(&format!("{id_hash:#x}"), span_name_ident.span());

    let label = span_name_ident.to_string();
    let mut sketch =
        format!("/* pseudocode */\nstruct {struct_name}!({label} [#{id_hash:#x}]) {{\n");
    if first > 0 {
        sketch += "    ...\n";
    }
    for (i, f) in span_fields.iter().enumerate() {
        let name = f.ident.as_ref().unwrap();
        sketch += &format!("    {name}: {}, //+{}\n", type_text(&f.ty), first + i);
    }
    if end < out_def.fields.len() {
        sketch += "    ...\n";
    }
    sketch += &format!(
        "}}\n\n/* span::<{ident}, {{ {ident}::{label} }}>() */\nSpan {{\n    id: {id},\n    first: {first},\n    end: {end},\n    offset: offset_of!({ident}, {first_field}),\n    size: size_of::<{struct_name}!({label} view)>(),\n}}"
    );
    let const_doc =
        format!("Span `{span_name}` of [`{ident}`]'s ID.\n\n```rust,ignore\n{sketch}\n```");
    let docs_separator = (!docs.is_empty()).then(|| quote!(#[doc = ""]));
    let inst_doc = format!(
        "Instantiation of span `{span_name}` of [`{ident}`] (`InstantiationOf<{ident}, {{ {ident}::{span_name} }}>`)."
    );
    let view_doc = format!(
        "View of span `{span_name}` of [`{ident}`] (`ViewOf<{ident}, {{ {ident}::{span_name} }}>`)."
    );

    let first_view_field = &span_fields[0];
    let mut view_fields = vec![quote!(#first_view_field)];
    for (i, pair) in span_fields.windows(2).enumerate() {
        let (prev, cur) = (&pair[0], &pair[1]);
        let (prev_ident, prev_ty, cur_ident) = (&prev.ident, &prev.ty, &cur.ident);
        let pad = format_ident!("__pad{}", i + 1);
        view_fields.push(quote! {
            #pad: [::core::mem::MaybeUninit<u8>; {
                ::core::mem::offset_of!(#ident, #cur_ident)
                    - (::core::mem::offset_of!(#ident, #prev_ident) + ::core::mem::size_of::<#prev_ty>())
            }]
        });
        view_fields.push(quote!(#cur));
    }

    let mut asserts = Vec::new();
    for f in span_field_idents {
        let msg_view = LitStr::new(
            &format!(
                "field `{f}` has a different relative offset in the view of span `{span_name}` of `{ident}` than in the full struct"
            ),
            f.span(),
        );
        asserts.push(quote! {
            ::core::assert!(
                ::core::mem::offset_of!(#view, #f)
                    == ::core::mem::offset_of!(#ident, #f) - ::core::mem::offset_of!(#ident, #first_field),
                #msg_view
            );
        });
        if !s.instantiable {
            continue;
        }
        let msg_inst = LitStr::new(
            &format!(
                "field `{f}` is at a different offset in the instantiation of span `{span_name}` of `{ident}` than in its view"
            ),
            f.span(),
        );
        asserts.push(quote! {
            ::core::assert!(
                ::core::mem::offset_of!(#inst, #f) == ::core::mem::offset_of!(#view, #f),
                #msg_inst
            );
        });
    }

    let instantiation = s.instantiable.then(|| {
        let offsets = out_def.field_idents.iter().enumerate().map(|(j, f)| {
            if (first..end).contains(&j) {
                quote!(::core::option::Option::Some(
                    ::core::mem::offset_of!(#inst, #f)
                ))
            } else {
                quote!(::core::option::Option::None)
            }
        });
        quote! {
            #[doc = #inst_doc]
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            #(#cfgs)*
            #(#shared)*
            #(#reprs)*
            #(#inst_attrs)*
            #vis struct #inst { #(#span_fields),* }

            #(#cfgs)*
            unsafe impl #krate::SpanInstantiation for #inst {
                type Full = #ident;
                const SPAN_ID: u128 = #ident::#span_name_ident;
                const SPAN: #krate::Span = <#ident as #krate::HasSpan<{ #ident::#span_name_ident }>>::SPAN;
                const OFFSETS: &'static [::core::option::Option<usize>] = &[#(#offsets),*];
            }

            #(#cfgs)*
            unsafe impl #krate::HasInstantiation<{ #ident::#span_name_ident }> for #ident {
                type Instantiation = #inst;
            }
        }
    });

    quote! {
        #(#cfgs)*
        #[allow(non_upper_case_globals)]
        impl #ident {
            #(#docs)*
            #docs_separator
            #[doc = #const_doc]
            #vis const #span_name_ident: u128 = #span_id;
        }

        #[doc = #view_doc]
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #(#cfgs)*
        #(#shared)*
        #[repr(C, packed)]
        #vis struct #view { #(#view_fields),* }

        #(#cfgs)*
        unsafe impl #krate::HasSpan<{ #ident::#span_name_ident }> for #ident {
            const SPAN: #krate::Span = #krate::Span {
                id: #id,
                first: #first,
                end: #end,
                offset: ::core::mem::offset_of!(#ident, #first_field),
                size: ::core::mem::size_of::<#view>(),
            };
        }

        #(#cfgs)*
        unsafe impl #krate::HasView<{ #ident::#span_name_ident }> for #ident {
            type View = #view;
        }

        #instantiation

        #(#cfgs)*
        const _: () = {
            #(#asserts)*
        };
    }
}

