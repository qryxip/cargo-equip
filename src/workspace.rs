use crate::shell::Shell;
use anyhow::{anyhow, bail, Context as _};
use cargo_metadata as cm;
use if_chain::if_chain;
use indoc::indoc;
use itertools::Itertools as _;
use krates::PkgSpec;
use proc_macro2::TokenStream;
use quote::quote;
use rand::Rng as _;
use serde::{
    de::{Deserializer, Error as _},
    Deserialize,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env,
    io::Cursor,
    mem,
    path::{Path, PathBuf},
    str,
};

pub(crate) fn locate_project(cwd: &Path) -> anyhow::Result<PathBuf> {
    cwd.ancestors()
        .map(|p| p.join("Cargo.toml"))
        .find(|p| p.exists())
        .with_context(|| {
            format!(
                "could not find `Cargo.toml` in `{}` or any parent directory",
                cwd.display(),
            )
        })
}

pub(crate) fn cargo_metadata(manifest_path: &Path, cwd: &Path) -> cm::Result<cm::Metadata> {
    cm::MetadataCommand::new()
        .manifest_path(manifest_path)
        .current_dir(cwd)
        .exec()
}

pub(crate) fn execute_build_scripts<'cm>(
    metadata: &'cm cm::Metadata,
    bin_package: &'cm cm::Package,
    bin_target: &'cm cm::Target,
    shell: &mut Shell,
) -> anyhow::Result<BTreeMap<&'cm cm::PackageId, PathBuf>> {
    let cargo_exe = crate::process::cargo_exe()?;

    let messages = crate::process::process(&cargo_exe)
        .arg("check")
        .arg("--message-format")
        .arg("json")
        .arg("-p")
        .arg(format!("{}:{}", bin_package.name, bin_package.version))
        .arg("--bin")
        .arg(&bin_target.name)
        .cwd(&metadata.workspace_root)
        .read_with_status(true, shell)?;

    // TODO: check if ≧ 1.41.0

    let messages =
        cm::Message::parse_stream(Cursor::new(messages)).collect::<Result<Vec<_>, _>>()?;

    Ok(messages
        .into_iter()
        .flat_map(|message| match message {
            cm::Message::BuildScriptExecuted(cm::BuildScript {
                package_id,
                out_dir,
                ..
            }) => Some((&metadata[&package_id].id, out_dir)),
            _ => None,
        })
        .collect())
}

pub(crate) struct MacroExpander {
    manifest_dir: PathBuf,
    inited: bool,
}

impl MacroExpander {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let manifest_dir = dirs_next::cache_dir()
            .with_context(|| "could not find the cache directory")?
            .join("cargo-equip")
            .join("macro-expander");

