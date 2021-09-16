use crate::{ra_proc_macro::ProcMacroExpander, shell::Shell};
use anyhow::{anyhow, bail, Context as _};
use camino::{Utf8Path, Utf8PathBuf};
use fixedbitset::FixedBitSet;
use if_chain::if_chain;
use itertools::Itertools as _;
use maplit::btreemap;
use proc_macro2::{LineColumn, Span, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, VecDeque},
    env, mem,
    ops::Range,
    str,
};
use syn::{
    parse::{ParseStream, Parser as _},
    parse_quote,
    punctuated::{Pair, Punctuated},
    spanned::Spanned,
    visit::{self, Visit},
    Arm, AttrStyle, Attribute, BareFnArg, ConstParam, Expr, ExprArray, ExprAssign, ExprAssignOp,
    ExprAsync, ExprAwait, ExprBinary, ExprBlock, ExprBox, ExprBreak, ExprCall, ExprCast,
    ExprClosure, ExprContinue, ExprField, ExprForLoop, ExprGroup, ExprIf, ExprIndex, ExprLet,
    ExprLit, ExprLoop, ExprMacro, ExprMatch, ExprMethodCall, ExprParen, ExprPath, ExprRange,
    ExprReference, ExprRepeat, ExprReturn, ExprStruct, ExprTry, ExprTryBlock, ExprTuple, ExprType,
    ExprUnary, ExprUnsafe, ExprWhile, ExprYield, Field, FieldPat, FieldValue, ForeignItemFn,
    ForeignItemMacro, ForeignItemStatic, ForeignItemType, Ident, ImplItemConst, ImplItemMacro,
    ImplItemMethod, ImplItemType, Item, ItemConst, ItemEnum, ItemExternCrate, ItemFn,
    ItemForeignMod, ItemImpl, ItemMacro, ItemMacro2, ItemMod, ItemStatic, ItemStruct, ItemTrait,
    ItemTraitAlias, ItemType, ItemUnion, ItemUse, LifetimeDef, Lit, LitStr, Local, Macro, Meta,
    MetaList, MetaNameValue, NestedMeta, PatBox, PatIdent, PatLit, PatMacro, PatOr, PatPath,
    PatRange, PatReference, PatRest, PatSlice, PatStruct, PatTuple, PatTupleStruct, PatType,
    PatWild, PathSegment, Receiver, Token, TraitItemConst, TraitItemMacro, TraitItemMethod,
    TraitItemType, TypeParam, UseGroup, UseName, UsePath, UseRename, UseTree, Variadic, Variant,
    VisRestricted,
};

pub(crate) fn find_skip_attribute(code: &str) -> anyhow::Result<bool> {
    let syn::File { attrs, .. } = syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;

    Ok(attrs
        .iter()
        .flat_map(Attribute::parse_meta)
        .flat_map(|meta| match meta {
            Meta::List(meta_list) => Some(meta_list),
            _ => None,
        })
        .filter(|MetaList { path, .. }| path.is_ident("cfg_attr"))
        .any(|MetaList { nested, .. }| {
            matches!(
                *nested.iter().collect::<Vec<_>>(),
                [pred, attr]
                if matches!(
                    cfg_expr::Expression::parse(&pred.to_token_stream().to_string()),
                    Ok(expr)
                    if expr.eval(|pred| match pred {
                        cfg_expr::Predicate::Flag("cargo_equip") => Some(true),
                        _ => None,
                    }) == Some(true)
                ) && *attr == parse_quote!(cargo_equip::skip)
            )
        }))
}

pub(crate) fn indent_code(code: &str, n: usize) -> String {
    let is_safe_to_indent = code.parse::<TokenStream>().map_or(false, |token_stream| {
        !token_stream.into_iter().any(|tt| {
            matches!(
                tt, TokenTree::Literal(lit)
                if lit.span().start().line != lit.span().end().line
            )
        })
    });

    if is_safe_to_indent {
        code.lines()
            .map(|line| match line {
                "" => "\n".to_owned(),
                line => format!("{}{}\n", "    ".repeat(n), line),
            })
            .join("")
    } else {
        code.to_owned()
    }
}

fn replace_ranges(code: &str, replacements: BTreeMap<(LineColumn, LineColumn), String>) -> String {
    if replacements.is_empty() {
        return code.to_owned();
    }
    let replacements = replacements.into_iter().collect::<Vec<_>>();
    let mut replacements = &*replacements;
    let mut skip_until = None;
    let mut ret = "".to_owned();
    let mut lines = code.trim_end().split('\n').enumerate().peekable();
    while let Some((i, s)) = lines.next() {
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
        while let Some(((start, end), replacement)) = replacements.get(0) {
            if i == start.line - 1 {
                ret += replacement;
                if start < end {
                    skip_until = Some(*end);
                }
                replacements = &replacements[1..];
            } else {
                break;
            }
        }
        if lines.peek().is_some() || code.ends_with('\n') {
            ret += "\n";
        }
    }

    debug_assert!(syn::parse_file(code).is_ok());

    ret
}

pub(crate) fn insert_prelude_for_main_crate(
    code: &str,
    cargo_equip_mod_name: &Ident,
) -> syn::Result<String> {
    let file = &syn::parse_file(code)?;
    let mut replacements = btreemap!();
    Visitor {
        replacements: &mut replacements,
        cargo_equip_mod_name,
    }
    .visit_file(file);
    return Ok(replace_ranges(code, replacements));

    struct Visitor<'a> {
        replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
        cargo_equip_mod_name: &'a Ident,
    }

    impl Visitor<'_> {
        fn visit_items(&mut self, items: &[Item], crate_root: bool) {
            if let Some(first) = items.first() {
                let pos = first.span().start();
                self.replacements.insert(
                    (pos, pos),
                    format!(
                        "pub use {}{}::prelude::*;\n\n",
                        if crate_root { "" } else { "crate::" },
                        self.cargo_equip_mod_name,
                    ),
                );
            }
            for item in items {
                if let Item::Mod(item) = item {
                    self.visit_item_mod(item);
                }
            }
        }
    }

    impl Visit<'_> for Visitor<'_> {
        fn visit_file(&mut self, i: &syn::File) {
            self.visit_items(&i.items, true);
        }

        fn visit_item_mod(&mut self, i: &ItemMod) {
            if let Some((_, items)) = &i.content {
                self.visit_items(items, false);
            }
        }
    }
}

