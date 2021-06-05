use crate::{ra_proc_macro::ProcMacroExpander, shell::Shell};
use anyhow::{anyhow, bail, Context as _};
use camino::{Utf8Path, Utf8PathBuf};
use fixedbitset::FixedBitSet;
use if_chain::if_chain;
use indoc::formatdoc;
use itertools::Itertools as _;
use maplit::btreemap;
use proc_macro2::{LineColumn, Spacing, Span, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, VecDeque},
    convert::Infallible,
    env, fs, mem,
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
    ItemTraitAlias, ItemType, ItemUnion, ItemUse, LifetimeDef, Lit, LitStr, Local, Macro,
    MacroDelimiter, Meta, MetaList, MetaNameValue, NestedMeta, PatBox, PatIdent, PatLit, PatMacro,
    PatOr, PatPath, PatRange, PatReference, PatRest, PatSlice, PatStruct, PatTuple, PatTupleStruct,
    PatType, PatWild, PathSegment, Receiver, Token, TraitItemConst, TraitItemMacro,
    TraitItemMethod, TraitItemType, TypeParam, UseGroup, UseName, UsePath, UseRename, UseTree,
    Variadic, Variant, VisRestricted,
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

pub(crate) fn translate_abs_paths(
    code: &str,
    translate_extern_crate_name: impl FnMut(&str) -> Option<String>,
) -> anyhow::Result<String> {
    let file = &syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;

    let mut replacements = btreemap!();
    Visitor {
        replacements: &mut replacements,
        translate_extern_crate_name,
    }
    .visit_file(file);

    return Ok(if replacements.is_empty() {
        code.to_owned()
    } else {
        replace_ranges(code, replacements)
    });

    struct Visitor<'a, F> {
        replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
        translate_extern_crate_name: F,
    }

    impl<F: FnMut(&str) -> Option<String>> Visitor<'_, F> {
        fn attempt_translate(&mut self, leading_colon: Span, extern_crate_name: &Ident) {
            if let Some(pseudo_extern_crate_name) =
                (self.translate_extern_crate_name)(&extern_crate_name.to_string())
            {
                self.replacements.insert(
                    (leading_colon.start(), leading_colon.end()),
                    "/*::*/crate::".to_owned(),
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

pub(crate) fn process_extern_crate_in_bin(
    code: &str,
    is_lib_to_bundle: impl FnMut(&str) -> bool,
) -> anyhow::Result<String> {
    struct Visitor<'a, F> {
        replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
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
                let uses = match (rename, is_macro_use) {
                    (Some((_, rename)), false) if rename == "_" => "".to_owned(),
                    (Some((_, rename)), false) => format!("{} as {}", ident, rename),
                    (None, false) => ident.to_string(),
                    (Some((_, rename)), true) if rename == "_" => format!("{}::__macros::*", ident),
                    (Some((_, rename)), true) => {
                        format!("{}::{{self as {},__macros::*}}", ident, rename)
                    }
                    (None, true) => format!("{}::{{self,__macros::*}}", ident),
                };
                if uses.is_empty() {
                    return;
                }
                let insertion = format!("{} use crate::{};", vis, uses);
                let insertion = insertion.trim_start();

                let pos = item_use.span().start();
                self.replacements.insert((pos, pos), "/*".to_owned());
                let pos = item_use.span().end();
                self.replacements
                    .insert((pos, pos), "*/".to_owned() + insertion);
            }
        }
    }

    let file = syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;

    let mut replacements = btreemap!();

    Visitor {
        replacements: &mut replacements,
        is_lib_to_bundle,
    }
    .visit_file(&file);

    Ok(replace_ranges(code, replacements))
}