        Ok(Self {
            manifest_dir,
            inited: false,
        })
    }

    pub(crate) fn expand(
        &mut self,
        path: &Path,
        name: &str,
        input: WattInput<'_>,
        shell: &mut Shell,
    ) -> anyhow::Result<String> {
        if !mem::replace(&mut self.inited, true) {
            static MAIN_MANIFEST: &str = indoc! {r#"
                [profile.dev.package.cargo_equip_macro_expander]
                opt-level = 3
                debug = false
                debug-assertions = false
                overflow-checks = false

                [package]
                name = "cargo-equip-macro-expand"
                version = "0.0.0"
                edition = "2018"

                [dependencies]
                cargo_equip_macro_expander = { path = "./expander" }
            "#};

            static SUB_MANIFEST: &str = indoc! {r#"
                [package]
                name = "cargo_equip_macro_expander"
                version = "0.0.0"
                edition = "2018"

                [lib]
                proc-macro = true

                [dependencies]
                watt = "*"
            "#};

            static LIB_RS: &str = indoc! {r#"
                use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};
                use std::{fs, iter};
                use watt::WasmMacro;

                #[proc_macro]
                pub fn expand(input: TokenStream) -> TokenStream {
                    run(|| {
                        let input = &*input.into_iter().collect::<Vec<_>>();

                        // `cargo_equip_macro_expander::expand("/path/to/lib.wasm", ProcMacro, "name", { $($input)* })`
                        if let [TokenTree::Literal(path), TokenTree::Punct(comma1), TokenTree::Ident(kind), TokenTree::Punct(comma2), TokenTree::Literal(fun), TokenTree::Punct(comma3), TokenTree::Group(input)] =
                            input
                        {
                            if kind.to_string() == "ProcMacro"
                                && comma1.as_char() == ','
                                && comma2.as_char() == ','
                                && comma3.as_char() == ','
                            {
                                if let (Some(path), Some(fun)) = (parse_str_lit(path), parse_str_lit(fun)) {
                                    let tt = read_wasm_macro(&path)?.proc_macro(&fun, input.stream());
                                    return write_tt(tt);
                                }
                            }
                        }

                        // `cargo_equip_macro_expander::expand("/path/to/lib.wasm", ProcMacroAttribute, "name", { $($input1)* }, { $($input2)* })`
                        if let [TokenTree::Literal(path), TokenTree::Punct(comma1), TokenTree::Ident(kind), TokenTree::Punct(comma2), TokenTree::Literal(fun), TokenTree::Punct(comma3), TokenTree::Group(input1), TokenTree::Punct(comma4), TokenTree::Group(input2)] =
                            input
                        {
                            if kind.to_string() == "ProcMacroAttribute"
                                && comma1.as_char() == ','
                                && comma2.as_char() == ','
                                && comma3.as_char() == ','
                                && comma4.as_char() == ','
                            {
                                if let (Some(path), Some(fun)) = (parse_str_lit(path), parse_str_lit(fun)) {
                                    let (input1, input2) = (input1.stream(), input2.stream());
                                    let tt = read_wasm_macro(&path)?.proc_macro_attribute(&fun, input1, input2);
                                    return write_tt(tt);
                                }
                            }
                        }

                        // `cargo_equip_macro_expander::expand("/path/to/lib.wasm", ProcMacroDerive, "name", { $($input)* })`
                        if let [TokenTree::Literal(path), TokenTree::Punct(comma1), TokenTree::Ident(kind), TokenTree::Punct(comma2), TokenTree::Literal(fun), TokenTree::Punct(comma3), TokenTree::Group(input)] =
                            input
                        {
                            if kind.to_string() == "ProcMacroDerive"
                                && comma1.as_char() == ','
                                && comma2.as_char() == ','
                                && comma3.as_char() == ','
                            {
                                if let (Some(path), Some(fun)) = (parse_str_lit(path), parse_str_lit(fun)) {
                                    let tt = read_wasm_macro(&path)?.proc_macro_derive(&fun, input.stream());
                                    return write_tt(tt);
                                }
                            }
                        }

                        Err("unexpected format".to_owned())
                    })
                }

                fn run(f: impl FnOnce() -> Result<(), String>) -> TokenStream {
                    if let Err(err) = f() {
                        vec![
                            TokenTree::Ident(Ident::new("compile_error", Span::call_site())),
                            TokenTree::Punct(Punct::new('!', Spacing::Alone)),
                            TokenTree::Group(Group::new(
                                Delimiter::Brace,
                                iter::once(TokenTree::Literal(Literal::string(&err))).collect(),
                            )),
                        ]
                        .into_iter()
                        .collect()
                    } else {
                        TokenStream::new()
                    }
                }

                fn read_wasm_macro(path: &str) -> Result<WasmMacro, String> {
                    let wasm = fs::read(path).map_err(|e| format!("could not read `{}`: {}", path, e))?;
                    Ok(WasmMacro::new(Box::leak(wasm.into_boxed_slice())))
                }

                fn parse_str_lit(lit: &Literal) -> Option<String> {
                    let lit = lit.to_string();
                    if lit.starts_with('"') && lit.ends_with('"') {
                        if lit.contains('\\') {
                            todo!("backslash in {:?}", lit);
                        }
                        Some(lit[1..lit.len() - 1].to_owned())
                    } else {
                        None
                    }
                }

                fn write_tt(output: TokenStream) -> Result<(), String> {
                    let output = output.to_string();
                    return fs::write(PATH, output).map_err(|e| format!("could not write `{}`: {}", PATH, e));
                    static PATH: &str = "./expanded.rs.txt";
                }
            "#};

            xshell::mkdir_p(self.manifest_dir.join("src"))?;
            xshell::mkdir_p(self.manifest_dir.join("expander").join("src"))?;

            write_file_unless_unchanged(&self.manifest_dir.join("Cargo.toml"), MAIN_MANIFEST)?;
            write_file_unless_unchanged(
                &self.manifest_dir.join("expander").join("Cargo.toml"),
                SUB_MANIFEST,
            )?;
            write_file_unless_unchanged(
                &self
                    .manifest_dir
                    .join("expander")
                    .join("src")
                    .join("lib.rs"),
                LIB_RS,
            )?;
        }

        let path = path
            .to_str()
            .with_context(|| format!("non utf-8 path: `{}`", path.display()))?;

        let main_rs = &match input {
            WattInput::ProcMacro(input) => quote! {
                fn main() {}
                cargo_equip_macro_expander::expand!(#path, ProcMacro, #name, { #input });
            },
            WattInput::ProcMacroAttribute(input1, input2) => quote! {
                fn main() {}
                cargo_equip_macro_expander::expand!(#path, ProcMacroAttribute, #name, { #input1 }, { #input2 });
            },
            WattInput::ProcMacroDerive(input) => quote! {
                fn main() {}
                cargo_equip_macro_expander::expand!(#path, ProcMacroDerive, #name, { #input });
            },
        }.to_string();

        write_file_unless_unchanged(&self.manifest_dir.join("src").join("main.rs"), main_rs)?;

        crate::process::process(crate::process::cargo_exe()?)
            .arg("check")
            .cwd(&self.manifest_dir)
            .exec_with_status(shell)?;

        let output = xshell::read_file(self.manifest_dir.join("expanded.rs.txt"))?;
        Ok(output)
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Copy, Clone)]
pub(crate) enum WattInput<'a> {
    ProcMacro(&'a TokenStream),
    ProcMacroAttribute(&'a TokenStream, &'a TokenStream),
    ProcMacroDerive(&'a TokenStream),
}

fn write_file_unless_unchanged(path: &Path, content: &str) -> anyhow::Result<()> {
    let read_file =
        || std::fs::read(path).with_context(|| format!("could not read `{}`", path.display()));

    let write_file = || -> _ {
        std::fs::write(path, content)
            .with_context(|| format!("could not write `{}`", path.display()))
    };

    if !(path.exists() && read_file()? == content.as_bytes()) {
        write_file()?;
    }
    Ok(())
}

pub(crate) fn get_author(workspace_root: &Path) -> anyhow::Result<String> {
    #[derive(Deserialize)]
    struct Manifest {
        package: ManifestPackage,
    }

    #[derive(Deserialize)]
    struct ManifestPackage {
        authors: [String; 1],
    }

    let tempdir = tempfile::Builder::new()
        .prefix("cargo-equip-get-author-")
        .tempdir()?;

    crate::process::process(crate::process::cargo_exe()?)
        .args(&["new", "-q", "--vcs", "none"])
        .arg(tempdir.path().join("a"))
        .cwd(workspace_root)
        .exec()?;

    let manifest = xshell::read_file(tempdir.path().join("a").join("Cargo.toml"))?;
    let author = toml::from_str::<Manifest>(&manifest)?.package.authors[0].clone();
    Ok(author)
}

pub(crate) fn cargo_check_using_current_lockfile_and_cache(
    metadata: &cm::Metadata,
    package: &cm::Package,
    exclude: &[PkgSpec],
    code: &str,
) -> anyhow::Result<()> {
    let name = {
        let mut rng = rand::thread_rng();
        let suf = (0..16)
            .map(|_| match rng.gen_range(0, 26 + 10) {
                n @ 0..=25 => b'a' + n,
                n @ 26..=35 => b'0' + n - 26,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        let suf = str::from_utf8(&suf).expect("should be valid ASCII");
        format!("cargo-equip-check-output-{}", suf)
    };

    let temp_pkg = tempfile::Builder::new()
        .prefix(&name)
        .rand_bytes(0)
        .tempdir()?;

    let cargo_exe = crate::process::cargo_exe()?;

    crate::process::process(&cargo_exe)
        .arg("init")
        .arg("-q")
        .arg("--vcs")
        .arg("none")
        .arg("--bin")
        .arg("--edition")
        .arg(&package.edition)
        .arg("--name")
        .arg(&name)
        .arg(temp_pkg.path())
        .cwd(&metadata.workspace_root)
        .exec()?;

    let orig_manifest =
        std::fs::read_to_string(&package.manifest_path)?.parse::<toml_edit::Document>()?;

    let mut temp_manifest = std::fs::read_to_string(temp_pkg.path().join("Cargo.toml"))?
        .parse::<toml_edit::Document>()?;

    temp_manifest["dependencies"] = orig_manifest["dependencies"].clone();
    if let toml_edit::Item::Table(dependencies) = &mut temp_manifest["dependencies"] {
        let renames = package
            .dependencies
            .iter()
            .flat_map(|cm::Dependency { rename, .. }| rename.as_ref())
            .collect::<HashSet<_>>();

        for name_in_toml in metadata
            .resolve
            .as_ref()
            .expect("`resolve` is `null`")
            .nodes
            .iter()
            .find(|cm::Node { id, .. }| *id == package.id)
            .expect("should contain")
            .deps
            .iter()
            .filter(|cm::NodeDep { pkg, .. }| !exclude.iter().any(|s| s.matches(&metadata[pkg])))
            .map(|cm::NodeDep { name, pkg, .. }| {
                if renames.contains(&name) {
                    name
                } else {
                    &metadata[pkg].name
                }
            })
        {
            dependencies.remove(name_in_toml);
        }
    }

    std::fs::write(
        temp_pkg.path().join("Cargo.toml"),
        temp_manifest.to_string(),
    )?;

    std::fs::create_dir(temp_pkg.path().join("src").join("bin"))?;
    std::fs::write(
        temp_pkg
            .path()
            .join("src")
            .join("bin")
            .join(name)
            .with_extension("rs"),
        code,
    )?;

    std::fs::remove_file(temp_pkg.path().join("src").join("main.rs"))?;

    std::fs::copy(
        metadata.workspace_root.join("Cargo.lock"),
        temp_pkg.path().join("Cargo.lock"),
    )?;

    crate::process::process(cargo_exe)
        .arg("check")
        .arg("--target-dir")
        .arg(&metadata.target_directory)
        .arg("--manifest-path")
        .arg(temp_pkg.path().join("Cargo.toml"))
        .arg("--offline")
        .cwd(&metadata.workspace_root)
        .exec()?;

    temp_pkg.close()?;
    Ok(())
}

pub(crate) trait MetadataExt {
    fn exactly_one_bin_target(&self) -> anyhow::Result<(&cm::Target, &cm::Package)>;
    fn bin_target_by_name<'a>(
        &'a self,
        name: &str,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)>;
    fn bin_target_by_src_path<'a>(
        &'a self,
        src_path: &Path,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)>;
    fn libs_to_bundle<'a>(
        &'a self,
        package_id: &cm::PackageId,
        cargo_udeps_outcome: &HashSet<String>,
        exclude: &[PkgSpec],
    ) -> anyhow::Result<BTreeMap<&'a cm::PackageId, (&'a cm::Target, String)>>;
    fn dep_lib_by_extern_crate_name<'a>(
        &'a self,
        package_id: &cm::PackageId,
        extern_crate_name: &str,
    ) -> anyhow::Result<&cm::Package>;
    fn libs_with_extern_crate_names(
        &self,
        package_id: &cm::PackageId,
        only: &HashSet<&cm::PackageId>,
    ) -> anyhow::Result<BTreeMap<&cm::PackageId, String>>;
}

impl MetadataExt for cm::Metadata {
    fn exactly_one_bin_target(&self) -> anyhow::Result<(&cm::Target, &cm::Package)> {
        match &*bin_targets(self).collect::<Vec<_>>() {
            [] => bail!("no bin target in this workspace"),
            [bin] => Ok(*bin),
            [bins @ ..] => bail!(
                "could not determine which binary to choose. Use the `--bin` option or \
                 `--src` option to specify a binary.\n\
                 available binaries: {}\n\
                 note: currently `cargo-equip` does not support the `default-run` manifest key.",
                bins.iter()
                    .map(|(cm::Target { name, .. }, _)| name)
                    .format(", "),
            ),
        }
    }

    fn bin_target_by_name<'a>(
        &'a self,
        name: &str,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)> {
        match *bin_targets(self)
            .filter(|(t, _)| t.name == name)
            .collect::<Vec<_>>()
        {
            [] => bail!("no bin target named `{}`", name),
            [bin] => Ok(bin),
            [..] => bail!("multiple bin targets named `{}` in this workspace", name),
        }
    }

    fn bin_target_by_src_path<'a>(
        &'a self,
        src_path: &Path,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)> {
        match *bin_targets(self)
            .filter(|(t, _)| t.src_path == src_path)
            .collect::<Vec<_>>()
        {
            [] => bail!(
                "`{}` is not the main source file of any bin targets in this workspace ",
                src_path.display(),
            ),
            [bin] => Ok(bin),
            [..] => bail!(
                "multiple bin targets which `src_path` is `{}`",
                src_path.display(),
            ),
        }
    }

    fn libs_to_bundle<'a>(
        &'a self,
        package_id: &cm::PackageId,
        cargo_udeps_outcome: &HashSet<String>,
        exclude: &[PkgSpec],
    ) -> anyhow::Result<BTreeMap<&'a cm::PackageId, (&'a cm::Target, String)>> {
        let package = &self[package_id];

        let renames = package
            .dependencies
            .iter()
            .flat_map(|cm::Dependency { rename, .. }| rename)
            .collect::<HashSet<_>>();

        let preds = {
            let rustc_exe = crate::process::cargo_exe()?
                .with_file_name("rustc")
                .with_extension(env::consts::EXE_EXTENSION);

            let preds = crate::process::process(rustc_exe)
                .args(&["--print", "cfg"])
                .cwd(package.manifest_path.with_file_name(""))
                .read(true)?;
            cfg_expr::Expression::parse(&format!("all({})", preds.lines().format(",")))?
        };
        let preds = preds.predicates().collect::<Vec<_>>();

        let cm::Resolve { nodes, .. } = self
            .resolve
            .as_ref()
            .with_context(|| "`resolve` is `null`")?;
        let nodes = nodes.iter().map(|n| (&n.id, n)).collect::<HashMap<_, _>>();

        let satisfies = |node_dep: &cm::NodeDep| -> _ {
            if exclude.iter().any(|s| s.matches(&self[&node_dep.pkg])) {
                return false;
            }

            let cm::Node { features, .. } = &nodes[&node_dep.pkg];
            let features = features.iter().map(|s| &**s).collect::<HashSet<_>>();

            node_dep
                .dep_kinds
                .iter()
                .any(|cm::DepKindInfo { kind, target, .. }| {
                    *kind == cm::DependencyKind::Normal
                        && target
                            .as_ref()
                            .and_then(|target| {
                                cfg_expr::Expression::parse(&target.to_string()).ok()
                            })
                            .map_or(true, |target| {
                                target.eval(|pred| match pred {
                                    cfg_expr::Predicate::Feature(feature) => {
                                        features.contains(feature)
                                    }
                                    pred => preds.contains(pred),
                                })
                            })
                })
        };

        if nodes[package_id]
            .deps
            .iter()
            .any(|cm::NodeDep { dep_kinds, .. }| dep_kinds.is_empty())
        {
            bail!("this tool requires Rust 1.41+ for calculating dependencies");
        }

        let mut deps = nodes[package_id]
            .deps
            .iter()
            .filter(|node_dep| satisfies(node_dep))
            .flat_map(|node_dep| {
                let lib_package = &self[&node_dep.pkg];
                let lib_target =
                    lib_package.targets.iter().find(|cm::Target { kind, .. }| {
                        *kind == ["lib".to_owned()] || *kind == ["proc-macro".to_owned()]
                    })?;
                let (lib_extern_crate_name, lib_name_in_toml) = if renames.contains(&node_dep.name)
                {
                    (node_dep.name.clone(), &node_dep.name)
                } else {
                    (lib_target.name.replace('-', "_"), &lib_package.name)
                };
                if cargo_udeps_outcome.contains(lib_name_in_toml) {
                    return None;
                }
                Some((&lib_package.id, (lib_target, lib_extern_crate_name)))
            })
            .collect::<BTreeMap<_, _>>();

        let all_package_ids = &mut deps.keys().copied().collect::<HashSet<_>>();
        let all_extern_crate_names = &mut deps
            .values()
            .map(|(_, s)| s.clone())
            .collect::<HashSet<_>>();

        while {
            let next = deps
                .iter()
                .filter(|(_, (cm::Target { kind, .. }, _))| *kind == ["lib".to_owned()])
                .map(|(package_id, _)| nodes[package_id])
                .flat_map(|cm::Node { deps, .. }| deps)
                .filter(|node_dep| satisfies(node_dep) && all_package_ids.insert(&node_dep.pkg))
                .flat_map(|cm::NodeDep { pkg, .. }| {
                    let package = &self[pkg];
                    let target = package.targets.iter().find(|cm::Target { kind, .. }| {
                        *kind == ["lib".to_owned()] || *kind == ["proc-macro".to_owned()]
                    })?;
                    let mut extern_crate_name = format!(
                        "__{}_{}",
                        package.name.replace('-', "_"),
                        package
                            .version
                            .to_string()
                            .replace(|c| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9'), "_"),
                    );
                    while !all_extern_crate_names.insert(extern_crate_name.clone()) {
                        extern_crate_name += "_";
                    }
                    Some((&package.id, (target, extern_crate_name)))
                })
                .collect::<Vec<_>>();
            let next_is_empty = next.is_empty();
            deps.extend(next);
            !next_is_empty
        } {}

        Ok(deps)
    }

    fn dep_lib_by_extern_crate_name<'a>(
        &'a self,
        package_id: &cm::PackageId,
        extern_crate_name: &str,
    ) -> anyhow::Result<&cm::Package> {
        // https://docs.rs/cargo/0.47.0/src/cargo/core/resolver/resolve.rs.html#323-352

        let package = &self[package_id];

        let node = self
            .resolve
            .as_ref()
            .into_iter()
            .flat_map(|cm::Resolve { nodes, .. }| nodes)
            .find(|cm::Node { id, .. }| id == package_id)
            .with_context(|| format!("`{}` not found in the dependency graph", package_id))?;

        let found_explicitly_renamed_one = package
            .dependencies
            .iter()
            .flat_map(|cm::Dependency { rename, .. }| rename)
            .any(|rename| rename == extern_crate_name);

        if found_explicitly_renamed_one {
            Ok(&self[&node
                .deps
                .iter()
                .find(|cm::NodeDep { name, .. }| name == extern_crate_name)
                .expect("found the dep in `dependencies`, not in `resolve.deps`")
                .pkg])
        } else {
            node.dependencies
                .iter()
                .map(|dep_id| &self[dep_id])
                .flat_map(|p| p.targets.iter().map(move |t| (t, p)))
                .find(|(t, _)| {
                    t.name == extern_crate_name && *t.kind == ["lib".to_owned()]
                        || *t.kind == ["proc-macro".to_owned()]
                })
                .map(|(_, p)| p)
                .with_context(|| {
                    format!(
                        "no external library found which `extern_crate_name` is `{}`",
                        extern_crate_name,
                    )
                })
        }
    }

    fn libs_with_extern_crate_names(
        &self,
        package_id: &cm::PackageId,
        only: &HashSet<&cm::PackageId>,
    ) -> anyhow::Result<BTreeMap<&cm::PackageId, String>> {
        let package = &self[package_id];

        let renames = package
            .dependencies
            .iter()
            .flat_map(|cm::Dependency { rename, .. }| rename)
            .collect::<HashSet<_>>();

        let cm::Resolve { nodes, .. } =
            self.resolve.as_ref().with_context(|| "`resolve` is null")?;

        let cm::Node { deps, .. } = nodes
            .iter()
            .find(|cm::Node { id, .. }| id == package_id)
            .with_context(|| "could not find the node")?;

        Ok(deps
            .iter()
            .filter(|cm::NodeDep { pkg, dep_kinds, .. }| {
                matches!(
                    &**dep_kinds,
                    [cm::DepKindInfo {
                        kind: cm::DependencyKind::Normal,
                        ..
                    }]
                ) && only.contains(pkg)
            })
            .flat_map(|cm::NodeDep { name, pkg, .. }| {
                let extern_crate_name = if renames.contains(name) {
                    name.clone()
                } else {
                    self[pkg]
                        .targets
                        .iter()
                        .find(|cm::Target { kind, .. }| {
                            *kind == ["lib".to_owned()] || *kind == ["proc-macro".to_owned()]
                        })?
                        .name
                        .replace('-', "_")
                };
                Some((pkg, extern_crate_name))
            })
            .collect())
    }
}

