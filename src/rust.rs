use crate::shell::Shell;
use anyhow::{anyhow, bail, Context as _};
use cargo_metadata as cm;
use fixedbitset::FixedBitSet;
use if_chain::if_chain;
use itertools::Itertools as _;
use maplit::{btreemap, btreeset};
use proc_macro2::{Span, TokenTree};
use quote::ToTokens as _;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fmt, io, mem,
    path::PathBuf,
    str,
};
use syn::{
    parse_quote,
    spanned::Spanned,
    visit::{self, Visit},
    Attribute, Ident, Item, ItemConst, ItemEnum, ItemExternCrate, ItemFn, ItemForeignMod, ItemImpl,
    ItemMacro, ItemMacro2, ItemMod, ItemStatic, ItemStruct, ItemTrait, ItemTraitAlias, ItemType,
    ItemUnion, ItemUse, Lit, Meta, MetaList, MetaNameValue, NestedMeta, PathSegment, UseGroup,
    UseName, UsePath, UseRename, UseTree,
};

#[derive(Default)]
pub(crate) struct Equipments {
    pub(crate) span: Option<Span>,
    pub(crate) uses: Vec<ItemUse>,
    pub(crate) contents: BTreeMap<(cm::PackageId, Ident), BTreeMap<Ident, Option<String>>>,
}

pub(crate) fn equipments(
    file: &syn::File,
    shell: &mut Shell,
    mut lib_info: impl FnMut(
        &Ident,
    ) -> anyhow::Result<(
        cm::PackageId,
        PathBuf,
        HashMap<String, BTreeSet<String>>,
    )>,
) -> anyhow::Result<Equipments> {
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
        let (lib_pkg, lib_src_path, mod_dependencies) = lib_info(&extern_crate_name)?;
        let (uses, contents) =
            extract_usage(&item_use, tree, &lib_src_path, &mod_dependencies, shell)?;
        equipments.uses.extend(uses);
        equipments
            .contents
            .entry((lib_pkg, extern_crate_name))
            .or_default()
            .extend(contents);
    }

    Ok(equipments)
}

#[allow(clippy::type_complexity)]
fn extract_usage(
    item_use: &ItemUse,
    tree: &UseTree,
    lib_src_path: &std::path::Path,
    mod_dependencies: &HashMap<String, BTreeSet<String>>,
    shell: &mut Shell,
) -> anyhow::Result<(Vec<ItemUse>, BTreeMap<Ident, Option<String>>)> {
    let new_item_use = |tree| ItemUse {
        leading_colon: None,
        tree,
        ..item_use.clone()
    };

    let (mod_names, uses) = match tree {
        UseTree::Path(use_path) => {
            let mods = Some(btreeset!(use_path.ident.clone()));
            let uses = vec![new_item_use(parse_quote!(self::#use_path))];
            (mods, uses)
        }
        UseTree::Name(UseName { ident }) => {
            let mods = Some(btreeset!(ident.clone()));
            let uses = vec![];
            (mods, uses)
        }
        UseTree::Rename(UseRename { ident, rename, .. }) => {
            let mods = Some(btreeset!(ident.clone()));
            let uses = vec![new_item_use(parse_quote!(self::#ident as #rename))];
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
                        uses.push(new_item_use(parse_quote!(self::#use_path)));
                    }
                    UseTree::Name(UseName { ident }) => {
                        if let Some(mods) = &mut mods {
                            mods.insert(ident.clone());
                        }
                    }
                    UseTree::Rename(UseRename { ident, rename, .. }) => {
                        if let Some(mods) = &mut mods {
                            mods.insert(ident.clone());
                        }
                        uses.push(new_item_use(parse_quote!(self::#ident as #rename)));
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

    let mod_names = mod_names
        .map(|mod_names| {
            let mut mod_names = mod_names
                .iter()
                .map(ToString::to_string)
                .collect::<BTreeSet<_>>();
            let mut queue = mod_names.iter().cloned().collect::<VecDeque<_>>();
            while let Some(name) = queue.pop_front() {
                if let Some(mod_dependencies) = mod_dependencies.get(&name) {
                    for mod_dependency in mod_dependencies {
                        if !mod_names.contains(mod_dependency) {
                            mod_names.insert(mod_dependency.clone());
                            queue.push_back(mod_dependency.clone());
                        }
                    }
                } else {
                    shell.warn(format!(
                        "missing `package.metadata.cargo-equip-lib.mod-dependencies.\"{}\"`. \
                         including all of the modules",
                        name,
                    ))?;
                    return Ok(None);
                }
            }
            Ok::<_, io::Error>(Some(mod_names))
        })
        .transpose()?
        .flatten();

    let lib_contents = {
        let file = syn::parse_file(&std::fs::read_to_string(lib_src_path)?)?;

        let mut lib_contents = btreemap!();

        for item in &file.items {
            if let Item::Mod(item_mod) = item {
                let is_target = mod_names
                    .as_ref()
                    .map_or(true, |names| names.contains(&item_mod.ident.to_string()));
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
                    let content = if is_target {
                        Some(std::fs::read_to_string(path)?)
                    } else {
                        None
                    };
                    lib_contents.insert(item_mod.ident.clone(), content);
                } else {
                    bail!("none of `{:?}` found", paths);
                }
            }
        }
        lib_contents
    };

    Ok((uses, lib_contents))
}

fn error_with_span(message: impl fmt::Display, span: Span) -> anyhow::Error {
    anyhow!("{}", message).context(format!("Error at {:?}", span))
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
    fn visit_file(
        mask: &mut [FixedBitSet],
        token_stream: proc_macro2::TokenStream,
    ) -> syn::Result<()> {
        fn visit_token_stream(mask: &mut [FixedBitSet], token_stream: proc_macro2::TokenStream) {
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
    visit_file: fn(&mut [FixedBitSet], proc_macro2::TokenStream) -> syn::Result<()>,
) -> anyhow::Result<String> {
    let code = if code.starts_with("#!") {
        let (_, code) = code.split_at(code.find('\n').unwrap_or_else(|| code.len()));
        code
    } else {
        code
    };

    let token_stream = code
        .parse::<proc_macro2::TokenStream>()
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
    fn minify(acc: &mut String, token_stream: proc_macro2::TokenStream) {
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
