use crate::shell::Shell;
use anyhow::{anyhow, bail, Context as _};
use cargo_metadata as cm;
use fixedbitset::FixedBitSet;
use if_chain::if_chain;
use itertools::Itertools as _;
use maplit::{btreemap, btreeset};
use proc_macro2::LineColumn;
use proc_macro2::TokenStream;
use proc_macro2::{Span, TokenTree};
use quote::{quote, ToTokens as _};
use std::collections::BTreeSet;
use std::{collections::BTreeMap, fmt, mem, path::PathBuf, str};
use syn::{
    parse_quote,
    spanned::Spanned,
    visit::{self, Visit},
    Attribute, Ident, Item, ItemConst, ItemEnum, ItemExternCrate, ItemFn, ItemForeignMod, ItemImpl,
    ItemMacro, ItemMacro2, ItemMod, ItemStatic, ItemStruct, ItemTrait, ItemTraitAlias, ItemType,
    ItemUnion, ItemUse, Lit, Macro, Meta, MetaList, MetaNameValue, NestedMeta, PathSegment,
    UseGroup, UseName, UsePath, UseRename, UseTree,
};

#[derive(Default)]
pub(crate) struct Equipments<'cm> {
    pub(crate) span: Option<Span>,
    pub(crate) uses: Vec<ItemUse>,
    pub(crate) directly_used_mods: BTreeMap<&'cm cm::PackageId, BTreeSet<Ident>>,
    pub(crate) contents: BTreeMap<(cm::PackageId, Ident), BTreeMap<Ident, Option<String>>>,
}

pub(crate) fn equipments<'cm>(
    file: &syn::File,
    mut lib_info: impl FnMut(&Ident) -> anyhow::Result<(&'cm cm::PackageId, PathBuf)>,
) -> anyhow::Result<Equipments<'cm>> {
    // TODO: find the matched attributes in inline/external `mod`s and raise an error

    let mut uses = vec![];

    for item in &file.items {
        if let Item::Use(item_use) = item {
            if let Some(i) = item_use
                .attrs
                .iter()
                .enumerate()
                .flat_map(|(i, a)| a.parse_meta().map(|m| (i, m)))
                .map(|(i, meta)| {
                    fn starts_with_cargo_equip(path: &syn::Path) -> bool {
                        matches!(
                            path.segments.first(),
                            Some(PathSegment { ident, .. }) if ident == "cargo_equip"
                        )
                    }

                    let validate = |meta: &Meta| -> syn::Result<()> {
                        if matches!(
                            meta.path().segments.first(),
                            Some(PathSegment { ident, .. }) if ident == "cargo_equip"
                        ) {
                            if meta
                                .path()
                                .segments
                                .iter()
                                .map(|PathSegment { ident, .. }| ident)
                                .collect::<Vec<_>>()
                                != ["cargo_equip", "equip"]
                            {
                                return Err(syn::Error::new(
                                    item_use.span(),
                                    "expected `cargo_equip::equip`",
                                ));
                            }

                            if let Meta::List(_) | Meta::NameValue(_) = meta {
                                return Err(syn::Error::new(
                                    item_use.span(),
                                    "`cargo_equip::equip` take no argument",
                                ));
                            }
                        }
                        Ok(())
                    };

                    if starts_with_cargo_equip(meta.path()) {
                        validate(&meta)?;
                        return Ok::<_, syn::Error>(Some(i));
                    }

                    if_chain! {
                        if let Meta::List(MetaList { path, nested, .. }) = &meta;
                        if matches!(path.get_ident(), Some(i) if i == "cfg_attr");
                        if let [expr, attr] = *nested.iter().collect::<Vec<_>>();
                        let expr = expr.to_token_stream().to_string();
                        if let Ok(expr) = cfg_expr::Expression::parse(&expr);
                        if expr.eval(|pred| *pred == cfg_expr::Predicate::Flag("cargo_equip"));
                        if let NestedMeta::Meta(attr) = attr;
                        if starts_with_cargo_equip(attr.path());
                        then {
                            validate(attr)?;
                            return Ok(Some(i));
                        }
                    }

                    Ok(None)
                })
                .flat_map(Result::transpose)
                .next()
            {
                let span = item_use.span();
                let mut item_use = item_use.clone();
                item_use.attrs.remove(i?);
                uses.push((item_use, span));
            }
        }
    }

    if uses.len() > 1 {
        return Err(error_with_span("multiple `cargo_equip` usage", file.span()));
    }

    let (item_use, span) = if let Some(target) = uses.pop() {
        target
    } else {
        return Ok(Equipments::default());
    };

    if item_use.leading_colon.is_none() {
        return Err(error_with_span(
            "leading semicolon (`::`) is requied",
            item_use.tree.span(),
        ));
    }

    let mut equipments = Equipments {
        span: Some(span),
        uses: vec![],
        directly_used_mods: btreemap!(),
        contents: btreemap!(),
    };

    let use_paths = match &item_use.tree {
        UseTree::Path(use_path) => vec![use_path],
        UseTree::Group(use_group) => use_group
            .items
            .iter()
            .map(|item| match item {
                UseTree::Path(use_path) => Ok(use_path),
                _ => Err(error_with_span(
                    "expected `::{ $ident::$tree, .. }`",
                    item_use.tree.span(),
                )),
            })
            .collect::<Result<_, _>>()?,
        _ => {
            return Err(error_with_span(
                "expected `::$ident::$tree` or `::{ .. }`",
                item_use.tree.span(),
            ));
        }
    };

    for UsePath { ident, tree, .. } in use_paths {
        let extern_crate_name = ident.clone();
        let (lib_pkg, lib_src_path) = lib_info(&extern_crate_name)?;
        let (uses, directly_used_mods, contents) =
            extract_usage(&item_use, tree, &extern_crate_name, &lib_src_path)?;
        equipments.uses.extend(uses);
        equipments
            .directly_used_mods
            .insert(lib_pkg, directly_used_mods);
        equipments
            .contents
            .entry((lib_pkg.clone(), extern_crate_name))
            .or_default()
            .extend(contents.into_iter().map(|(e, c)| (e, Some(c))));
    }

    Ok(equipments)
}