fn bin_targets(metadata: &cm::Metadata) -> impl Iterator<Item = (&cm::Target, &cm::Package)> {
    metadata
        .packages
        .iter()
        .filter(move |cm::Package { id, .. }| metadata.workspace_members.contains(id))
        .flat_map(|p| p.targets.iter().map(move |t| (t, p)))
        .filter(|(cm::Target { kind, .. }, _)| *kind == ["bin".to_owned()])
}

pub(crate) trait PackageExt {
    fn has_custom_build(&self) -> bool;
    fn manifest_dir(&self) -> &Path;
    fn metadata(&self) -> anyhow::Result<PackageMetadataCargoEquip>;
    fn read_license_text(&self) -> anyhow::Result<Option<String>>;
}

impl PackageExt for cm::Package {
    fn has_custom_build(&self) -> bool {
        self.targets
            .iter()
            .any(|cm::Target { kind, .. }| *kind == ["custom-build".to_owned()])
    }

    fn manifest_dir(&self) -> &Path {
        self.manifest_path.parent().expect("should not be empty")
    }

    fn metadata(&self) -> anyhow::Result<PackageMetadataCargoEquip> {
        let PackageMetadata { cargo_equip } =
            serde_json::from_value::<Option<_>>(self.metadata.clone())?.unwrap_or_default();
        return Ok(cargo_equip);

        #[derive(Deserialize, Default)]
        #[serde(rename_all = "kebab-case")]
        struct PackageMetadata {
            #[serde(default)]
            cargo_equip: PackageMetadataCargoEquip,
        }
    }