pub(crate) fn expand_mods(src_path: &Utf8Path) -> anyhow::Result<String> {
    fn expand_mods(src_path: &Utf8Path, depth: usize) -> anyhow::Result<String> {
        let content = std::fs::read_to_string(src_path)
            .with_context(|| format!("could not read `{}`", src_path))?;

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
                    let content = expand_mods(&path, depth + 1)?;
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

    expand_mods(src_path, 0)
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

pub(crate) fn expand_proc_macros(
    code: &str,
    expander: &mut ProcMacroExpander<'_>,
    shell: &mut Shell,
) -> anyhow::Result<String> {
    let mut code = code.to_owned();

    loop {
        let code_lines = &code.split('\n').collect::<Vec<_>>();

        let file = syn::parse_file(&code)
            .map_err(|e| anyhow!("{:?}", e))
            .with_context(|| "could not parse the code")?;

        let mut output = Ok(None);
        AttributeMacroVisitor {
            expander,
            output: &mut output,
            shell,
        }
        .visit_file(&file);

        if let Some((span, expansion)) = output? {
            let end = to_index(code_lines, span.end());
            let start = to_index(code_lines, span.start());
            code.insert_str(end, &format!("*/{}", minify_group(expansion)));
            code.insert_str(start, "/*");

            continue;
        }

        let mut output = Ok(None);
        DeriveMacroVisitor {
            expander,
            output: &mut output,
            shell,
        }
        .visit_file(&file);

        if let Some((expansion, item_span, macro_path_span, comma_span)) = output? {
            let insert_at = to_index(code_lines, item_span.end());
            let comma_end = comma_span.map(|comma_end| to_index(code_lines, comma_end));
            let path_range = to_range(code_lines, macro_path_span);

            code.insert_str(insert_at, &minify_group(expansion));
            let end = if let Some(comma_end) = comma_end {
                comma_end
            } else {
                path_range.end
            };
            code.insert_str(end, "*/");
            code.insert_str(path_range.start, "/*");

            continue;
        }

        let mut output = Ok(None);
        FunctionLikeMacroVisitor {
            expander,
            output: &mut output,
            shell,
        }
        .visit_file(&file);

        if let Some((span, expansion)) = output? {
            let i1 = to_index(code_lines, span.end());
            let i2 = to_index(code_lines, span.start());
            code.insert_str(i1, &format!("*/{}", minify_group(expansion)));
            code.insert_str(i2, "/*");
            continue;
        }

        return Ok(code);
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
                                syn::parse2(attr.tokens.clone()).unwrap_or_else(|_| {
                                    proc_macro2::Group::new(
                                        proc_macro2::Delimiter::None,
                                        attr.tokens.clone(),
                                    )
                                })
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
        output:
            &'a mut anyhow::Result<Option<(proc_macro2::Group, Span, Span, Option<LineColumn>)>>,
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
}

pub(crate) fn expand_includes(code: &str, out_dir: &Utf8Path) -> anyhow::Result<String> {
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
                        Punctuated::<Expr, Token![,]>::parse_separated_nonempty(&parse_stream)
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
                            if let Ok(content) = fs::read_to_string(path) {
                                self.replacements
                                    .insert((i.span().start(), i.span().end()), content);
                            }
                        }
                    }
                }
            }
        }
    }

    let file = syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;

    let mut replacements = btreemap!();
    Visitor {
        replacements: &mut replacements,
        out_dir,
    }
    .visit_file(&file);

    Ok(replace_ranges(code, replacements))
}