#[allow(clippy::type_complexity)]
fn extract_usage(
    item_use: &ItemUse,
    tree: &UseTree,
    extern_crate_name: &Ident,
    lib_src_path: &std::path::Path,
) -> anyhow::Result<(Vec<ItemUse>, BTreeSet<Ident>, BTreeMap<Ident, String>)> {
    let new_item_use = |tree| ItemUse {
        vis: parse_quote!(pub),
        leading_colon: None,
        tree,
        ..item_use.clone()
    };

    let (mod_names, uses) = match tree {
        UseTree::Path(use_path) => {
            let mods = Some(btreeset!(use_path.ident.clone()));
            let uses = vec![new_item_use(
                parse_quote!(self::#extern_crate_name::#use_path),
            )];
            (mods, uses)
        }
        UseTree::Name(UseName { ident }) => {
            let mods = Some(btreeset!(ident.clone()));
            let uses = vec![new_item_use(parse_quote!(self::#extern_crate_name::#ident))];
            (mods, uses)
        }
        UseTree::Rename(UseRename { ident, rename, .. }) => {
            let mods = Some(btreeset!(ident.clone()));
            let uses = vec![new_item_use(
                parse_quote!(self::#extern_crate_name::#ident as #rename),
            )];
            (mods, uses)
        }
        UseTree::Glob(_) => {
            let mods = None;
            let uses = vec![];
            (mods, uses)
        }
        UseTree::Group(UseGroup { items, .. }) => {
            let mut flatten = items.iter().collect::<Vec<_>>();
            while flatten.iter().any(|x| matches!(x, UseTree::Group(_))) {
                for item in mem::take(&mut flatten) {
                    if let UseTree::Group(UseGroup { items, .. }) = item {
                        flatten.extend(items);
                    } else {
                        flatten.push(item);
                    }
                }
            }
            let (mut mods, mut uses) = (Some(btreeset![]), vec![]);
            for item in flatten {
                match item {
                    UseTree::Path(use_path) => {
                        if let Some(mods) = &mut mods {
                            mods.insert(use_path.ident.clone());
                        }
                        uses.push(new_item_use(
                            parse_quote!(self::#extern_crate_name::#use_path),
                        ));
                    }
                    UseTree::Name(UseName { ident }) => {
                        if let Some(mods) = &mut mods {
                            mods.insert(ident.clone());
                        }
                        uses.push(new_item_use(parse_quote!(self::#extern_crate_name::#ident)));
                    }
                    UseTree::Rename(UseRename { ident, rename, .. }) => {
                        if let Some(mods) = &mut mods {
                            mods.insert(ident.clone());
                        }
                        uses.push(new_item_use(
                            parse_quote!(self::#extern_crate_name::#ident as #rename),
                        ));
                    }
                    UseTree::Glob(_) => {
                        mods = None;
                    }
                    UseTree::Group(_) => {
                        unreachable!("should be flatten");
                    }
                }
            }
            (mods, uses)
        }
    };

    let lib_contents = {
        let file = syn::parse_file(&std::fs::read_to_string(lib_src_path)?)?;

        let mut lib_contents = btreemap!();

        for item in &file.items {
            if let Item::Mod(item_mod) = item {
                if item_mod.content.is_some() {
                    todo!("TODO: inline mod");
                }
                if let Some(Meta::List(_)) = item_mod
                    .attrs
                    .iter()
                    .flat_map(|a| a.parse_meta())
                    .find(|m| matches!(m.path().get_ident(), Some(i) if i == "path"))
                {
                    todo!("TODO: `#[path = \"..\"]`");
                }
                let paths = vec![
                    lib_src_path
                        .with_file_name("")
                        .join(item_mod.ident.to_string())
                        .join("mod.rs"),
                    lib_src_path
                        .with_file_name("")
                        .join(item_mod.ident.to_string())
                        .with_extension("rs"),
                ];
                if let Some(path) = paths.iter().find(|p| p.exists()) {
                    let content = std::fs::read_to_string(path)?;
                    lib_contents.insert(item_mod.ident.clone(), content);
                } else {
                    bail!("none of `{:?}` found", paths);
                }
            }
        }
        lib_contents
    };

    let mod_names = mod_names.unwrap_or_else(|| lib_contents.keys().cloned().collect());

    Ok((uses, mod_names, lib_contents))
}

fn error_with_span(message: impl fmt::Display, span: Span) -> anyhow::Error {
    anyhow!("{}", message).context(format!("Error at {:?}", span))
}

pub(crate) fn replace_extern_crates(
    code: &str,
    convert_extern_crate_name: impl FnMut(&syn::Ident) -> Option<String>,
) -> anyhow::Result<String> {
    struct Visitor<'a, F> {
        replacements: &'a mut anyhow::Result<BTreeMap<(LineColumn, LineColumn), String>>,
        convert_extern_crate_name: F,
    };

    impl<F: FnMut(&syn::Ident) -> Option<String>> Visit<'_> for Visitor<'_, F> {
        fn visit_item_extern_crate(&mut self, item_use: &ItemExternCrate) {
            let ItemExternCrate {
                attrs,
                vis,
                ident,
                rename,
                semi_token,
                ..
            } = item_use;

            if contains_attr(&attrs, &parse_quote!(use_another_lib)) {
                let to = if let Some(to) = (self.convert_extern_crate_name)(ident) {
                    to
                } else {
                    *self.replacements = Err(anyhow!("`{}` is not on the list", ident));
                    return;
                };
                let to = Ident::new(&to, Span::call_site());
                self.replacements
                    .as_mut()
                    .unwrap_or_else(|_| unreachable!())
                    .insert(
                        (item_use.span().start(), semi_token.span().end()),
                        if let Some((_, rename)) = rename {
                            quote!(#(#attrs)* #vis use crate::#to as #rename;).to_string()
                        } else {
                            quote!(#(#attrs)* #vis use crate::#to as #ident;).to_string()
                        },
                    );
            }
        }
    }

    let file = syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;

    let mut replacements = Ok(btreemap!());

    Visitor {
        replacements: &mut replacements,
        convert_extern_crate_name,
    }
    .visit_file(&file);

    let replacements = replacements?;

    Ok(replace_ranges(code, replacements))
}

pub(crate) fn modify_macros(code: &str, extern_crate_name: &str) -> anyhow::Result<String> {
    fn find_dollar_crates(token_stream: TokenStream, acc: &mut BTreeSet<LineColumn>) {
        for (i, (tt1, tt2)) in token_stream.into_iter().tuple_windows().enumerate() {
            if i == 0 {
                if let proc_macro2::TokenTree::Group(group) = &tt1 {
                    find_dollar_crates(group.stream(), acc);
                }
            }

            if let proc_macro2::TokenTree::Group(group) = &tt2 {
                find_dollar_crates(group.stream(), acc);
            }

            if matches!(
                (&tt1, &tt2),
                (proc_macro2::TokenTree::Punct(p), proc_macro2::TokenTree::Ident(i))
                if p.as_char() == '$' && i == "crate"
            ) {
                acc.insert(tt2.span().end());
            }
        }
    };

    fn exclude_crate_macros(token_stream: TokenStream, acc: &mut BTreeSet<LineColumn>) {
        for tts in token_stream
            .clone()
            .into_iter()
            .collect::<Vec<_>>()
            .windows(6)
        {
            if let [proc_macro2::TokenTree::Punct(punct1), proc_macro2::TokenTree::Ident(ident), proc_macro2::TokenTree::Punct(punct2), proc_macro2::TokenTree::Punct(punct3), proc_macro2::TokenTree::Ident(_), proc_macro2::TokenTree::Punct(punct4)] =
                &*tts
            {
                if punct1.as_char() == '$'
                    && ident == "crate"
                    && punct2.as_char() == ':'
                    && punct3.as_char() == ':'
                    && punct4.as_char() == '!'
                {
                    acc.remove(&ident.span().end());
                }
            }
        }

        for tt in token_stream.clone() {
            if let proc_macro2::TokenTree::Group(group) = tt {
                exclude_crate_macros(group.stream(), acc);
            }
        }
    }

    let syn::File { items, .. } = syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;

    let mut dollar_crates = btreeset!();

    for item in items {
        if let Item::Macro(ItemMacro {
            attrs,
            mac: Macro { tokens, .. },
            ..
        }) = item
        {
            if contains_attr(&attrs, &parse_quote!(translate_dollar_crates)) {
                find_dollar_crates(tokens.clone(), &mut dollar_crates);
                exclude_crate_macros(tokens, &mut dollar_crates);
            }
        }
    }

    Ok(replace_ranges(
        code,
        dollar_crates
            .into_iter()
            .map(|p| ((p, p), format!("::{}", extern_crate_name)))
            .collect(),
    ))
}

fn contains_attr(attrs: &[Attribute], target: &Ident) -> bool {
    for attr in attrs {
        if_chain! {
            if let Ok(meta) = attr.parse_meta();
            if let Meta::List(MetaList { path, nested, .. }) = &meta;
            if matches!(path.get_ident(), Some(i) if i == "cfg_attr");
            if let [expr, attrs @ ..] = &*nested.iter().collect::<Vec<_>>();
            let expr = expr.to_token_stream().to_string();
            if let Ok(expr) = cfg_expr::Expression::parse(&expr);
            if expr.eval(|pred| *pred == cfg_expr::Predicate::Flag("cargo_equip"));
            then {
                for attr in attrs {
                    if_chain! {
                        if let NestedMeta::Meta(attr) = attr;
                        if let [seg1, seg2] = *attr.path().segments.iter().collect::<Vec<_>>();
                        if matches!(seg1, PathSegment { ident, .. } if ident == "cargo_equip");
                        if let PathSegment { ident, .. } = seg2;
                        if ident == target;
                        then {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn replace_ranges(code: &str, replacements: BTreeMap<(LineColumn, LineColumn), String>) -> String {
    let replacements = replacements.into_iter().collect::<Vec<_>>();
    let mut replacements = &*replacements;
    let mut skip_until = None;
    let mut ret = "".to_owned();
    for (i, s) in code.trim_end().split('\n').enumerate() {
        for (j, c) in s.chars().enumerate() {
            if_chain! {
                if let Some(((start, end), replacement)) = replacements.get(0);
                if (i, j) == (start.line - 1, start.column);
                then {
                    ret += replacement;
                    if start == end {
                        ret.push(c);
                    } else {
                        skip_until = Some(*end);
                    }
                    replacements = &replacements[1..];
                } else {
                    if !matches!(skip_until, Some(LineColumn { line, column }) if (i, j) < (line - 1, column)) {
                        ret.push(c);
                        skip_until = None;
                    }
                }
            }
        }
        ret += "\n";
    }

    debug_assert!(syn::parse_file(&code).is_ok());

    ret
}

pub(crate) fn prepend_mod_doc(code: &str, append: &str) -> syn::Result<String> {
    let syn::File { shebang, attrs, .. } = syn::parse_file(code)?;

    let mut code = code.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let mut doc = vec![];

    if shebang.is_some() {
        code[0] = "".to_owned();
    }

    for (val, span) in attrs
        .iter()
        .flat_map(Attribute::parse_meta)
        .flat_map(|meta| match meta {
            Meta::NameValue(name_value) => Some(name_value),
            _ => None,
        })
        .filter(|MetaNameValue { path, .. }| matches!(path.get_ident(), Some(i) if i == "doc"))
        .flat_map(|name_value| match &name_value.lit {
            Lit::Str(val) => Some((val.value(), name_value.span())),
            _ => None,
        })
    {
        doc.push(val);

        if span.start().line == span.end().line {
            let i = span.start().line - 1;
            let l = span.start().column;
            let r = span.end().column;
            code[i] = format!("{}{}{}", &code[i][..l], " ".repeat(r - l), &code[i][r..]);
        } else {
            let i = span.start().line - 1;
            let l = span.start().column;
            code[i] = format!("{}{}", &code[i][..l], code[i].len() - l);

            for line in &mut code[span.start().line..span.end().line - 2] {
                *line = " ".repeat(line.len());
            }

            let i = span.end().line - 1;
            let r = span.end().column;
            code[i] = format!("{}{}", " ".repeat(code[i].len() - r), &code[i][r..]);
        }
    }

    Ok(format!(
        "{}{}{}{}\n{}\n",
        match shebang {
            Some(shebang) => format!("{}\n", shebang),
            None => "".to_owned(),
        },
        doc.iter()
            .format_with("", |l, f| f(&format_args!("//!{}\n", l))),
        if doc.iter().all(|s| s.is_empty()) {
            ""
        } else {
            "//!\n"
        },
        append
            .lines()
            .format_with("", |l, f| f(&format_args!("//!{}\n", l))),
        code.join("\n").trim_start(),
    ))
}

pub(crate) fn erase_test_items(code: &str) -> anyhow::Result<String> {
    fn contains_cfg_test(attrs: &[Attribute]) -> bool {
        attrs
            .iter()
            .flat_map(Attribute::parse_meta)
            .flat_map(|meta| match meta {
                Meta::List(MetaList { path, nested, .. }) => Some((path, nested)),
                _ => None,
            })
            .any(|(path, nested)| {
                matches!(path.get_ident(), Some(i) if i == "cfg")
                    && matches!(
                        cfg_expr::Expression::parse(&nested.to_token_stream().to_string()), Ok(expr)
                        if expr.eval(|pred| *pred == cfg_expr::Predicate::Test)
                    )
            })
    }

    struct Visitor<'a>(&'a mut [FixedBitSet]);

    macro_rules! visit {
        ($(($method:ident, <$ty:ty>)),* $(,)?) => {
            $(
                fn $method(&mut self, item: &'_ $ty) {
                    if contains_cfg_test(&item.attrs) {
                        set_span(self.0, item.span(), true);
                    } else {
                        visit::$method(self, item);
                    }
                }
            )*
        }
    }

    impl Visit<'_> for Visitor<'_> {
        visit! {
            (visit_item_const, <ItemConst>),
            (visit_item_enum, <ItemEnum>),
            (visit_item_extern_crate, <ItemExternCrate>),
            (visit_item_fn, <ItemFn>),
            (visit_item_foreign_mod, <ItemForeignMod>),
            (visit_item_impl, <ItemImpl>),
            (visit_item_macro, <ItemMacro>),
            (visit_item_macro2, <ItemMacro2>),
            (visit_item_mod, <ItemMod>),
            (visit_item_static, <ItemStatic>),
            (visit_item_struct, <ItemStruct>),
            (visit_item_trait, <ItemTrait>),
            (visit_item_trait_alias, <ItemTraitAlias>),
            (visit_item_type, <ItemType>),
            (visit_item_union, <ItemUnion>),
            (visit_item_use, <ItemUse>),
        }

        fn visit_file(&mut self, file: &syn::File) {
            if contains_cfg_test(&file.attrs) {
                for mask in &mut *self.0 {
                    mask.insert_range(..);
                }
            } else {
                visit::visit_file(self, file);
            }
        }
    }

    erase(code, |mask, token_stream| {
        syn::parse2(token_stream).map(|f| Visitor(mask).visit_file(&f))
    })
}

pub(crate) fn erase_docs(code: &str) -> anyhow::Result<String> {
    struct Visitor<'a>(&'a mut [FixedBitSet]);

    impl Visit<'_> for Visitor<'_> {
        fn visit_attribute(&mut self, attr: &'_ Attribute) {
            if matches!(
                attr.parse_meta(), Ok(m) if matches!(m.path().get_ident(), Some(i) if i == "doc")
            ) {
                set_span(self.0, attr.span(), true);
            }
        }
    }

    erase(code, |mask, token_stream| {
        syn::parse2(token_stream).map(|f| Visitor(mask).visit_file(&f))
    })
}

pub(crate) fn erase_comments(code: &str) -> anyhow::Result<String> {
    fn visit_file(mask: &mut [FixedBitSet], token_stream: TokenStream) -> syn::Result<()> {
        fn visit_token_stream(mask: &mut [FixedBitSet], token_stream: TokenStream) {
            for tt in token_stream {
                if let TokenTree::Group(group) = tt {
                    set_span(mask, group.span_open(), false);
                    visit_token_stream(mask, group.stream());
                    set_span(mask, group.span_close(), false);
                } else {
                    set_span(mask, tt.span(), false);
                }
            }
        }

        for mask in &mut *mask {
            mask.insert_range(..);
        }
        visit_token_stream(mask, token_stream);
        Ok(())
    }

    erase(code, visit_file)
}

fn erase(
    code: &str,
    visit_file: fn(&mut [FixedBitSet], TokenStream) -> syn::Result<()>,
) -> anyhow::Result<String> {
    let code = if code.starts_with("#!") {
        let (_, code) = code.split_at(code.find('\n').unwrap_or_else(|| code.len()));
        code
    } else {
        code
    };

    let token_stream = code
        .parse::<TokenStream>()
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could lex the code")?;

    let mut erase = code
        .lines()
        .map(|l| FixedBitSet::with_capacity(l.chars().count()))
        .collect::<Vec<_>>();

    visit_file(&mut erase, token_stream)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could parse the code")?;

    let mut acc = "".to_owned();
    for (line, erase) in code.lines().zip_eq(erase) {
        for (j, ch) in line.chars().enumerate() {
            acc.push(if erase[j] { ' ' } else { ch });
        }
        acc += "\n";
    }
    Ok(acc.trim_start().to_owned())
}

fn set_span(mask: &mut [FixedBitSet], span: Span, p: bool) {
    if span.start().line == span.end().line {
        let i = span.start().line - 1;
        let l = span.start().column;
        let r = span.end().column;
        mask[i].set_range(l..r, p);
    } else {
        let i1 = span.start().line - 1;
        let i2 = span.end().line - 1;
        let l = span.start().column;
        mask[i1].insert_range(l..);
        for mask in &mut mask[i1 + 1..i2] {
            mask.set_range(.., p);
        }
        let r = span.end().column;
        mask[i2].set_range(..r, p);
    }
}

pub(crate) fn minify(code: &str, shell: &mut Shell, name: Option<&str>) -> anyhow::Result<String> {
    fn minify(acc: &mut String, token_stream: TokenStream) {
        let mut space_on_ident = false;
        let mut space_on_punct = false;
        let mut space_on_literal = false;
        for tt in token_stream {
            match tt {
                proc_macro2::TokenTree::Group(group) => {
                    let (left, right) = match group.delimiter() {
                        proc_macro2::Delimiter::Parenthesis => ('(', ')'),
                        proc_macro2::Delimiter::Brace => ('{', '}'),
                        proc_macro2::Delimiter::Bracket => ('[', ']'),
                        proc_macro2::Delimiter::None => (' ', ' '),
                    };
                    acc.push(left);
                    minify(acc, group.stream());
                    acc.push(right);
                    space_on_ident = false;
                    space_on_punct = false;
                    space_on_literal = false;
                }
                proc_macro2::TokenTree::Ident(ident) => {
                    if space_on_ident {
                        *acc += " ";
                    }
                    *acc += &ident.to_string();
                    space_on_ident = true;
                    space_on_punct = false;
                    space_on_literal = true;
                }
                proc_macro2::TokenTree::Punct(punct) => {
                    if space_on_punct {
                        *acc += " ";
                    }
                    acc.push(punct.as_char());
                    space_on_ident = false;
                    space_on_punct = punct.spacing() == proc_macro2::Spacing::Alone;
                    space_on_literal = false;
                }
                proc_macro2::TokenTree::Literal(literal) => {
                    if space_on_literal {
                        *acc += " ";
                    }
                    *acc += &literal.to_string();
                    space_on_ident = false;
                    space_on_punct = false;
                    space_on_literal = true;
                }
            }
        }
    }

    let token_stream = syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?
        .into_token_stream();

    let safe = token_stream.to_string();

    let mut acc = "".to_owned();
    minify(&mut acc, token_stream);

    if matches!(syn::parse_file(&acc), Ok(f) if f.to_token_stream().to_string() == safe) {
        Ok(acc)
    } else {
        shell.warn(format!(
            "could not minify the code. inserting spaces{}",
            name.map(|s| format!(": `{}`", s)).unwrap_or_default(),
        ))?;
        Ok(safe)
    }
}

#[cfg(test)]
mod tests {
    use difference::assert_diff;

    #[test]
    fn modify_macros() -> anyhow::Result<()> {
        fn test(input: &str, expected: &str) -> anyhow::Result<()> {
            static EXTERN_CRATE_NAME: &str = "lib";
            let actual = super::modify_macros(input, EXTERN_CRATE_NAME)?;
            assert_diff!(expected, &actual, "\n", 0);
            Ok(())
        }

        test(
            r#"#[cfg_attr(cargo_equip, cargo_equip::translate_dollar_crates)]
#[macro_export]
macro_rules! hello {
    (1 $(,)?) => {
        $crate::hello::hello();
        $crate::__hello_inner!($n)
    };
    (0 $(,)?) => {};
}

macro_rules! _without_attr {
    () => {
        let _ = $crate::hello;
        $crate::hello!(0);
    };
}
"#,
            r#"#[cfg_attr(cargo_equip, cargo_equip::translate_dollar_crates)]
#[macro_export]
macro_rules! hello {
    (1 $(,)?) => {
        $crate::lib::hello::hello();
        $crate::__hello_inner!($n)
    };
    (0 $(,)?) => {};
}

macro_rules! _without_attr {
    () => {
        let _ = $crate::hello;
        $crate::hello!(0);
    };
}
"#,
        )
    }

    #[test]
    fn prepend_mod_doc() -> syn::Result<()> {
        fn test(code: &str, append: &str, expected: &str) -> syn::Result<()> {
            let actual = super::prepend_mod_doc(code, append)?;
            assert_diff!(expected, &actual, "\n", 0);
            Ok(())
        }

        test(
            r#"//! aaaaaaa
//! bbbbbbb
//! ccccccc

fn main() {
    todo!();
}
"#,
            r" ddddddd
",
            r#"//! aaaaaaa
//! bbbbbbb
//! ccccccc
//!
//! ddddddd

fn main() {
    todo!();
}
"#,
        )?;
        test(
            r#"fn main() {
    todo!();
}
"#,
            r" dddddd
",
            r#"//! dddddd

fn main() {
    todo!();
}
"#,
        )?;
        Ok(())
    }

    #[test]
    fn erase_test_items() -> anyhow::Result<()> {
        fn test(input: &str, expected: &str) -> anyhow::Result<()> {
            let actual = super::erase_test_items(input)?;
            assert_diff!(expected, &actual, "\n", 0);
            Ok(())
        }

        test(
            r#"//
#[cfg(test)]
use foo::Foo;

fn hello() -> &'static str {
    #[cfg(test)]
    use bar::Bar;

    "Hello!"
}

#[cfg(test)]
mod tests {}
"#,
            r#"//
            
             

fn hello() -> &'static str {
                
                 

    "Hello!"
}

            
            
"#,
        )
    }

    #[test]
    fn erase_docs() -> anyhow::Result<()> {
        fn test(input: &str, expected: &str) -> anyhow::Result<()> {
            let actual = super::erase_docs(input)?;
            assert_diff!(expected, &actual, "\n", 0);
            Ok(())
        }

        test(
            r#"//! aaaaa
//! bbbbb

fn main() {}

/// ccccc
struct Foo;
"#,
            r#"fn main() {}

         
struct Foo;
"#,
        )?;

        test(
            r#"//! モジュールのドキュメント

/// アイテムのドキュメント
fn foo() {}
"#,
            r#"fn foo() {}
"#,
        )
    }

    #[test]
    fn erase_comments() -> anyhow::Result<()> {
        fn test(input: &str, expected: &str) -> anyhow::Result<()> {
            let actual = super::erase_comments(input)?;
            assert_diff!(expected, &actual, "\n", 0);
            Ok(())
        }

        test(
            r#"// aaaaa
// bbbbb
fn main() {
    // ccccc
    /*ddddd*/println!("Hi!");/*eeeee*/
    // fffff
}
// ggggg
"#,
            r#"fn main() {
            
             println!("Hi!");         
            
}
        
"#,
        )?;

        test(
            r#"/* aaaaa */ type A = (i64, i64, i64); // bbbbb
"#,
            r#"type A = (i64, i64, i64);         
"#,
        )?;

        test(
            r#"// あああ
/*いいい*/fn foo() {
    let _ = 1 + 1; // ううううう
}
"#,
            r#"fn foo() {
    let _ = 1 + 1;         
}
"#,
        )
    }
}