    fn read_license_text(&self) -> anyhow::Result<Option<String>> {
        let mut license = self
            .license
            .as_deref()
            .with_context(|| format!("`{}`: missing `license`", self.id))?;

        if license == "MIT/Apache-2.0" {
            license = "MIT OR Apache-2.0";
        }
        if license == "Apache-2.0/MIT" {
            license = "Apache-2.0 OR MIT";
        }

        let license = spdx::Expression::parse(license).map_err(|e| {
            anyhow!("{}", e).context(format!("`{}`: could not parse `license`", self.id))
        })?;

        let is = |name| license.evaluate(|r| r.license.id() == spdx::license_id(name));

        let read_license_file = |file_names: &[&str]| -> _ {
            let path = file_names
                .iter()
                .map(|name| self.manifest_path.with_file_name(name))
                .find(|path| path.exists())
                .with_context(|| format!("`{}`: could not find the license file", self.id))?;
            xshell::read_file(path).map_err(anyhow::Error::from)
        };

        if is("CC0-1.0") || is("Unlicense") {
            Ok(None)
        } else if is("MIT") {
            read_license_file(&["LICENSE-MIT", "LICENSE"]).map(Some)
        } else if is("Apache-2.0") {
            read_license_file(&["LICENSE-APACHE", "LICENSE"]).map(Some)
        } else {
            bail!("`{}`: unsupported license: `{}`", self.id, license);
        }
    }
}