pub(crate) fn check_local_inner_macros(code: &str) -> anyhow::Result<bool> {
    let file = &syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;

    let mut out = false;
    Visitor { out: &mut out }.visit_file(file);
    return Ok(out);

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

pub(crate) fn replace_crate_paths(
    code: &str,
    extern_crate_name: &str,
    shell: &mut Shell,
) -> anyhow::Result<String> {
    struct Visitor<'a> {
        extern_crate_name: &'a str,
        replacements: BTreeMap<(LineColumn, LineColumn), String>,
    }

    impl Visitor<'_> {
        fn insert(&mut self, crate_token: &Ident) {
            let pos = crate_token.span().end();
            self.replacements
                .insert((pos, pos), format!("::{}", self.extern_crate_name));
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

    let file = syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;

    let mut visitor = Visitor {
        extern_crate_name,
        replacements: btreemap!(),
    };

    visitor.visit_file(&file);

    let Visitor { replacements, .. } = visitor;

    if replacements.is_empty() {
        Ok(code.to_owned())
    } else {
        shell.warn(format!(
            "found `crate` paths. replacing them with `crate::{}`",
            extern_crate_name,
        ))?;
        Ok(replace_ranges(code, replacements))
    }
}

pub(crate) fn process_extern_crates_in_lib(
    shell: &mut Shell,
    code: &str,
    convert_extern_crate_name: impl FnMut(&str) -> Option<String>,
) -> anyhow::Result<String> {
    struct Visitor<'a, F> {
        replacements: &'a mut BTreeMap<(LineColumn, LineColumn), String>,
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
                self.replacements.insert(
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

    for item in &file.items {
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

    let mut replacements = btreemap!();

    Visitor {
        replacements: &mut replacements,
        convert_extern_crate_name,
    }
    .visit_file(&file);

    Ok(replace_ranges(code, replacements))
}

pub(crate) fn modify_declarative_macros(
    code: &str,
    pseudo_extern_crate_name: &str,
    remove_docs: bool,
) -> anyhow::Result<(String, BTreeMap<String, String>)> {
    let file = &syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;

    let mut contents = btreemap!();
    let mut replacements = btreemap!();

    for item_macro in collect_item_macros(file) {
        if let ItemMacro {
            attrs,
            ident: Some(ident),
            mac: Macro { tokens, .. },
            ..
        } = item_macro
        {
            if attrs
                .iter()
                .flat_map(Attribute::parse_meta)
                .any(|m| m.path().is_ident("macro_export"))
            {
                let (rename, content) = take(
                    item_macro,
                    ident,
                    pseudo_extern_crate_name,
                    remove_docs,
                    &mut replacements,
                );
                contents.insert(rename, (ident.to_string(), content));
            } else {
                replace_dollar_crates(tokens.clone(), pseudo_extern_crate_name, &mut replacements);
            }
        }
    }

    if let Some(first) = file.items.first() {
        let pos = first.span().start();
        replacements.entry((pos, pos)).or_default().insert_str(
            0,
            &if contents.is_empty() {
                "pub mod __macros {}".to_owned()
            } else {
                formatdoc! {r#"
                    pub mod __macros {{
                        pub use crate::{}{}{};
                    }}
                    pub use self::__macros::*;
                    "#,
                    if contents.len() > 1 { "{" } else { "" },
                    contents
                        .iter()
                        .map(|(rename, (name, _))| if rename == name {
                            name.clone()
                        } else {
                            format!("{} as {}", rename, name)
                        })
                        .format(", "),
                    if contents.len() > 1 { "}" } else { "" },
                }
            },
        );
    }

    return Ok((
        replace_ranges(code, replacements),
        contents.into_iter().map(|(_, v)| v).collect(),
    ));

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

    fn take(
        item: &ItemMacro,
        item_ident: &syn::Ident,
        pseudo_extern_crate_name: &str,
        remove_docs: bool,
        replacements: &mut BTreeMap<(LineColumn, LineColumn), String>,
    ) -> (String, String) {
        let ItemMacro {
            attrs,
            ident,
            mac:
                Macro {
                    path,
                    bang_token,
                    tokens,
                    delimiter,
                },
            semi_token,
        } = item;

        debug_assert!(ident.is_some() && path.is_ident("macro_rules"));

        let pos = item.span().start();
        replacements.insert((pos, pos), "/*".to_owned());
        let pos = item.span().end();
        replacements.insert((pos, pos), "*/".to_owned());

        let name = format!("__macro_def_{}_{}", pseudo_extern_crate_name, item_ident);

        let name_as_ident = proc_macro2::Ident::new(&name, Span::call_site());

        let body = proc_macro2::Group::new(
            match delimiter {
                MacroDelimiter::Paren(_) => proc_macro2::Delimiter::Parenthesis,
                MacroDelimiter::Brace(_) => proc_macro2::Delimiter::Brace,
                MacroDelimiter::Bracket(_) => proc_macro2::Delimiter::Bracket,
            },
            take(
                tokens.clone(),
                &proc_macro2::Ident::new(pseudo_extern_crate_name, Span::call_site()),
            ),
        );

        let attrs = attrs
            .iter()
            .filter(|a| {
                !(remove_docs && matches!(a.parse_meta(), Ok(m) if m.path().is_ident("doc")))
            })
            .map(|a| {
                if matches!(a.parse_meta(), Ok(m) if m.path().is_ident("macro_export")) {
                    quote!(#[macro_export])
                } else {
                    a.to_token_stream()
                }
            });

        return (
            name,
            minify_token_stream::<_, Infallible>(
                quote!(#[cfg_attr(any(), rustfmt::skip)] #(#attrs)* #path#bang_token #name_as_ident #body #semi_token),
                |o| Ok(syn::parse_str::<ItemMacro>(o).is_ok()),
            )
            .unwrap(),
        );

        fn take(tokens: TokenStream, pseudo_extern_crate_name: &syn::Ident) -> TokenStream {
            let mut out = vec![];
            for tt in tokens {
                if let TokenTree::Group(group) = &tt {
                    out.push(
                        proc_macro2::Group::new(
                            group.delimiter(),
                            take(group.stream(), pseudo_extern_crate_name),
                        )
                        .into(),
                    );
                } else {
                    out.push(tt);
                    if let [.., TokenTree::Punct(punct), TokenTree::Ident(ident)] = &*out {
                        if punct.as_char() == '$' && ident == "crate" {
                            out.extend(quote!(::#pseudo_extern_crate_name));
                        }
                    }
                }
            }
            out.into_iter().collect()
        }
    }

    fn replace_dollar_crates(
        token_stream: TokenStream,
        pseudo_extern_crate_name: &str,
        acc: &mut BTreeMap<(LineColumn, LineColumn), String>,
    ) {
        let mut token_stream = token_stream.into_iter().peekable();

        if let Some(proc_macro2::TokenTree::Group(group)) = token_stream.peek() {
            replace_dollar_crates(group.stream(), pseudo_extern_crate_name, acc);
        }

        for (tt1, tt2) in token_stream.tuple_windows() {
            if let proc_macro2::TokenTree::Group(group) = &tt2 {
                replace_dollar_crates(group.stream(), pseudo_extern_crate_name, acc);
            }

            if matches!(
                (&tt1, &tt2),
                (proc_macro2::TokenTree::Punct(p), proc_macro2::TokenTree::Ident(i))
                if p.as_char() == '$' && i == "crate"
            ) {
                let pos = tt2.span().end();
                acc.insert((pos, pos), format!("::{}", pseudo_extern_crate_name));
            }
        }
    }
}

pub(crate) fn insert_pseudo_preludes(
    code: &str,
    libs_with_local_inner_macros: &BTreeSet<&str>,
    extern_crate_name_translation: &BTreeMap<String, String>,
) -> anyhow::Result<String> {
    if extern_crate_name_translation.is_empty() && libs_with_local_inner_macros.is_empty() {
        return Ok(code.to_owned());
    }

    let syn::File { attrs, items, .. } = syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;

    let external_local_inner_macros = {
        let macros = libs_with_local_inner_macros
            .iter()
            .map(|name| format!("{}::__macros::*", name))
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

    let modules = {
        let mut modules = "".to_owned();
        if let Some(external_local_inner_macros) = &external_local_inner_macros {
            modules += &formatdoc! {r"
                mod __external_local_inner_macros {{
                    pub(super) use crate::{};
                }}
                ",
                external_local_inner_macros,
            };
        }
        if let Some(pseudo_extern_crates) = &pseudo_extern_crates {
            modules += &formatdoc! {r"
                mod __pseudo_extern_prelude {{
                    pub(super) use crate::{};
                }}
                ",
                pseudo_extern_crates,
            };
        }
        modules
    };

    let uses = match (external_local_inner_macros, pseudo_extern_crates) {
        (Some(_), Some(_)) => "{__external_local_inner_macros::*, __pseudo_extern_prelude::*}",
        (Some(_), None) => "__external_local_inner_macros::*",
        (None, Some(_)) => "__pseudo_extern_prelude::*",
        (None, None) => unreachable!(),
    };

    let mut replacements = btreemap!(
        {
            let pos = if let Some(item) = items.first() {
                item.span().start()
            } else if let Some(attr) = attrs.last() {
                attr.span().end()
            } else {
                LineColumn { line: 0, column: 0 }
            };
            (pos, pos)
        } => formatdoc!{r"
            {}
            use self::{};

            ",
            modules, uses,
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
        replacements.insert(
            (pos, pos),
            format!("use {}{};\n\n", "super::".repeat(depth), uses),
        );
        for item in items {
            if let Item::Mod(item_mod) = item {
                queue.push_back((depth + 1, item_mod));
            }
        }
    }

    Ok(replace_ranges(code, replacements))
}

fn replace_ranges(code: &str, replacements: BTreeMap<(LineColumn, LineColumn), String>) -> String {
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

pub(crate) fn resolve_cfgs(code: &str, features: &[String]) -> anyhow::Result<String> {
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
                        cfg_expr::Expression::parse(&nested.to_token_stream().to_string()).ok()?;
                    Some((span, expr))
                })
                .map(|(span, expr)| {
                    let sufficiency = expr.eval(|pred| match pred {
                        cfg_expr::Predicate::Test | cfg_expr::Predicate::ProcMacro => Some(false),
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

    let file = syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;

    let mut replacements = btreemap!();

    Visitor {
        replacements: &mut replacements,
        features,
    }
    .visit_file(&file);

    Ok(replace_ranges(code, replacements))
}

pub(crate) fn allow_missing_docs(code: &str) -> anyhow::Result<String> {
    let file = syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?;
    let mut replacements = btreemap!();
    Visitor {
        replacements: &mut replacements,
    }
    .visit_file(&file);
    return Ok(if replacements.is_empty() {
        code.to_owned()
    } else {
        replace_ranges(code, replacements)
    });

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
    return erase(code, |mask, token_stream| {
        for mask in &mut *mask {
            mask.insert_range(..);
        }
        visit_token_stream(mask, token_stream);
        Ok(())
    });

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
    code: &str,
    visit_file: fn(&mut [FixedBitSet], TokenStream) -> syn::Result<()>,
) -> anyhow::Result<String> {
    let code = &if code.contains("\r\n") {
        Cow::from(code.replace("\r\n", "\n"))
    } else {
        Cow::from(code)
    };

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

pub(crate) fn minify_file(
    code: &str,
    check: impl FnOnce(&str) -> anyhow::Result<bool>,
) -> anyhow::Result<String> {
    if syn::parse_file(code).is_err() {
        println!("{}", code);
        todo!();
    }

    let tokens = syn::parse_file(code)
        .map_err(|e| anyhow!("{:?}", e))
        .with_context(|| "could not parse the code")?
        .into_token_stream();
    minify_token_stream(tokens, check)
}

fn minify_group(group: proc_macro2::Group) -> String {
    minify_token_stream(TokenTree::from(group).into(), |_| Ok::<_, Infallible>(true)).unwrap()
}

fn minify_token_stream<F: FnOnce(&str) -> Result<bool, E>, E>(
    tokens: TokenStream,
    check: F,
) -> Result<String, E> {
    let safe = tokens.to_string();

    let mut acc = "".to_owned();
    minify(&mut acc, tokens);

    return if check(&acc)? { Ok(acc) } else { Ok(safe) };

    fn minify(acc: &mut String, token_stream: TokenStream) {
        #[derive(PartialEq)]
        enum State {
            None,
            AlnumUnderscoreQuote,
            PunctChars(String, Spacing),
        }

        let mut prev = State::None;
        for tt in token_stream {
            match tt {
                TokenTree::Group(group) => {
                    if let State::PunctChars(puncts, _) = mem::replace(&mut prev, State::None) {
                        *acc += &puncts;
                    }
                    let (left, right) = match group.delimiter() {
                        proc_macro2::Delimiter::Parenthesis => ('(', ')'),
                        proc_macro2::Delimiter::Brace => ('{', '}'),
                        proc_macro2::Delimiter::Bracket => ('[', ']'),
                        proc_macro2::Delimiter::None => (' ', ' '),
                    };
                    acc.push(left);
                    minify(acc, group.stream());
                    acc.push(right);
                    prev = State::None;
                }
                TokenTree::Ident(ident) => {
                    match mem::replace(&mut prev, State::AlnumUnderscoreQuote) {
                        State::AlnumUnderscoreQuote => *acc += " ",
                        State::PunctChars(puncts, _) => *acc += &puncts,
                        _ => {}
                    }
                    *acc += &ident.to_string();
                }
                TokenTree::Literal(literal) => {
                    let literal = literal.to_string();
                    let (literal, next) = if let Some(literal) = literal.strip_suffix('.') {
                        (literal, State::PunctChars(".".to_owned(), Spacing::Alone))
                    } else {
                        (&*literal, State::AlnumUnderscoreQuote)
                    };
                    match mem::replace(&mut prev, next) {
                        State::AlnumUnderscoreQuote => *acc += " ",
                        State::PunctChars(puncts, _) => *acc += &puncts,
                        _ => {}
                    }
                    *acc += &literal.to_string();
                }
                TokenTree::Punct(punct) => {
                    if let State::PunctChars(puncts, spacing) = &mut prev {
                        if *spacing == Spacing::Alone {
                            *acc += puncts;
                            // https://docs.rs/syn/1.0.46/syn/token/index.html
                            if [
                                ("!", '='),
                                ("%", '='),
                                ("&", '&'),
                                ("&", '='),
                                ("*", '='),
                                ("+", '='),
                                ("-", '='),
                                ("-", '>'),
                                (".", '.'),
                                ("..", '.'),
                                ("..", '='),
                                ("/", '='),
                                (":", ':'),
                                ("<", '-'),
                                ("<", '<'),
                                ("<", '='),
                                ("<<", '='),
                                ("=", '='),
                                ("=", '>'),
                                (">", '='),
                                (">", '>'),
                                (">>", '='),
                                ("^", '='),
                                ("|", '='),
                                ("|", '|'),
                            ]
                            .contains(&(&&*puncts, punct.as_char()))
                            {
                                *acc += " ";
                            }
                            prev = State::PunctChars(punct.as_char().to_string(), punct.spacing());
                        } else {
                            puncts.push(punct.as_char());
                            *spacing = punct.spacing();
                        }
                    } else {
                        prev = State::PunctChars(punct.as_char().to_string(), punct.spacing());
                    }
                }
            }
        }
        if let State::PunctChars(puncts, _) = prev {
            *acc += &puncts;
        }
    }
}

#[cfg(test)]
mod tests {
    use difference::assert_diff;
    use proc_macro2::TokenStream;
    use quote::quote;
    use std::convert::Infallible;
    use test_case::test_case;

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

    #[test_case(quote!(a + *b)                                => "a+*b"                           ; "joint_add_deref"       )]
    #[test_case(quote!(a + !b)                                => "a+!b"                           ; "joint_add_not"         )]
    #[test_case(quote!(a + -b)                                => "a+-b"                           ; "joint_add_neg"         )]
    #[test_case(quote!(a + &b)                                => "a+&b"                           ; "joint_add_reference"   )]
    #[test_case(quote!(a && &b)                               => "a&&&b"                          ; "joint_andand_reference")]
    #[test_case(quote!(a & &b)                                => "a& &b"                          ; "space_and_reference"   )]
    #[test_case(quote!(a < -b)                                => "a< -b"                          ; "space_le_neg"          )]
    #[test_case(quote!(0. ..1.)                               => "0. ..1."                        ; "space_dec_point_range" )]
    #[test_case(quote!(println!("{}", 2 * 2 + 1))             => r#"println!("{}",2*2+1)"#        ; "println"               )]
    #[test_case(quote!(macro_rules! m { ($($_:tt)*) => {}; }) => "macro_rules!m{($($_:tt)*)=>{};}"; "macro_rules"           )]
    fn minify_token_stream(tokens: TokenStream) -> String {
        crate::rust::minify_token_stream::<_, Infallible>(tokens, |o| {
            Ok(o.parse::<TokenStream>().is_ok())
        })
        .unwrap()
    }
}