pub(crate) fn allow_unused_imports_for_seemingly_proc_macros(
    code: &str,
    mut seemingly_proc_macro: impl FnMut(&str, &str) -> bool,
) -> syn::Result<String> {
    let file = &syn::parse_file(code)?;
    let mut replacements = btreemap!();
    Visitor {
        replacements: &mut replacements,
        seemingly_proc_macro: &mut seemingly_proc_macro,
    }
    .visit_file(file);
    return Ok(if replacements.is_empty() {
        code.to_owned()
    } else {
        replace_ranges(code, replacements)
    });

    struct Visitor<'a, P> {
        replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
        seemingly_proc_macro: P,
    }

    impl<P: FnMut(&str, &str) -> bool> Visit<'_> for Visitor<'_, P> {
        fn visit_item_use(&mut self, i: &ItemUse) {
            if let UseTree::Path(UsePath { ident, tree, .. }) = &i.tree {
                match &**tree {
                    UseTree::Name(name)
                        if (self.seemingly_proc_macro)(
                            &ident.to_string(),
                            &name.ident.to_string(),
                        ) =>
                    {
                        self.replacements.insert(
                            (i.span().start(), i.span().start()),
                            "#[allow(unused_imports)]\n".to_owned(),
                        );
                    }
                    UseTree::Group(UseGroup { items, .. }) => {
                        for pair in items.pairs() {
                            let item = pair.value();
                            if let UseTree::Name(name) = item {
                                if (self.seemingly_proc_macro)(
                                    &ident.to_string(),
                                    &name.ident.to_string(),
                                ) {
                                    self.replacements.insert(
                                        (pair.span().start(), pair.span().start()),
                                        "/*".to_owned(),
                                    );
                                    self.replacements.insert(
                                        (pair.span().end(), pair.span().end()),
                                        "*/".to_owned(),
                                    );
                                    self.replacements.insert(
                                        (i.span().end(), i.span().end()),
                                        format!(
                                            "\n#[allow(unused_imports)]\n{}use {}::{};",
                                            (i.vis.to_token_stream().to_string() + " ").trim(),
                                            ident,
                                            name.ident,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn set_span(mask: &mut [FixedBitSet], span: Span, p: bool) {
    let i1 = span.start().line - 1;
    if span.start().line == span.end().line {
        let l = span.start().column;
        let r = span.end().column;
        mask[i1].set_range(l..r, p);
    } else {
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

pub(crate) fn parse_file(code: &str) -> anyhow::Result<syn::File> {
    syn::parse_file(code)
        .map_err(|e| anyhow!("{}", e))
        .with_context(|| "broke the code during modification")
}

pub(crate) fn process_bin<'cm>(
    cargo_equip_mod_name: &Ident,
    src_path: &Utf8Path,
    proc_macro_expander: Option<&mut ProcMacroExpander<'_>>,
    translate_extern_crate_name: impl FnMut(&str) -> Option<String>,
    is_lib_to_bundle: impl FnMut(&str) -> bool,
    shell: &mut Shell,
    context: impl FnOnce() -> (String, &'cm str),
) -> anyhow::Result<String> {
    let mut edit = CodeEdit::new(cargo_equip_mod_name, src_path, context)?;
    if let Some(proc_macro_expander) = proc_macro_expander {
        edit.expand_proc_macros(proc_macro_expander, shell)?;
    }
    edit.translate_extern_crate_paths(translate_extern_crate_name)?;
    edit.process_extern_crate_in_bin(is_lib_to_bundle)?;
    edit.finish()
}

pub(crate) struct CodeEdit<'opt> {
    cargo_equip_mod_name: &'opt Ident,
    has_local_inner_macros_attr: bool,
    string: String,
    file: syn::File,
    replacements: BTreeMap<(LineColumn, LineColumn), String>,
}

impl<'opt> CodeEdit<'opt> {
    pub(crate) fn new<'cm>(
        cargo_equip_mod_name: &'opt Ident,
        src_path: &Utf8Path,
        err_context: impl FnOnce() -> (String, &'cm str),
    ) -> anyhow::Result<Self> {
        return (|| {
            Self::from_code(cargo_equip_mod_name, &expand_mods(src_path, 0)?)
                .map_err(anyhow::Error::from)
        })()
        .with_context(|| {
            let (crate_name, package_id) = err_context();
            format!("could not expand `{}` from `{}`", crate_name, package_id)
        });

        fn expand_mods(src_path: &Utf8Path, depth: usize) -> anyhow::Result<String> {
            let content = cargo_util::paths::read(src_path.as_ref())?;

            let syn::File { items, .. } = syn::parse_file(&content)
                .map_err(|e| anyhow!("{:?}", e))
                .with_context(|| format!("could not parse `{}`", src_path))?;

            let replacements = items
                .into_iter()
                .flat_map(|item| match item {
                    Item::Mod(ItemMod {
                        attrs,
                        ident,
                        content: None,
                        semi,
                        ..
                    }) => Some((attrs, ident, semi)),
                    _ => None,
                })
                .map(|(attrs, ident, semi)| {
                    let paths = if let Some(path) = attrs
                        .iter()
                        .flat_map(Attribute::parse_meta)
                        .flat_map(|meta| match meta {
                            Meta::NameValue(name_value) => Some(name_value),
                            _ => None,
                        })
                        .filter(|MetaNameValue { path, .. }| {
                            matches!(path.get_ident(), Some(i) if i == "path")
                        })
                        .find_map(|MetaNameValue { lit, .. }| match lit {
                            Lit::Str(s) => Some(s.value()),
                            _ => None,
                        }) {
                        vec![src_path.with_file_name("").join(path)]
                    } else if depth == 0 || src_path.file_name() == Some("mod.rs") {
                        vec![
                            src_path
                                .with_file_name(&ident.to_string())
                                .with_extension("rs"),
                            src_path.with_file_name(&ident.to_string()).join("mod.rs"),
                        ]
                    } else {
                        vec![
                            src_path
                                .with_extension("")
                                .with_file_name(&ident.to_string())
                                .with_extension("rs"),
                            src_path
                                .with_extension("")
                                .with_file_name(&ident.to_string())
                                .join("mod.rs"),
                        ]
                    };

                    if let Some(path) = paths.iter().find(|p| p.exists()) {
                        let start = semi.span().start();
                        let end = semi.span().end();
                        let content = expand_mods(path, depth + 1)?;
                        let content = indent_code(&content, depth + 1);
                        let content = format!(" {{\n{}{}}}", content, "    ".repeat(depth + 1));
                        Ok(((start, end), content))
                    } else {
                        bail!("one of {:?} does not exist", paths);
                    }
                })
                .collect::<anyhow::Result<_>>()?;

            Ok(replace_ranges(&content, replacements))
        }
    }

    fn from_code(cargo_equip_mod_name: &'opt Ident, string: &str) -> syn::Result<Self> {
        let file = syn::parse_file(string)?;
        return Ok(Self {
            cargo_equip_mod_name,
            has_local_inner_macros_attr: check_local_inner_macros(&file),
            string: string.to_owned(),
            file,
            replacements: btreemap!(),
        });

        fn check_local_inner_macros(file: &syn::File) -> bool {
            let mut out = false;
            Visitor { out: &mut out }.visit_file(file);
            return out;

            struct Visitor<'a> {
                out: &'a mut bool,
            }

            impl Visit<'_> for Visitor<'_> {
                fn visit_item_macro(&mut self, i: &ItemMacro) {
                    *self.out |= i
                        .attrs
                        .iter()
                        .flat_map(Attribute::parse_meta)
                        .flat_map(|meta| match meta {
                            Meta::List(MetaList { path, nested, .. }) => Some((path, nested)),
                            _ => None,
                        })
                        .any(|(path, nested)| {
                            path.is_ident("macro_export")
                                && nested.iter().any(|meta| {
                                    matches!(
                                        meta,
                                        NestedMeta::Meta(Meta::Path(path))
                                        if path.is_ident("local_inner_macros")
                                    )
                                })
                        });
                }
            }
        }
    }

    pub(crate) fn has_local_inner_macros_attr(&self) -> bool {
        self.has_local_inner_macros_attr
    }

    pub(crate) fn finish(mut self) -> anyhow::Result<String> {
        self.apply()?;
        Ok(self.string)
    }

    fn apply(&mut self) -> anyhow::Result<()> {
        if !self.replacements.is_empty() {
            self.force_apply()?;
        }
        Ok(())
    }

    fn force_apply(&mut self) -> anyhow::Result<()> {
        self.string = replace_ranges(&self.string, mem::take(&mut self.replacements));
        self.file =
            syn::parse_file(&self.string).with_context(|| "broke the code during modification")?;
        Ok(())
    }

    fn process_extern_crate_in_bin(
        &mut self,
        is_lib_to_bundle: impl FnMut(&str) -> bool,
    ) -> anyhow::Result<()> {
        self.apply()?;
        Visitor {
            replacements: &mut self.replacements,
            cargo_equip_mod_name: self.cargo_equip_mod_name,
            is_lib_to_bundle,
        }
        .visit_file(&self.file);
        return Ok(());

        struct Visitor<'a, F> {
            replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
            cargo_equip_mod_name: &'a Ident,
            is_lib_to_bundle: F,
        }

        impl<F: FnMut(&str) -> bool> Visit<'_> for Visitor<'_, F> {
            fn visit_item_extern_crate(&mut self, item_use: &ItemExternCrate) {
                let ItemExternCrate {
                    attrs,
                    vis,
                    ident,
                    rename,
                    ..
                } = item_use;

                if (self.is_lib_to_bundle)(&ident.to_string()) {
                    let is_macro_use = attrs
                        .iter()
                        .flat_map(Attribute::parse_meta)
                        .any(|m| m.path().is_ident("macro_use"));
                    let vis = vis.to_token_stream();

                    let mut insertion = "".to_owned();

                    if let Some((_, rename)) = rename {
                        if rename != "_" {
                            insertion = format!(
                                "{} use crate::{}::crates::{} as {};",
                                vis, self.cargo_equip_mod_name, ident, rename
                            );
                        }
                    } else {
                        insertion = format!(
                            "{} use crate::{}::crates::{};",
                            vis, self.cargo_equip_mod_name, ident,
                        );
                    }

                    if is_macro_use {
                        insertion += &format!(
                            "{} use crate::{}::macros::{}::*;",
                            vis, self.cargo_equip_mod_name, ident,
                        );
                    }

                    let insertion = insertion.trim_start();

                    let pos = item_use.span().start();
                    self.replacements.insert((pos, pos), "/*".to_owned());
                    let pos = item_use.span().end();
                    self.replacements
                        .insert((pos, pos), "*/".to_owned() + insertion);
                }
            }
        }
    }

    pub(crate) fn process_extern_crates_in_lib(
        &mut self,
        convert_extern_crate_name: impl FnMut(&str) -> Option<String>,
        shell: &mut Shell,
    ) -> anyhow::Result<()> {
        self.apply()?;

        for item in &self.file.items {
            if let Item::ExternCrate(ItemExternCrate {
                vis,
                ident,
                rename: Some((_, rename)),
                ..
            }) = item
            {
                shell.warn(format!(
                    "declaring `extern crate .. as ..` in a root module is not recommended: \
                     `{} extern crate {} as {}`",
                    vis.to_token_stream(),
                    ident,
                    rename,
                ))?;
            }
        }

        Visitor {
            replacements: &mut self.replacements,
            cargo_equip_mod_name: self.cargo_equip_mod_name,
            convert_extern_crate_name,
        }
        .visit_file(&self.file);
        return Ok(());

        struct Visitor<'a, F> {
            replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
            cargo_equip_mod_name: &'a Ident,
            convert_extern_crate_name: F,
        }

        impl<F: FnMut(&str) -> Option<String>> Visit<'_> for Visitor<'_, F> {
            fn visit_item_extern_crate(&mut self, item_use: &ItemExternCrate) {
                let ItemExternCrate {
                    attrs,
                    vis,
                    ident,
                    rename,
                    semi_token,
                    ..
                } = item_use;

                if let Some(to) = (self.convert_extern_crate_name)(&ident.to_string()) {
                    let to = Ident::new(&to, Span::call_site());
                    let Self {
                        cargo_equip_mod_name,
                        ..
                    } = self;
                    self.replacements.insert(
                        (item_use.span().start(), semi_token.span().end()),
                        if let Some((_, rename)) = rename {
                            quote!(
                                #(#attrs)* #vis use crate::#cargo_equip_mod_name::crates::#to as #rename;
                            )
                            .to_string()
                        } else {
                            quote!(
                                #(#attrs)* #vis use crate::#cargo_equip_mod_name::crates::#to as #ident;
                            )
                            .to_string()
                        },
                    );
                }
            }
        }
    }

    pub(crate) fn expand_proc_macros(
        &mut self,
        expander: &mut ProcMacroExpander<'_>,
        shell: &mut Shell,
    ) -> anyhow::Result<()> {
        self.apply()?;

        loop {
            self.force_apply()?;

            let code_lines = &self.string.split('\n').collect::<Vec<_>>();

            let mut output = Ok(None);
            AttributeMacroVisitor {
                expander,
                output: &mut output,
                shell,
            }
            .visit_file(&self.file);

            if let Some((span, expansion)) = output? {
                let end = to_index(code_lines, span.end());
                let start = to_index(code_lines, span.start());
                self.string
                    .insert_str(end, &format!("*/{}", minify_group(expansion)));
                self.string.insert_str(start, "/*");

                continue;
            }

            let mut output = Ok(None);
            DeriveMacroVisitor {
                expander,
                output: &mut output,
                shell,
            }
            .visit_file(&self.file);

            if let Some((expansion, item_span, macro_path_span, comma_span)) = output? {
                let insert_at = to_index(code_lines, item_span.end());
                let comma_end = comma_span.map(|comma_end| to_index(code_lines, comma_end));
                let path_range = to_range(code_lines, macro_path_span);

                self.string.insert_str(insert_at, &minify_group(expansion));
                let end = if let Some(comma_end) = comma_end {
                    comma_end
                } else {
                    path_range.end
                };
                self.string.insert_str(end, "*/");
                self.string.insert_str(path_range.start, "/*");

                continue;
            }

            let mut output = Ok(None);
            FunctionLikeMacroVisitor {
                expander,
                output: &mut output,
                shell,
            }
            .visit_file(&self.file);

            if let Some((span, expansion)) = output? {
                let i1 = to_index(code_lines, span.end());
                let i2 = to_index(code_lines, span.start());
                self.string
                    .insert_str(i1, &format!("*/{}", minify_group(expansion)));
                self.string.insert_str(i2, "/*");
                continue;
            }

            return Ok(());
        }

        struct AttributeMacroVisitor<'a, 'msg> {
            expander: &'a mut ProcMacroExpander<'msg>,
            output: &'a mut anyhow::Result<Option<(Span, proc_macro2::Group)>>,
            shell: &'a mut Shell,
        }

        impl AttributeMacroVisitor<'_, '_> {
            fn visit_item_with_attrs<'a, T: ToTokens + Clone + 'a>(
                &mut self,
                i: &'a T,
                attrs: &[Attribute],
                remove_attr: fn(&mut T, usize) -> Attribute,
                visit: fn(&mut Self, &'a T),
            ) {
                if !matches!(self.output, Ok(None)) {
                    return;
                }

                if let Some(result) = attrs
                    .iter()
                    .enumerate()
                    .filter(|(_, Attribute { style, .. })| *style == AttrStyle::Outer)
                    .find_map(|(nth, attr)| {
                        let Self {
                            expander, shell, ..
                        } = self;
                        let macro_name = attr.path.get_ident()?.to_string();
                        expander
                            .expand_attr_macro(
                                &macro_name,
                                || {
                                    let i = &mut i.clone();
                                    remove_attr(i, nth);
                                    i.to_token_stream()
                                },
                                || {
                                    proc_macro2::Group::new(
                                        proc_macro2::Delimiter::None,
                                        syn::parse2::<proc_macro2::Group>(attr.tokens.clone())
                                            .map(|attr| attr.stream())
                                            .unwrap_or_default(),
                                    )
                                },
                                |msg| {
                                    shell.warn(format!("error from RA: {}", msg))?;
                                    Ok(())
                                },
                            )
                            .transpose()
                    })
                {
                    *self.output = match result {
                        Ok(expansion) => Ok(Some((i.span(), expansion))),
                        Err(err) => Err(err),
                    };
                } else {
                    visit(self, i);
                }
            }
        }

        macro_rules! impl_visits {
            ($(fn $method:ident(&mut self, _: &'_ $ty:path) { _(_, _, _, $visit:path) })*) => {
                $(
                    fn $method(&mut self, i: &'_ $ty) {
                        self.visit_item_with_attrs(i, &i.attrs, |i, nth| i.attrs.remove(nth), $visit)
                    }
                )*
            };
        }

        impl Visit<'_> for AttributeMacroVisitor<'_, '_> {
            impl_visits! {
                fn visit_item_const       (&mut self, _: &'_ ItemConst      ) { _(_, _, _, visit::visit_item_const       ) }
                fn visit_item_enum        (&mut self, _: &'_ ItemEnum       ) { _(_, _, _, visit::visit_item_enum        ) }
                fn visit_item_extern_crate(&mut self, _: &'_ ItemExternCrate) { _(_, _, _, visit::visit_item_extern_crate) }
                fn visit_item_fn          (&mut self, _: &'_ ItemFn         ) { _(_, _, _, visit::visit_item_fn          ) }
                fn visit_item_foreign_mod (&mut self, _: &'_ ItemForeignMod ) { _(_, _, _, visit::visit_item_foreign_mod ) }
                fn visit_item_impl        (&mut self, _: &'_ ItemImpl       ) { _(_, _, _, visit::visit_item_impl        ) }
                fn visit_item_macro       (&mut self, _: &'_ ItemMacro      ) { _(_, _, _, visit::visit_item_macro       ) }
                fn visit_item_macro2      (&mut self, _: &'_ ItemMacro2     ) { _(_, _, _, visit::visit_item_macro2      ) }
                fn visit_item_mod         (&mut self, _: &'_ ItemMod        ) { _(_, _, _, visit::visit_item_mod         ) }
                fn visit_item_static      (&mut self, _: &'_ ItemStatic     ) { _(_, _, _, visit::visit_item_static      ) }
                fn visit_item_struct      (&mut self, _: &'_ ItemStruct     ) { _(_, _, _, visit::visit_item_struct      ) }
                fn visit_item_trait       (&mut self, _: &'_ ItemTrait      ) { _(_, _, _, visit::visit_item_trait       ) }
                fn visit_item_trait_alias (&mut self, _: &'_ ItemTraitAlias ) { _(_, _, _, visit::visit_item_trait_alias ) }
                fn visit_item_type        (&mut self, _: &'_ ItemType       ) { _(_, _, _, visit::visit_item_type        ) }
                fn visit_item_union       (&mut self, _: &'_ ItemUnion      ) { _(_, _, _, visit::visit_item_union       ) }
                fn visit_item_use         (&mut self, _: &'_ ItemUse        ) { _(_, _, _, visit::visit_item_use         ) }
            }
        }

        #[allow(clippy::type_complexity)]
        struct DeriveMacroVisitor<'a, 'msg> {
            expander: &'a mut ProcMacroExpander<'msg>,
            output: &'a mut anyhow::Result<
                Option<(proc_macro2::Group, Span, Span, Option<LineColumn>)>,
            >,
            shell: &'a mut Shell,
        }

        impl DeriveMacroVisitor<'_, '_> {
            fn visit_struct_enum_union(&mut self, i: impl ToTokens, attrs: &[Attribute]) {
                if !matches!(self.output, Ok(None)) {
                    return;
                }

                if let Some(result) = attrs
                    .iter()
                    .flat_map(Attribute::parse_meta)
                    .flat_map(|meta| match meta {
                        Meta::List(list_meta) => Some(list_meta),
                        _ => None,
                    })
                    .filter(|MetaList { path, .. }| path.is_ident("derive"))
                    .flat_map(|MetaList { nested, .. }| nested.into_pairs())
                    .flat_map(|pair| {
                        fn get_ident(nested_meta: &NestedMeta) -> Option<String> {
                            if let NestedMeta::Meta(Meta::Path(path)) = nested_meta {
                                path.get_ident().map(ToString::to_string)
                            } else {
                                None
                            }
                        }

                        match pair {
                            Pair::Punctuated(m, p) => {
                                Some((get_ident(&m)?, m.span(), Some(p.span().end())))
                            }
                            Pair::End(m) => Some((get_ident(&m)?, m.span(), None)),
                        }
                    })
                    .find_map(|(macro_name, path_span, comma_end)| {
                        let Self {
                            expander, shell, ..
                        } = self;
                        expander
                            .expand_derive_macro(
                                &macro_name,
                                || i.to_token_stream(),
                                |msg| {
                                    shell.warn(format!("error from RA: {}", msg))?;
                                    Ok(())
                                },
                            )
                            .transpose()
                            .map(move |expansion| {
                                expansion.map(move |expansion| (expansion, path_span, comma_end))
                            })
                    })
                {
                    *self.output = match result {
                        Ok((expansion, path_span, comma_end)) => {
                            Ok(Some((expansion, i.span(), path_span, comma_end)))
                        }
                        Err(err) => Err(err),
                    };
                }
            }
        }

        impl Visit<'_> for DeriveMacroVisitor<'_, '_> {
            fn visit_item_struct(&mut self, i: &'_ ItemStruct) {
                self.visit_struct_enum_union(i, &i.attrs);
            }

            fn visit_item_enum(&mut self, i: &'_ ItemEnum) {
                self.visit_struct_enum_union(i, &i.attrs);
            }

            fn visit_item_union(&mut self, i: &'_ ItemUnion) {
                self.visit_struct_enum_union(i, &i.attrs);
            }
        }

        struct FunctionLikeMacroVisitor<'a, 'msg> {
            expander: &'a mut ProcMacroExpander<'msg>,
            output: &'a mut anyhow::Result<Option<(Span, proc_macro2::Group)>>,
            shell: &'a mut Shell,
        }

        impl Visit<'_> for FunctionLikeMacroVisitor<'_, '_> {
            fn visit_item_macro(&mut self, i: &'_ ItemMacro) {
                if i.ident.is_none() {
                    self.visit_macro(&i.mac);
                }
            }

            fn visit_macro(&mut self, i: &'_ Macro) {
                if !matches!(self.output, Ok(None)) {
                    return;
                }

                if let Some(macro_name) = i.path.get_ident() {
                    let Self {
                        expander, shell, ..
                    } = self;
                    let expansion = expander.expand_func_like_macro(
                        &macro_name.to_string(),
                        || i.tokens.clone(),
                        |msg| {
                            shell.warn(format!("error from RA: {}", msg))?;
                            Ok(())
                        },
                    );

                    *self.output = match expansion {
                        Ok(Some(expansion)) => Ok(Some((i.span(), expansion))),
                        Ok(None) => Ok(None),
                        Err(err) => Err(err),
                    };
                }
            }
        }

        fn to_range(lines: &[&str], span: Span) -> Range<usize> {
            to_index(lines, span.start())..to_index(lines, span.end())
        }

        fn to_index(lines: &[&str], loc: LineColumn) -> usize {
            lines[..loc.line - 1]
                .iter()
                .map(|s| s.len() + 1)
                .sum::<usize>()
                + lines[loc.line - 1]
                    .char_indices()
                    .nth(loc.column)
                    .map(|(i, _)| i)
                    .unwrap_or_else(|| lines[loc.line - 1].len())
        }

        fn minify_group(group: proc_macro2::Group) -> String {
            rustminify::minify_tokens(TokenTree::from(group).into())
        }
    }

    pub(crate) fn expand_includes(&mut self, out_dir: &Utf8Path) -> anyhow::Result<()> {
        self.apply()?;
        Visitor {
            out_dir,
            replacements: &mut self.replacements,
        }
        .visit_file(&self.file);
        return Ok(());

        struct Visitor<'a> {
            out_dir: &'a Utf8Path,
            replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
        }

        impl Visitor<'_> {
            fn resolve(&self, expr: &Expr) -> Option<String> {
                if let Expr::Macro(ExprMacro {
                    mac: Macro { path, tokens, .. },
                    ..
                }) = expr
                {
                    if [parse_quote!(::core::concat), parse_quote!(::std::concat)].contains(path) {
                        (|parse_stream: ParseStream<'_>| {
                            Punctuated::<Expr, Token![,]>::parse_separated_nonempty(parse_stream)
                        })
                        .parse2(tokens.clone())
                        .ok()?
                        .iter()
                        .map(|expr| self.resolve(expr))
                        .collect()
                    } else if [parse_quote!(::core::env), parse_quote!(::std::env)].contains(path) {
                        let name = syn::parse2::<LitStr>(tokens.clone()).ok()?.value();
                        if name == "OUT_DIR" {
                            Some(self.out_dir.as_str().to_owned())
                        } else {
                            env::var(name).ok()
                        }
                    } else {
                        None
                    }
                } else if let Expr::Lit(ExprLit {
                    lit: Lit::Str(lit_str),
                    ..
                }) = expr
                {
                    Some(lit_str.value())
                } else {
                    None
                }
            }
        }

        impl Visit<'_> for Visitor<'_> {
            fn visit_item_macro(&mut self, i: &ItemMacro) {
                if i.ident.is_none()
                    && [parse_quote!(::core::include), parse_quote!(::std::include)]
                        .contains(&i.mac.path)
                {
                    if let Ok(expr) = syn::parse2(i.mac.tokens.clone()) {
                        if let Some(path) = self.resolve(&expr) {
                            let path = Utf8PathBuf::from(path);
                            if path.is_absolute() {
                                if let Ok(content) = cargo_util::paths::read(path.as_ref()) {
                                    self.replacements
                                        .insert((i.span().start(), i.span().end()), content);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn translate_extern_crate_paths(
        &mut self,
        translate_extern_crate_name: impl FnMut(&str) -> Option<String>,
    ) -> anyhow::Result<()> {
        self.apply()?;
        Visitor {
            replacements: &mut self.replacements,
            cargo_equip_mod_name: self.cargo_equip_mod_name,
            translate_extern_crate_name,
        }
        .visit_file(&self.file);
        return Ok(());

        struct Visitor<'a, F> {
            replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
            cargo_equip_mod_name: &'a Ident,
            translate_extern_crate_name: F,
        }

        impl<F: FnMut(&str) -> Option<String>> Visitor<'_, F> {
            fn attempt_translate(&mut self, leading_colon: Span, extern_crate_name: &Ident) {
                if let Some(pseudo_extern_crate_name) =
                    (self.translate_extern_crate_name)(&extern_crate_name.to_string())
                {
                    self.replacements.insert(
                        (leading_colon.start(), leading_colon.end()),
                        format!("/*::*/crate::{}::crates::", self.cargo_equip_mod_name),
                    );

                    if extern_crate_name != &*pseudo_extern_crate_name {
                        let span = extern_crate_name.span();
                        self.replacements.insert(
                            (span.start(), span.end()),
                            format!("/*{}*/{}", extern_crate_name, pseudo_extern_crate_name),
                        );
                    }
                }
            }
        }

        impl<F: FnMut(&str) -> Option<String>> Visit<'_> for Visitor<'_, F> {
            fn visit_item_use(&mut self, i: &'_ ItemUse) {
                if let Some(leading_colon) = i.leading_colon {
                    for extern_crate_name in extract_first_segments(&i.tree) {
                        self.attempt_translate(leading_colon.span(), extern_crate_name);
                    }
                }

                fn extract_first_segments(tree: &UseTree) -> Vec<&Ident> {
                    match tree {
                        UseTree::Path(UsePath { ident, .. })
                        | UseTree::Name(UseName { ident })
                        | UseTree::Rename(UseRename { ident, .. }) => {
                            vec![ident]
                        }
                        UseTree::Glob(_) => vec![],
                        UseTree::Group(UseGroup { items, .. }) => {
                            items.iter().flat_map(extract_first_segments).collect()
                        }
                    }
                }
            }

            fn visit_path(&mut self, i: &'_ syn::Path) {
                if let Some(leading_colon) = i.leading_colon {
                    let PathSegment { ident, .. } = i
                        .segments
                        .last()
                        .expect("`syn::Path::segments` is considered not to be empty");
                    self.attempt_translate(leading_colon.span(), ident);
                }
            }
        }
    }

    pub(crate) fn translate_crate_path(&mut self, extern_crate_name: &str) -> anyhow::Result<()> {
        self.apply()?;
        Visitor {
            extern_crate_name,
            cargo_equip_mod_name: self.cargo_equip_mod_name,
            replacements: &mut self.replacements,
        }
        .visit_file(&self.file);
        return Ok(());

        struct Visitor<'a> {
            extern_crate_name: &'a str,
            cargo_equip_mod_name: &'a Ident,
            replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
        }

        impl Visitor<'_> {
            fn insert(&mut self, crate_token: &Ident) {
                let pos = crate_token.span().end();
                self.replacements.insert(
                    (pos, pos),
                    format!(
                        "::{}::crates::{}",
                        self.cargo_equip_mod_name, self.extern_crate_name,
                    ),
                );
            }
        }

        impl Visit<'_> for Visitor<'_> {
            fn visit_path(&mut self, path: &'_ syn::Path) {
                if let (None, Some(first)) = (path.leading_colon, path.segments.first()) {
                    if first.ident == "crate" && first.arguments.is_empty() {
                        self.insert(&first.ident);
                    }
                }
            }

            fn visit_item_use(&mut self, item_use: &'_ ItemUse) {
                if item_use.leading_colon.is_none() {
                    self.visit_use_tree(&item_use.tree);
                }
            }

            fn visit_use_tree(&mut self, use_tree: &UseTree) {
                match &use_tree {
                    UseTree::Path(UsePath { ident, .. })
                    | UseTree::Name(UseName { ident })
                    | UseTree::Rename(UseRename { ident, .. })
                        if ident == "crate" =>
                    {
                        self.insert(ident);
                    }
                    UseTree::Group(UseGroup { items, .. }) => {
                        for item in items {
                            self.visit_use_tree(item);
                        }
                    }
                    _ => {}
                }
            }

            fn visit_vis_restricted(&mut self, vis_restricted: &VisRestricted) {
                if vis_restricted.in_token.is_some() {
                    self.visit_path(&vis_restricted.path);
                } else if let Some(ident) = vis_restricted.path.get_ident() {
                    if ident == "crate" {
                        let pos = vis_restricted.path.span().start();
                        self.replacements.insert((pos, pos), "in ".to_owned());

                        self.insert(ident);
                    }
                }
            }
        }
    }

    pub(crate) fn modify_declarative_macros(
        &mut self,
        pseudo_extern_crate_name: &str,
    ) -> anyhow::Result<String> {
        self.apply()?;

        let mut macro_names = btreemap!();

        for item_macro in collect_item_macros(&self.file) {
            if let ItemMacro {
                attrs,
                ident: Some(ident),
                mac: Macro { tokens, .. },
                ..
            } = item_macro
            {
                replace_dollar_crates(
                    tokens.clone(),
                    self.cargo_equip_mod_name,
                    pseudo_extern_crate_name,
                    &mut self.replacements,
                );
                if attrs
                    .iter()
                    .flat_map(Attribute::parse_meta)
                    .any(|m| m.path().is_ident("macro_export"))
                {
                    let rename = format!(
                        "{}_macro_def_{}_{}",
                        self.cargo_equip_mod_name, pseudo_extern_crate_name, ident,
                    );
                    self.replacements.insert(
                        (ident.span().start(), ident.span().end()),
                        format!("/*{}*/{}", ident, rename),
                    );
                    let pos = item_macro.span().end();
                    self.replacements.insert(
                        (pos, pos),
                        format!(
                            "\nmacro_rules!{}{{($($tt:tt)*)=>(crate::{}!{{$($tt)*}})}}",
                            ident, rename,
                        ),
                    );
                    macro_names.insert(rename, ident);
                }
            }
        }

        if !macro_names.is_empty() {
            if let Some(first) = self.file.items.first() {
                let pos = first.span().start();
                self.replacements.entry((pos, pos)).or_default().insert_str(
                    0,
                    &format!(
                        "pub use crate::{}::macros::{}::*;",
                        self.cargo_equip_mod_name, pseudo_extern_crate_name,
                    ),
                );
            }
        }

        let macro_mod_content = if macro_names.is_empty() {
            "".to_owned()
        } else {
            format!(
                "pub use crate::{}{}{};\n",
                if macro_names.len() > 1 { "{" } else { "" },
                macro_names
                    .iter()
                    .map(|(rename, name)| if *name == rename {
                        name.to_string()
                    } else {
                        format!("{} as {}", rename, name)
                    })
                    .format(", "),
                if macro_names.len() > 1 { "}" } else { "" },
            )
        };

        return Ok(macro_mod_content);

        fn collect_item_macros(file: &syn::File) -> Vec<&ItemMacro> {
            let mut acc = vec![];
            Visitor { acc: &mut acc }.visit_file(file);
            return acc;

            struct Visitor<'a, 'b> {
                acc: &'b mut Vec<&'a ItemMacro>,
            }

            impl<'a, 'b> Visit<'a> for Visitor<'a, 'b> {
                fn visit_item_macro(&mut self, i: &'a ItemMacro) {
                    self.acc.push(i);
                }
            }
        }

        fn replace_dollar_crates(
            token_stream: TokenStream,
            cargo_equip_mod_name: &Ident,
            pseudo_extern_crate_name: &str,
            acc: &mut BTreeMap<(LineColumn, LineColumn), String>,
        ) {
            let mut token_stream = token_stream.into_iter().peekable();

            if let Some(proc_macro2::TokenTree::Group(group)) = token_stream.peek() {
                replace_dollar_crates(
                    group.stream(),
                    cargo_equip_mod_name,
                    pseudo_extern_crate_name,
                    acc,
                );
            }

            for (tt1, tt2) in token_stream.tuple_windows() {
                if let proc_macro2::TokenTree::Group(group) = &tt2 {
                    replace_dollar_crates(
                        group.stream(),
                        cargo_equip_mod_name,
                        pseudo_extern_crate_name,
                        acc,
                    );
                }

                if matches!(
                    (&tt1, &tt2),
                    (proc_macro2::TokenTree::Punct(p), proc_macro2::TokenTree::Ident(i))
                    if p.as_char() == '$' && i == "crate"
                ) {
                    let pos = tt2.span().end();
                    acc.insert(
                        (pos, pos),
                        format!(
                            "::{}::crates::{}",
                            cargo_equip_mod_name, pseudo_extern_crate_name,
                        ),
                    );
                }
            }
        }
    }

    pub(crate) fn resolve_pseudo_prelude(
        &mut self,
        pseudo_extern_crate_name: &str,
        libs_with_local_inner_macros: &BTreeSet<&str>,
        extern_crate_name_translation: &BTreeMap<String, String>,
    ) -> anyhow::Result<String> {
        if extern_crate_name_translation.is_empty() && libs_with_local_inner_macros.is_empty() {
            return Ok("".to_owned());
        }

        self.apply()?;

        let syn::File { attrs, items, .. } = &self.file;

        let external_local_inner_macros = {
            let macros = libs_with_local_inner_macros
                .iter()
                .map(|name| format!("{}::*", name))
                .join(", ");
            match libs_with_local_inner_macros.len() {
                0 => None,
                1 => Some(macros),
                _ => Some(format!("{{{}}}", macros)),
            }
        };

        let pseudo_extern_crates = {
            let uses = extern_crate_name_translation
                .iter()
                .map(|(extern_crate_name, pseudo_extern_crate_name)| {
                    if extern_crate_name == pseudo_extern_crate_name {
                        extern_crate_name.clone()
                    } else {
                        format!("{} as {}", pseudo_extern_crate_name, extern_crate_name)
                    }
                })
                .join(", ");
            match extern_crate_name_translation.len() {
                0 => None,
                1 => Some(uses),
                _ => Some(format!("{{{}}}", uses)),
            }
        };

        let mut prelude = "".to_owned();
        if let Some(external_local_inner_macros) = &external_local_inner_macros {
            prelude += &format!(
                "pub(in crate::{0}) use crate::{0}::macros::{1};",
                self.cargo_equip_mod_name, external_local_inner_macros,
            );
        }
        if let Some(pseudo_extern_crates) = &pseudo_extern_crates {
            prelude += &format!(
                "pub(in crate::{0}) use crate::{0}::crates::{1};",
                self.cargo_equip_mod_name, pseudo_extern_crates,
            );
        }

        self.replacements.insert(
            {
                let pos = if let Some(item) = items.first() {
                    item.span().start()
                } else if let Some(attr) = attrs.last() {
                    attr.span().end()
                } else {
                    LineColumn { line: 0, column: 0 }
                };
                (pos, pos)
            },
            {
                format!(
                    "use crate::{}::preludes::{}::*;",
                    self.cargo_equip_mod_name, pseudo_extern_crate_name,
                )
            },
        );

        let mut queue = items
            .iter()
            .flat_map(|item| match item {
                Item::Mod(item_mod) => Some((1, item_mod)),
                _ => None,
            })
            .collect::<VecDeque<_>>();

        while let Some((depth, ItemMod { attrs, content, .. })) = queue.pop_front() {
            let (_, items) = content.as_ref().expect("should be expanded");
            let pos = if let Some(item) = items.first() {
                item.span().start()
            } else if let Some(attr) = attrs.last() {
                attr.span().end()
            } else {
                LineColumn { line: 0, column: 0 }
            };
            self.replacements.insert(
                (pos, pos),
                format!(
                    "use crate::{}::preludes::{}::*;",
                    self.cargo_equip_mod_name, pseudo_extern_crate_name,
                ),
            );
            for item in items {
                if let Item::Mod(item_mod) = item {
                    queue.push_back((depth + 1, item_mod));
                }
            }
        }
        Ok(prelude)
    }

    pub(crate) fn resolve_cfgs(&mut self, features: &[String]) -> anyhow::Result<()> {
        self.apply()?;
        Visitor {
            replacements: &mut self.replacements,
            features,
        }
        .visit_file(&self.file);
        return Ok(());

        struct Visitor<'a> {
            replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
            features: &'a [String],
        }

        impl Visitor<'_> {
            fn proceed<'a, T: ToTokens>(
                &mut self,
                i: &'a T,
                attrs: fn(&T) -> &[Attribute],
                visit: fn(&mut Self, &'a T),
            ) {
                let sufficiencies = attrs(i)
                    .iter()
                    .flat_map(|a| a.parse_meta().map(|m| (a.span(), m)))
                    .flat_map(|(span, meta)| match meta {
                        Meta::List(meta_list) => Some((span, meta_list)),
                        _ => None,
                    })
                    .filter(|(_, MetaList { path, .. })| path.is_ident("cfg"))
                    .flat_map(|(span, MetaList { nested, .. })| {
                        let expr =
                            cfg_expr::Expression::parse(&nested.to_token_stream().to_string())
                                .ok()?;
                        Some((span, expr))
                    })
                    .map(|(span, expr)| {
                        let sufficiency = expr.eval(|pred| match pred {
                            cfg_expr::Predicate::Test | cfg_expr::Predicate::ProcMacro => {
                                Some(false)
                            }
                            cfg_expr::Predicate::Flag("cargo_equip") => Some(true),
                            cfg_expr::Predicate::Feature(feature) => {
                                Some(self.features.contains(&(*feature).to_owned()))
                            }
                            _ => None,
                        });
                        (span, sufficiency)
                    })
                    .collect::<Vec<_>>();

                if sufficiencies.iter().any(|&(_, p)| p == Some(false)) {
                    self.replacements
                        .insert((i.span().start(), i.span().end()), "".to_owned());
                } else {
                    for (span, p) in sufficiencies {
                        if p == Some(true) {
                            self.replacements
                                .insert((span.start(), span.end()), "".to_owned());
                        }
                    }
                    visit(self, i);
                }
            }
        }

        macro_rules! impl_visits {
            ($(fn $method:ident(&mut self, _: &'_ $ty:path) { _(_, _, $visit:path) })*) => {
                $(
                    fn $method(&mut self, i: &'_ $ty) {
                        self.proceed(i, |$ty { attrs, .. }| attrs, $visit);
                    }
                )*
            };
        }

        impl Visit<'_> for Visitor<'_> {
            impl_visits! {
                fn visit_arm                (&mut self, _: &'_ Arm              ) { _(_, _, visit::visit_arm                ) }
                fn visit_bare_fn_arg        (&mut self, _: &'_ BareFnArg        ) { _(_, _, visit::visit_bare_fn_arg        ) }
                fn visit_const_param        (&mut self, _: &'_ ConstParam       ) { _(_, _, visit::visit_const_param        ) }
                fn visit_expr_array         (&mut self, _: &'_ ExprArray        ) { _(_, _, visit::visit_expr_array         ) }
                fn visit_expr_assign        (&mut self, _: &'_ ExprAssign       ) { _(_, _, visit::visit_expr_assign        ) }
                fn visit_expr_assign_op     (&mut self, _: &'_ ExprAssignOp     ) { _(_, _, visit::visit_expr_assign_op     ) }
                fn visit_expr_async         (&mut self, _: &'_ ExprAsync        ) { _(_, _, visit::visit_expr_async         ) }
                fn visit_expr_await         (&mut self, _: &'_ ExprAwait        ) { _(_, _, visit::visit_expr_await         ) }
                fn visit_expr_binary        (&mut self, _: &'_ ExprBinary       ) { _(_, _, visit::visit_expr_binary        ) }
                fn visit_expr_block         (&mut self, _: &'_ ExprBlock        ) { _(_, _, visit::visit_expr_block         ) }
                fn visit_expr_box           (&mut self, _: &'_ ExprBox          ) { _(_, _, visit::visit_expr_box           ) }
                fn visit_expr_break         (&mut self, _: &'_ ExprBreak        ) { _(_, _, visit::visit_expr_break         ) }
                fn visit_expr_call          (&mut self, _: &'_ ExprCall         ) { _(_, _, visit::visit_expr_call          ) }
                fn visit_expr_cast          (&mut self, _: &'_ ExprCast         ) { _(_, _, visit::visit_expr_cast          ) }
                fn visit_expr_closure       (&mut self, _: &'_ ExprClosure      ) { _(_, _, visit::visit_expr_closure       ) }
                fn visit_expr_continue      (&mut self, _: &'_ ExprContinue     ) { _(_, _, visit::visit_expr_continue      ) }
                fn visit_expr_field         (&mut self, _: &'_ ExprField        ) { _(_, _, visit::visit_expr_field         ) }
                fn visit_expr_for_loop      (&mut self, _: &'_ ExprForLoop      ) { _(_, _, visit::visit_expr_for_loop      ) }
                fn visit_expr_group         (&mut self, _: &'_ ExprGroup        ) { _(_, _, visit::visit_expr_group         ) }
                fn visit_expr_if            (&mut self, _: &'_ ExprIf           ) { _(_, _, visit::visit_expr_if            ) }
                fn visit_expr_index         (&mut self, _: &'_ ExprIndex        ) { _(_, _, visit::visit_expr_index         ) }
                fn visit_expr_let           (&mut self, _: &'_ ExprLet          ) { _(_, _, visit::visit_expr_let           ) }
                fn visit_expr_lit           (&mut self, _: &'_ ExprLit          ) { _(_, _, visit::visit_expr_lit           ) }
                fn visit_expr_loop          (&mut self, _: &'_ ExprLoop         ) { _(_, _, visit::visit_expr_loop          ) }
                fn visit_expr_macro         (&mut self, _: &'_ ExprMacro        ) { _(_, _, visit::visit_expr_macro         ) }
                fn visit_expr_match         (&mut self, _: &'_ ExprMatch        ) { _(_, _, visit::visit_expr_match         ) }
                fn visit_expr_method_call   (&mut self, _: &'_ ExprMethodCall   ) { _(_, _, visit::visit_expr_method_call   ) }
                fn visit_expr_paren         (&mut self, _: &'_ ExprParen        ) { _(_, _, visit::visit_expr_paren         ) }
                fn visit_expr_path          (&mut self, _: &'_ ExprPath         ) { _(_, _, visit::visit_expr_path          ) }
                fn visit_expr_range         (&mut self, _: &'_ ExprRange        ) { _(_, _, visit::visit_expr_range         ) }
                fn visit_expr_reference     (&mut self, _: &'_ ExprReference    ) { _(_, _, visit::visit_expr_reference     ) }
                fn visit_expr_repeat        (&mut self, _: &'_ ExprRepeat       ) { _(_, _, visit::visit_expr_repeat        ) }
                fn visit_expr_return        (&mut self, _: &'_ ExprReturn       ) { _(_, _, visit::visit_expr_return        ) }
                fn visit_expr_struct        (&mut self, _: &'_ ExprStruct       ) { _(_, _, visit::visit_expr_struct        ) }
                fn visit_expr_try           (&mut self, _: &'_ ExprTry          ) { _(_, _, visit::visit_expr_try           ) }
                fn visit_expr_try_block     (&mut self, _: &'_ ExprTryBlock     ) { _(_, _, visit::visit_expr_try_block     ) }
                fn visit_expr_tuple         (&mut self, _: &'_ ExprTuple        ) { _(_, _, visit::visit_expr_tuple         ) }
                fn visit_expr_type          (&mut self, _: &'_ ExprType         ) { _(_, _, visit::visit_expr_type          ) }
                fn visit_expr_unary         (&mut self, _: &'_ ExprUnary        ) { _(_, _, visit::visit_expr_unary         ) }
                fn visit_expr_unsafe        (&mut self, _: &'_ ExprUnsafe       ) { _(_, _, visit::visit_expr_unsafe        ) }
                fn visit_expr_while         (&mut self, _: &'_ ExprWhile        ) { _(_, _, visit::visit_expr_while         ) }
                fn visit_expr_yield         (&mut self, _: &'_ ExprYield        ) { _(_, _, visit::visit_expr_yield         ) }
                fn visit_field              (&mut self, _: &'_ Field            ) { _(_, _, visit::visit_field              ) }
                fn visit_field_pat          (&mut self, _: &'_ FieldPat         ) { _(_, _, visit::visit_field_pat          ) }
                fn visit_field_value        (&mut self, _: &'_ FieldValue       ) { _(_, _, visit::visit_field_value        ) }
                fn visit_file               (&mut self, _: &'_ syn::File        ) { _(_, _, visit::visit_file               ) }
                fn visit_foreign_item_fn    (&mut self, _: &'_ ForeignItemFn    ) { _(_, _, visit::visit_foreign_item_fn    ) }
                fn visit_foreign_item_macro (&mut self, _: &'_ ForeignItemMacro ) { _(_, _, visit::visit_foreign_item_macro ) }
                fn visit_foreign_item_static(&mut self, _: &'_ ForeignItemStatic) { _(_, _, visit::visit_foreign_item_static) }
                fn visit_foreign_item_type  (&mut self, _: &'_ ForeignItemType  ) { _(_, _, visit::visit_foreign_item_type  ) }
                fn visit_impl_item_const    (&mut self, _: &'_ ImplItemConst    ) { _(_, _, visit::visit_impl_item_const    ) }
                fn visit_impl_item_macro    (&mut self, _: &'_ ImplItemMacro    ) { _(_, _, visit::visit_impl_item_macro    ) }
                fn visit_impl_item_method   (&mut self, _: &'_ ImplItemMethod   ) { _(_, _, visit::visit_impl_item_method   ) }
                fn visit_impl_item_type     (&mut self, _: &'_ ImplItemType     ) { _(_, _, visit::visit_impl_item_type     ) }
                fn visit_item_const         (&mut self, _: &'_ ItemConst        ) { _(_, _, visit::visit_item_const         ) }
                fn visit_item_enum          (&mut self, _: &'_ ItemEnum         ) { _(_, _, visit::visit_item_enum          ) }
                fn visit_item_extern_crate  (&mut self, _: &'_ ItemExternCrate  ) { _(_, _, visit::visit_item_extern_crate  ) }
                fn visit_item_fn            (&mut self, _: &'_ ItemFn           ) { _(_, _, visit::visit_item_fn            ) }
                fn visit_item_foreign_mod   (&mut self, _: &'_ ItemForeignMod   ) { _(_, _, visit::visit_item_foreign_mod   ) }
                fn visit_item_impl          (&mut self, _: &'_ ItemImpl         ) { _(_, _, visit::visit_item_impl          ) }
                fn visit_item_macro         (&mut self, _: &'_ ItemMacro        ) { _(_, _, visit::visit_item_macro         ) }
                fn visit_item_macro2        (&mut self, _: &'_ ItemMacro2       ) { _(_, _, visit::visit_item_macro2        ) }
                fn visit_item_mod           (&mut self, _: &'_ ItemMod          ) { _(_, _, visit::visit_item_mod           ) }
                fn visit_item_static        (&mut self, _: &'_ ItemStatic       ) { _(_, _, visit::visit_item_static        ) }
                fn visit_item_struct        (&mut self, _: &'_ ItemStruct       ) { _(_, _, visit::visit_item_struct        ) }
                fn visit_item_trait         (&mut self, _: &'_ ItemTrait        ) { _(_, _, visit::visit_item_trait         ) }
                fn visit_item_trait_alias   (&mut self, _: &'_ ItemTraitAlias   ) { _(_, _, visit::visit_item_trait_alias   ) }
                fn visit_item_type          (&mut self, _: &'_ ItemType         ) { _(_, _, visit::visit_item_type          ) }
                fn visit_item_union         (&mut self, _: &'_ ItemUnion        ) { _(_, _, visit::visit_item_union         ) }
                fn visit_item_use           (&mut self, _: &'_ ItemUse          ) { _(_, _, visit::visit_item_use           ) }
                fn visit_lifetime_def       (&mut self, _: &'_ LifetimeDef      ) { _(_, _, visit::visit_lifetime_def       ) }
                fn visit_local              (&mut self, _: &'_ Local            ) { _(_, _, visit::visit_local              ) }
                fn visit_pat_box            (&mut self, _: &'_ PatBox           ) { _(_, _, visit::visit_pat_box            ) }
                fn visit_pat_ident          (&mut self, _: &'_ PatIdent         ) { _(_, _, visit::visit_pat_ident          ) }
                fn visit_pat_lit            (&mut self, _: &'_ PatLit           ) { _(_, _, visit::visit_pat_lit            ) }
                fn visit_pat_macro          (&mut self, _: &'_ PatMacro         ) { _(_, _, visit::visit_pat_macro          ) }
                fn visit_pat_or             (&mut self, _: &'_ PatOr            ) { _(_, _, visit::visit_pat_or             ) }
                fn visit_pat_path           (&mut self, _: &'_ PatPath          ) { _(_, _, visit::visit_pat_path           ) }
                fn visit_pat_range          (&mut self, _: &'_ PatRange         ) { _(_, _, visit::visit_pat_range          ) }
                fn visit_pat_reference      (&mut self, _: &'_ PatReference     ) { _(_, _, visit::visit_pat_reference      ) }
                fn visit_pat_rest           (&mut self, _: &'_ PatRest          ) { _(_, _, visit::visit_pat_rest           ) }
                fn visit_pat_slice          (&mut self, _: &'_ PatSlice         ) { _(_, _, visit::visit_pat_slice          ) }
                fn visit_pat_struct         (&mut self, _: &'_ PatStruct        ) { _(_, _, visit::visit_pat_struct         ) }
                fn visit_pat_tuple          (&mut self, _: &'_ PatTuple         ) { _(_, _, visit::visit_pat_tuple          ) }
                fn visit_pat_tuple_struct   (&mut self, _: &'_ PatTupleStruct   ) { _(_, _, visit::visit_pat_tuple_struct   ) }
                fn visit_pat_type           (&mut self, _: &'_ PatType          ) { _(_, _, visit::visit_pat_type           ) }
                fn visit_pat_wild           (&mut self, _: &'_ PatWild          ) { _(_, _, visit::visit_pat_wild           ) }
                fn visit_receiver           (&mut self, _: &'_ Receiver         ) { _(_, _, visit::visit_receiver           ) }
                fn visit_trait_item_const   (&mut self, _: &'_ TraitItemConst   ) { _(_, _, visit::visit_trait_item_const   ) }
                fn visit_trait_item_macro   (&mut self, _: &'_ TraitItemMacro   ) { _(_, _, visit::visit_trait_item_macro   ) }
                fn visit_trait_item_method  (&mut self, _: &'_ TraitItemMethod  ) { _(_, _, visit::visit_trait_item_method  ) }
                fn visit_trait_item_type    (&mut self, _: &'_ TraitItemType    ) { _(_, _, visit::visit_trait_item_type    ) }
                fn visit_type_param         (&mut self, _: &'_ TypeParam        ) { _(_, _, visit::visit_type_param         ) }
                fn visit_variadic           (&mut self, _: &'_ Variadic         ) { _(_, _, visit::visit_variadic           ) }
                fn visit_variant            (&mut self, _: &'_ Variant          ) { _(_, _, visit::visit_variant            ) }
            }
        }
    }

    pub(crate) fn allow_missing_docs(&mut self) {
        Visitor {
            replacements: &mut self.replacements,
        }
        .visit_file(&self.file);

        struct Visitor<'a> {
            replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
        }

        impl Visit<'_> for Visitor<'_> {
            fn visit_attribute(&mut self, i: &Attribute) {
                if let Ok(Meta::List(MetaList { path, nested, .. })) = i.parse_meta() {
                    if ["warn", "deny", "forbid"]
                        .iter()
                        .any(|lint| path.is_ident(lint))
                    {
                        for meta in nested {
                            if let NestedMeta::Meta(Meta::Path(path)) = meta {
                                if ["missing_docs", "missing_crate_level_docs"]
                                    .iter()
                                    .any(|lint| path.is_ident(lint))
                                {
                                    let pos = path.span().start();
                                    self.replacements.insert((pos, pos), "/*".to_owned());
                                    let pos = path.span().end();
                                    self.replacements.insert((pos, pos), "*/".to_owned());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn erase_docs(&mut self) -> anyhow::Result<()> {
        return self.erase(
            |mask, token_stream| syn::parse2(token_stream).map(|f| Visitor(mask).visit_file(&f)),
            || "broke the code during erasing doc comments",
        );

        struct Visitor<'a>(&'a mut [FixedBitSet]);

        impl Visit<'_> for Visitor<'_> {
            fn visit_attribute(&mut self, attr: &'_ Attribute) {
                if matches!(attr.parse_meta(), Ok(m) if m.path().is_ident("doc")) {
                    set_span(self.0, attr.span(), true);
                }
            }
        }
    }

    pub(crate) fn erase_comments(&mut self) -> anyhow::Result<()> {
        return self.erase(
            |mask, token_stream| {
                for mask in &mut *mask {
                    mask.insert_range(..);
                }
                visit_token_stream(mask, token_stream);
                Ok(())
            },
            || "broke the code during erasing comments",
        );

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
    }

    fn erase(
        &mut self,
        visit_file: fn(&mut [FixedBitSet], TokenStream) -> syn::Result<()>,
        err_msg: fn() -> &'static str,
    ) -> anyhow::Result<()> {
        self.apply()?;

        let code = &if self.string.contains("\r\n") {
            Cow::from(self.string.replace("\r\n", "\n"))
        } else {
            Cow::from(&self.string)
        };

        let code = if code.starts_with("#!") {
            let (_, code) = code.split_at(code.find('\n').unwrap_or_else(|| code.len()));
            code
        } else {
            code
        };

        let token_stream = code
            .parse::<TokenStream>()
            .map_err(|e| anyhow!("{}", e))
            .with_context(|| "broke the code during modification")?;

        let mut erase = code
            .lines()
            .map(|l| FixedBitSet::with_capacity(l.chars().count()))
            .collect::<Vec<_>>();

        visit_file(&mut erase, token_stream)
            .map_err(|e| anyhow!("{:?}", e))
            .with_context(err_msg)?;

        let mut acc = "".to_owned();
        for (line, erase) in code.lines().zip_eq(erase) {
            for (j, ch) in line.chars().enumerate() {
                acc.push(if erase[j] { ' ' } else { ch });
            }
            acc += "\n";
        }
        self.string = acc.trim_start().to_owned();
        self.apply()
    }
}

#[cfg(test)]
mod tests {
    use crate::rust::CodeEdit;
    use pretty_assertions::assert_eq;
    use proc_macro2::Span;
    use syn::Ident;

    thread_local! {
        static DUMMY_MOD_NAME: Ident = Ident::new("__", Span::call_site());
    }

    #[test]
    fn erase_docs() -> anyhow::Result<()> {
        fn test(input: &str, expected: &str) -> anyhow::Result<()> {
            DUMMY_MOD_NAME.with(|dummy_mod_name| {
                let mut edit = CodeEdit::from_code(dummy_mod_name, input)?;
                edit.erase_docs()?;
                assert_eq!(expected, edit.finish()?);
                Ok(())
            })
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
            DUMMY_MOD_NAME.with(|dummy_mod_name| {
                let mut edit = CodeEdit::from_code(dummy_mod_name, input)?;
                edit.erase_comments()?;
                assert_eq!(expected, edit.finish()?);
                Ok(())
            })
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