#[derive(Default, Deserialize)]
pub(crate) struct PackageMetadataCargoEquip {
    #[serde(default)]
    pub(crate) watt: PackageMetadataCargoEquipWatt,
}

#[derive(Default)]
pub(crate) struct PackageMetadataCargoEquipWatt {
    proc_macro: HashMap<String, PackageMetadataCargoEquipWattValue>,
    proc_macro_attribute: HashMap<String, PackageMetadataCargoEquipWattValue>,
    proc_macro_derive: HashMap<String, PackageMetadataCargoEquipWattValue>,
}

impl PackageMetadataCargoEquipWatt {
    pub(crate) fn expand_file_paths(
        &self,
        manifest_dir: &Path,
        out_dir: Option<&Path>,
    ) -> anyhow::Result<WattRuntimes> {
        let variables = liquid::object!({
            "manifest_dir": manifest_dir,
            "out_dir": out_dir,
        });

        let mut runtimes = WattRuntimes::default();

        let render = |value: &PackageMetadataCargoEquipWattValue| -> Result<_, liquid::Error> {
            let path = PathBuf::from(value.path.render(&variables)?);
            let function = value.function.clone();
            Ok(WattRuntime { path, function })
        };

        for (name, value) in &self.proc_macro {
            runtimes.proc_macro.insert(name.clone(), render(value)?);
        }

        for (name, template) in &self.proc_macro_attribute {
            runtimes
                .proc_macro_attribute
                .insert(name.clone(), render(template)?);
        }

        for (name, template) in &self.proc_macro_derive {
            runtimes
                .proc_macro_derive
                .insert(name.clone(), render(template)?);
        }

        Ok(runtimes)
    }
}

impl<'de> Deserialize<'de> for PackageMetadataCargoEquipWatt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let Repr {
            proc_macro,
            proc_macro_attribute,
            proc_macro_derive,
        } = Repr::deserialize(deserializer)?;

        let parser = liquid::ParserBuilder::with_stdlib()
            .build()
            .map_err(D::Error::custom)?;

        let parse = |map: HashMap<String, ReprValue>| -> _ {
            map.into_iter()
                .map(|(name, ReprValue { path, function })| {
                    parser.parse(&path).map(move |path| {
                        (name, PackageMetadataCargoEquipWattValue { path, function })
                    })
                })
                .collect::<Result<_, _>>()
                .map_err(D::Error::custom)
        };

        let proc_macro = parse(proc_macro)?;
        let proc_macro_attribute = parse(proc_macro_attribute)?;
        let proc_macro_derive = parse(proc_macro_derive)?;

        return Ok(Self {
            proc_macro,
            proc_macro_attribute,
            proc_macro_derive,
        });

        #[derive(Deserialize)]
        #[serde(rename_all = "kebab-case")]
        struct Repr {
            #[serde(default)]
            proc_macro: HashMap<String, ReprValue>,
            #[serde(default)]
            proc_macro_attribute: HashMap<String, ReprValue>,
            #[serde(default)]
            proc_macro_derive: HashMap<String, ReprValue>,
        }

        #[derive(Deserialize)]
        struct ReprValue {
            path: String,
            function: String,
        }
    }
}

struct PackageMetadataCargoEquipWattValue {
    path: liquid::Template,
    function: String,
}

#[derive(Default, Debug)]
pub(crate) struct WattRuntimes {
    pub(crate) proc_macro: HashMap<String, WattRuntime>,
    pub(crate) proc_macro_attribute: HashMap<String, WattRuntime>,
    pub(crate) proc_macro_derive: HashMap<String, WattRuntime>,
}

impl WattRuntimes {
    pub(crate) fn merge(mut self, other: Self) -> anyhow::Result<Self> {
        for (name, contents) in other.proc_macro {
            if self.proc_macro.insert(name.clone(), contents).is_some() {
                bail!("duplicated function-like macro: {}", name);
            }
        }

        for (name, contents) in other.proc_macro_attribute {
            if self
                .proc_macro_attribute
                .insert(name.clone(), contents)
                .is_some()
            {
                bail!("duplicated attribute macro: {}", name);
            }
        }

        for (name, contents) in other.proc_macro_derive {
            if self
                .proc_macro_derive
                .insert(name.clone(), contents)
                .is_some()
            {
                bail!("duplicated derive macro: {}", name);
            }
        }

        Ok(self)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct WattRuntime {
    pub(crate) path: PathBuf,
    pub(crate) function: String,
}

pub(crate) trait PackageIdExt {
    fn mask_path(&self) -> String;
}

impl PackageIdExt for cm::PackageId {
    fn mask_path(&self) -> String {
        if_chain! {
            if let [s1, s2] = *self.repr.split(" (path+").collect::<Vec<_>>();
            if s2.ends_with(')');
            then {
                format!(
                    "{} (path+{})",
                    s1,
                    s2.chars().map(|_| '█').collect::<String>(),
                )
            } else {
                self.repr.clone()
            }
        }
    }
}
