#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]
#![recursion_limit = "256"]

mod cargo_udeps;
mod process;
mod ra_proc_macro;
mod rust;
mod rustfmt;
pub mod shell;
mod toolchain;
mod workspace;

use crate::{
    ra_proc_macro::ProcMacroExpander,
    shell::Shell,
    workspace::{MetadataExt as _, PackageExt as _, PackageIdExt as _, TargetExt as _},
};
use anyhow::{ensure, Context as _};
use cargo_metadata as cm;
use itertools::{iproduct, Itertools as _};
use krates::PkgSpec;
use maplit::{btreeset, hashmap, hashset};
use petgraph::{graph::Graph, visit::Dfs};
use prettytable::{cell, format::FormatBuilder, row, Table};
use quote::quote;
use std::{
    cmp,
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    str::FromStr,
};
use structopt::{clap::AppSettings, StructOpt};

#[derive(StructOpt, Debug)]
#[structopt(
    about,
    author,
    bin_name("cargo"),
    global_settings(&[AppSettings::DeriveDisplayOrder, AppSettings::UnifiedHelpMessage])
)]
pub enum Opt {
    #[structopt(
        about,
        author,
        usage(
            r#"cargo equip [OPTIONS]
    cargo equip [OPTIONS] --src <PATH>
    cargo equip [OPTIONS] --bin <NAME>
    cargo equip [OPTIONS] --example <NAME>"#,
        )
    )]
    Equip {
        /// Path the main source file of the bin target
        #[structopt(long, value_name("PATH"), conflicts_with_all(&["bin", "example"]))]
        src: Option<PathBuf>,

        /// Name of the bin target
        #[structopt(long, value_name("NAME"), conflicts_with("example"))]
        bin: Option<String>,

        /// Name of the example target
        #[structopt(long, value_name("NAME"))]
        example: Option<String>,

        /// Path to Cargo.toml
        #[structopt(long, value_name("PATH"))]
        manifest_path: Option<PathBuf>,

        /// Exclude library crates from bundling
        #[structopt(long, value_name("SPEC"))]
        exclude: Vec<PkgSpec>,

        /// Alias for `--exclude https://github.com/rust-lang/crates.io-index#alga:0.9.3 ..`
        #[structopt(long)]
        exclude_atcoder_crates: bool,

        /// Alias for `--exclude https://github.com/rust-lang/crates.io-index#chrono:0.4.9 ..`
        #[structopt(long)]
        exclude_codingame_crates: bool,

        /// `nightly` toolchain for `cargo-udeps`
        #[structopt(long, value_name("TOOLCHAIN"), default_value("nightly"))]
        toolchain: String,

        /// Remove `cfg(..)`s as possible
        #[structopt(long)]
        resolve_cfgs: bool,

        /// Remove some part
        #[structopt(long, value_name("REMOVE"), possible_values(Remove::VARIANTS))]
        remove: Vec<Remove>,

        /// Minify part of the output before emitting
        #[structopt(
            long,
            value_name("MINIFY"),
            possible_values(Minify::VARIANTS),
            default_value("none")
        )]
        minify: Minify,

        /// Alias for `--minify`. Deprecated
        #[structopt(
            long,
            value_name("MINIFY"),
            possible_values(Minify::VARIANTS),
            default_value("none")
        )]
        oneline: Minify,

        /// Format the output before emitting
        #[structopt(long)]
        rustfmt: bool,

        /// Check the output before emitting
        #[structopt(long)]
        check: bool,

        /// Write to the file instead of STDOUT
        #[structopt(short, long, value_name("PATH"))]
        output: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Remove {
    Docs,
    Comments,
}

impl Remove {
    const VARIANTS: &'static [&'static str] = &["docs", "comments"];
}

impl FromStr for Remove {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, &'static str> {
        match s {
            "docs" => Ok(Self::Docs),
            "comments" => Ok(Self::Comments),
            _ => Err(r#"expected "docs", or "comments""#),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Minify {
    None,
    Libs,
    All,
}

impl Minify {
    const VARIANTS: &'static [&'static str] = &["none", "libs", "all"];
}

impl FromStr for Minify {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, &'static str> {
        match s {
            "none" => Ok(Self::None),
            "libs" => Ok(Self::Libs),
            "all" => Ok(Self::All),
            _ => Err(r#"expected "none", "libs", or "all""#),
        }
    }
}

pub struct Context<'a> {
    pub cwd: PathBuf,
    pub cache_dir: PathBuf,
    pub shell: &'a mut Shell,
}

pub fn run(opt: Opt, ctx: Context<'_>) -> anyhow::Result<()> {
    static ATCODER_CRATES: &[&str] = &[
        "https://github.com/rust-lang/crates.io-index#alga:0.9.3",
        "https://github.com/rust-lang/crates.io-index#ascii:1.0.0",
        "https://github.com/rust-lang/crates.io-index#bitset-fixed:0.1.0",
        "https://github.com/rust-lang/crates.io-index#either:1.5.3",
        "https://github.com/rust-lang/crates.io-index#fixedbitset:0.2.0",
        "https://github.com/rust-lang/crates.io-index#getrandom:0.1.14",
        "https://github.com/rust-lang/crates.io-index#im-rc:14.3.0",
        "https://github.com/rust-lang/crates.io-index#indexmap:1.3.2",
        "https://github.com/rust-lang/crates.io-index#itertools:0.9.0",
        "https://github.com/rust-lang/crates.io-index#itertools-num:0.1.3",
        "https://github.com/rust-lang/crates.io-index#lazy_static:1.4.0",
        "https://github.com/rust-lang/crates.io-index#libm:0.2.1",
        "https://github.com/rust-lang/crates.io-index#maplit:1.0.2",
        "https://github.com/rust-lang/crates.io-index#nalgebra:0.20.0",
        "https://github.com/rust-lang/crates.io-index#ndarray:0.13.0",
        "https://github.com/rust-lang/crates.io-index#num:0.2.1",
        "https://github.com/rust-lang/crates.io-index#num-bigint:0.2.6",
        "https://github.com/rust-lang/crates.io-index#num-complex:0.2.4",
        "https://github.com/rust-lang/crates.io-index#num-derive:0.3.0",
        "https://github.com/rust-lang/crates.io-index#num-integer:0.1.42",
        "https://github.com/rust-lang/crates.io-index#num-iter:0.1.40",
        "https://github.com/rust-lang/crates.io-index#num-rational:0.2.4",
        "https://github.com/rust-lang/crates.io-index#num-traits:0.2.11",
        "https://github.com/rust-lang/crates.io-index#ordered-float:1.0.2",
        "https://github.com/rust-lang/crates.io-index#permutohedron:0.2.4",
        "https://github.com/rust-lang/crates.io-index#petgraph:0.5.0",
        "https://github.com/rust-lang/crates.io-index#proconio:0.3.6",
        "https://github.com/rust-lang/crates.io-index#proconio:0.3.7",
        "https://github.com/rust-lang/crates.io-index#rand:0.7.3",
        "https://github.com/rust-lang/crates.io-index#rand_chacha:0.2.2",
        "https://github.com/rust-lang/crates.io-index#rand_core:0.5.1",
        "https://github.com/rust-lang/crates.io-index#rand_distr:0.2.2",
        "https://github.com/rust-lang/crates.io-index#rand_hc:0.2.0",
        "https://github.com/rust-lang/crates.io-index#rand_pcg:0.2.1",
        "https://github.com/rust-lang/crates.io-index#regex:1.3.6",
        "https://github.com/rust-lang/crates.io-index#rustc-hash:1.1.0",
        "https://github.com/rust-lang/crates.io-index#smallvec:1.2.0",
        "https://github.com/rust-lang/crates.io-index#superslice:1.0.0",
        "https://github.com/rust-lang/crates.io-index#text_io:0.1.8",
        "https://github.com/rust-lang/crates.io-index#whiteread:0.5.0",
    ];

    static CODINGAME_CRATES: &[&str] = &[
        "https://github.com/rust-lang/crates.io-index#chrono:0.4.9",
        "https://github.com/rust-lang/crates.io-index#itertools:0.8.0",
        "https://github.com/rust-lang/crates.io-index#libc:0.2.62",
        "https://github.com/rust-lang/crates.io-index#rand:0.7.2",
        "https://github.com/rust-lang/crates.io-index#regex:1.3.0",
        "https://github.com/rust-lang/crates.io-index#time:0.1.42",
    ];

    let Opt::Equip {
        src,
        bin,
        example,
        manifest_path,
        exclude,
        exclude_atcoder_crates,
        exclude_codingame_crates,
        toolchain,
        resolve_cfgs,
        remove,
        minify,
        oneline,
        rustfmt,
        check,
        output,
    } = opt;

    let minify = match (minify, oneline) {
        (Minify::None, oneline) => oneline,
        (minify, _) => minify,
    };

    let exclude = {
        let mut exclude = exclude;
        if exclude_atcoder_crates {
            exclude.extend(ATCODER_CRATES.iter().map(|s| s.parse().unwrap()));
        }
        if exclude_codingame_crates {
            exclude.extend(CODINGAME_CRATES.iter().map(|s| s.parse().unwrap()));
        }
        exclude
    };

    let Context {
        cwd,
        cache_dir,
        shell,
    } = ctx;

    let manifest_path = if let Some(manifest_path) = manifest_path {
        cwd.join(manifest_path.strip_prefix(".").unwrap_or(&manifest_path))
    } else {
        workspace::locate_project(&cwd)?
    };

    let metadata = workspace::cargo_metadata(&manifest_path, &cwd)?;

    let (bin, bin_package) = if let Some(bin) = bin {
        metadata.bin_target_by_name(&bin)
    } else if let Some(example) = example {
        metadata.example_target_by_name(&example)
    } else if let Some(src) = src {
        metadata.bin_like_target_by_src_path(&cwd.join(src))
    } else {
        metadata.exactly_one_bin_like_target()
    }?;

    let libs_to_bundle = {
        let unused_deps = &match cargo_udeps::cargo_udeps(&bin_package, &bin, &toolchain, shell) {
            Ok(unused_deps) => unused_deps,
            Err(warning) => {
                shell.warn(warning)?;
                hashset!()
            }
        };
        metadata.libs_to_bundle(&bin_package.id, bin.is_example(), unused_deps, &exclude)?
    };

    let error_message = |head: &str| {
        let mut msg = head.to_owned();

        msg += "\n\n";
        msg += &libs_to_bundle
            .iter()
            .map(|(package_id, (_, pseudo_extern_crate_name))| {
                format!(
                    "- `{}` as `crate::{}`\n",
                    package_id, pseudo_extern_crate_name,
                )
            })
            .join("");

        let crates_available_on_atcoder = iproduct!(libs_to_bundle.keys(), ATCODER_CRATES)
            .filter(|(id, s)| s.parse::<PkgSpec>().unwrap().matches(&metadata[id]))
            .map(|(id, _)| format!("- `{}`\n", id))
            .join("");

        if !crates_available_on_atcoder.is_empty() {
            msg += &format!(
                "\nnote: attempted to bundle with the following crate(s), which are available on \
                 AtCoder. to exclude them from bundling, run with `--exclude-atcoder-crates`\n\n{}",
                crates_available_on_atcoder,
            );
        }

        msg
    };

    let code = bundle(
        &metadata,
        &bin_package,
        &bin,
        &libs_to_bundle,
        resolve_cfgs,
        &remove,
        minify,
        rustfmt,
        &cache_dir,
        shell,
    )
    .with_context(|| error_message("could not bundle the code"))?;

    if check {
        workspace::cargo_check_using_current_lockfile_and_cache(
            &metadata,
            &bin_package,
            bin.is_example(),
            &exclude,
            &code,
        )
        .with_context(|| error_message("the bundled code was not valid"))?;
    }

    if let Some(output) = output {
        let output = cwd.join(output);
        std::fs::write(&output, code)
            .with_context(|| format!("could not write `{}`", output.display()))
    } else {
        write!(shell.out(), "{}", code)?;
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn bundle(
    metadata: &cm::Metadata,
    bin_package: &cm::Package,
    bin: &cm::Target,
    libs_to_bundle: &BTreeMap<&cm::PackageId, (&cm::Target, String)>,
    resolve_cfgs: bool,
    remove: &[Remove],
    minify: Minify,
    rustfmt: bool,
    cache_dir: &Path,
    shell: &mut Shell,
) -> anyhow::Result<String> {
    let cargo_check_message_format_json = |toolchain: &str, shell: &mut Shell| -> _ {
        workspace::cargo_check_message_format_json(toolchain, metadata, &bin_package, bin, shell)
    };

    let cargo_messages_for_out_dirs = &libs_to_bundle
        .keys()
        .any(|p| metadata[p].has_custom_build())
        .then(|| {
            let toolchain = &toolchain::active_toolchain(bin_package.manifest_dir())?;
            cargo_check_message_format_json(toolchain, shell)
        })
        .unwrap_or_else(|| Ok(vec![]))?;

    let cargo_messages_for_proc_macro_dll_paths = &libs_to_bundle
        .keys()
        .any(|p| metadata[p].has_proc_macro())
        .then(|| {
            let toolchain =
                &toolchain::find_toolchain_compatible_with_ra(bin_package.manifest_dir(), shell)?;
            cargo_check_message_format_json(toolchain, shell)
        })
        .unwrap_or_else(|| Ok(vec![]))?;

    let out_dirs = workspace::list_out_dirs(metadata, cargo_messages_for_out_dirs);
    let proc_macro_crate_dlls =
        &ra_proc_macro::list_proc_macro_dlls(cargo_messages_for_proc_macro_dll_paths, |p| {
            libs_to_bundle.contains_key(p)
        });

    let macro_expander = (!proc_macro_crate_dlls.is_empty())
        .then(|| {
            ProcMacroExpander::new(
                &ra_proc_macro::dl_ra(cache_dir, shell)?,
                proc_macro_crate_dlls,
                shell,
            )
        })
        .transpose()?;

    let proc_macro_names = macro_expander
        .as_ref()
        .map(|macro_expander| {
            let mut proc_macro_names = HashMap::<_, BTreeSet<_>>::new();
            for (pkg, macro_names) in macro_expander.macro_names() {
                for macro_name in macro_names {
                    proc_macro_names
                        .entry(pkg)
                        .or_default()
                        .insert(macro_name.to_owned());
                }
            }
            proc_macro_names
        })
        .unwrap_or_default();

    let resolve_nodes = metadata
        .resolve
        .as_ref()
        .map(|cm::Resolve { nodes, .. }| &nodes[..])
        .unwrap_or(&[])
        .iter()
        .map(|node| (&node.id, node))
        .collect::<HashMap<_, _>>();

    let code = xshell::read_file(&bin.src_path)?;

    if rust::find_skip_attribute(&code)? {
        shell.status("Found", "`#![cfg_attr(cargo_equip, cargo_equip::skip)]`")?;
        return Ok(code);
    }

    shell.status("Bundling", "the code")?;

    let mut code = rust::expand_mods(&bin.src_path)?;
    if let Some(macro_expander) = macro_expander {
        code = rust::expand_proc_macros(&code, &mut { macro_expander }, shell)?;
    }
    let code = rust::translate_abs_paths(&code, |extern_crate_name| {
        metadata
            .dep_lib_by_extern_crate_name(&bin_package.id, extern_crate_name)
            .map(|_| extern_crate_name.to_owned())
    })?;
    let mut code = rust::process_extern_crate_in_bin(&code, |extern_crate_name| {
        matches!(
            metadata.dep_lib_by_extern_crate_name(&bin_package.id, extern_crate_name),
            Some(lib_package) if libs_to_bundle.contains_key(&lib_package.id)
        )
    })?;

    let contents = libs_to_bundle
        .iter()
        .map(|(pkg, (krate, pseudo_extern_crate_name))| {
            let content = rust::expand_mods(&krate.src_path)?;
            let content = match out_dirs.get(pkg) {
                Some(out_dir) => rust::expand_includes(&content, out_dir)?,
                None => content,
            };
            Ok((*pkg, (pseudo_extern_crate_name, content)))
        })
        .collect::<anyhow::Result<BTreeMap<_, _>>>()?;

    let libs_with_local_inner_macros = {
        let mut graph = Graph::new();
        let mut indices = hashmap!();
        for pkg in libs_to_bundle.keys() {
            indices.insert(*pkg, graph.add_node(*pkg));
        }
        for (from_pkg, (from_crate, _)) in libs_to_bundle {
            if from_crate.kind != ["proc-macro".to_owned()] {
                for cm::NodeDep {
                    pkg: to, dep_kinds, ..
                } in &resolve_nodes[from_pkg].deps
                {
                    if *from_pkg != to
                        && dep_kinds
                            .iter()
                            .any(|cm::DepKindInfo { kind, .. }| *kind == cm::DependencyKind::Normal)
                        && libs_to_bundle.contains_key(to)
                    {
                        graph.add_edge(indices[to], indices[*from_pkg], ());
                    }
                }
            }
        }
        let mut libs_with_local_inner_macros = libs_to_bundle
            .keys()
            .map(|pkg| (*pkg, btreeset!()))
            .collect::<HashMap<_, _>>();
        for (goal, (_, pseudo_extern_crate_name)) in libs_to_bundle {
            let (_, code) = &contents[goal];
            if rust::check_local_inner_macros(code)? {
                libs_with_local_inner_macros
                    .get_mut(*goal)
                    .unwrap()
                    .insert(&**pseudo_extern_crate_name);
                let mut dfs = Dfs::new(&graph, indices[goal]);
                while let Some(next) = dfs.next(&graph) {
                    libs_with_local_inner_macros
                        .get_mut(graph[next])
                        .unwrap()
                        .insert(&**pseudo_extern_crate_name);
                }
            }
        }
        libs_with_local_inner_macros
    };

    let contents = libs_to_bundle
        .iter()
        .map(|(lib_package, (lib_target, pseudo_extern_crate_name))| {
            let lib_package: &cm::Package = &metadata[lib_package];

            if let Some(names) = proc_macro_names.get(&lib_package.id) {
                debug_assert_eq!(["proc-macro".to_owned()], *lib_target.kind);
                let names = names
                    .iter()
                    .map(|name| {
                        let rename = format!("__macro_def_{}_{}", pseudo_extern_crate_name, name);
                        (name, rename)
                    })
                    .collect::<Vec<_>>();
                let content = format!(
                    "pub mod __macros{{pub use crate::{}{}{};}}pub use self::__macros::*;",
                    if names.len() == 1 { " " } else { "{" },
                    names
                        .iter()
                        .map(|(name, rename)| format!("{} as {}", rename, name))
                        .format(","),
                    if names.len() == 1 { "" } else { "}" },
                );
                let macros = names
                    .into_iter()
                    .map(|(name, rename)| {
                        let msg = format!(
                            "`{}` from `{} {}` should have been expanded",
                            name, lib_package.name, lib_package.version,
                        );
                        let def = format!(
                            "#[cfg_attr(any(),rustfmt::skip)]#[macro_export]macro_rules!{}\
                             (($(_:tt)*)=>(::std::compile_error!({});));",
                            rename,
                            quote!(#msg),
                        );
                        (rename, def)
                    })
                    .collect::<BTreeMap<_, _>>();
                return Ok((pseudo_extern_crate_name, (lib_package, content, macros)));
            }

            let cm::Node { features, .. } = resolve_nodes[&lib_package.id];

            let translate_extern_crate_name = |dst: &_| -> _ {
                let dst_package = metadata.dep_lib_by_extern_crate_name(&lib_package.id, dst)?;
                let (_, dst_pseudo_extern_crate_name) =
                    libs_to_bundle.get(&dst_package.id).unwrap_or_else(|| {
                        panic!(
                            "missing `extern_crate_name` for `{}`. generated one should be given \
                             beforehead. this is a bug",
                            dst_package.id,
                        );
                    });
                Some(dst_pseudo_extern_crate_name.clone())
            };

            let (_, content) = &contents[&&lib_package.id];
            let content = rust::replace_crate_paths(content, &pseudo_extern_crate_name, shell)?;
            let content = rust::translate_abs_paths(&content, translate_extern_crate_name)?;
            let content =
                rust::process_extern_crates_in_lib(shell, &content, translate_extern_crate_name)?;

            let (content, macros) = rust::modify_declarative_macros(
                &content,
                &pseudo_extern_crate_name,
                remove.contains(&Remove::Docs),
            )?;

            let mut content = rust::insert_pseudo_preludes(
                &content,
                &libs_with_local_inner_macros[&lib_package.id],
                &{
                    metadata
                        .libs_with_extern_crate_names(
                            &lib_package.id,
                            &libs_to_bundle.keys().copied().collect(),
                        )?
                        .into_iter()
                        .map(|(package_id, extern_crate_name)| {
                            let (_, pseudo_extern_crate_name) =
                                libs_to_bundle.get(package_id).with_context(|| {
                                    "could not translate pseudo extern crate names. this is a bug"
                                })?;
                            Ok((extern_crate_name, pseudo_extern_crate_name.clone()))
                        })
                        .collect::<anyhow::Result<_>>()?
                },
            )?;
            if resolve_cfgs {
                content = rust::resolve_cfgs(&content, features)?;
            }
            if remove.contains(&Remove::Docs) {
                content = rust::allow_missing_docs(&content)?;
                content = rust::erase_docs(&content)?;
            }
            if remove.contains(&Remove::Comments) {
                content = rust::erase_comments(&content)?;
            }

            Ok((pseudo_extern_crate_name, (lib_package, content, macros)))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let minify_file = &mut |content, name: Option<&_>, shell: &mut Shell| -> _ {
        rust::minify_file(content, |output| {
            let is_valid = syn::parse_file(output).is_ok();
            if !is_valid {
                shell.warn(format!(
                    "could not minify the code. inserting spaces{}",
                    name.map(|s| format!(": `{}`", s)).unwrap_or_default(),
                ))?;
            }
            Ok(is_valid)
        })
    };

    if !contents.is_empty() {
        let authors = if bin_package.authors.is_empty() {
            workspace::attempt_get_author(&metadata.workspace_root)?
                .into_iter()
                .collect()
        } else {
            bin_package.authors.clone()
        };

        ensure!(
            !authors.is_empty(),
            "cannot know who you are. see https://github.com/qryxip/cargo-equip/issues/120",
        );

        code = rust::prepend_mod_doc(&code, &{
            fn list_packages<'a>(
                doc: &mut String,
                title: &str,
                contents: impl Iterator<Item = (Option<&'a str>, &'a cm::Package)>,
            ) {
                let mut table = Table::new();

                *table.get_format() = FormatBuilder::new()
                    .column_separator(' ')
                    .borders(' ')
                    .build();

                let contents = contents.collect::<Vec<_>>();
                let any_from_local_filesystem = contents.iter().any(|(_, p)| p.source.is_none());

                for (pseudo_extern_crate_name, package) in contents {
                    let mut row = row![format!("- `{}`", package.id.mask_path())];

                    if any_from_local_filesystem {
                        row.add_cell(if package.source.is_some() {
                            cell!("")
                        } else if let Some(repository) = &package.repository {
                            cell!(format!("published in {}", repository))
                        } else {
                            cell!("published in **missing**")
                        });
                    }

                    row.add_cell(if let Some(license) = &package.license {
                        cell!(format!("licensed under `{}`", license))
                    } else {
                        cell!("licensed under **missing**")
                    });

                    if let Some(pseudo_extern_crate_name) = pseudo_extern_crate_name {
                        row.add_cell(cell!(format!("as `crate::{}`", pseudo_extern_crate_name)));
                    }

                    table.add_row(row);
                }

                if !table.is_empty() {
                    if !doc.is_empty() {
                        *doc += "\n";
                    }
                    *doc += &format!(" # {}\n\n", title);
                    for line in table.to_string().lines() {
                        *doc += line.trim_end();
                        *doc += "\n";
                    }
                }
            }

            let mut doc = "".to_owned();

            list_packages(
                &mut doc,
                "Bundled libraries",
                contents
                    .iter()
                    .filter(|(_, (p, _, _))| p.has_lib())
                    .map(|(k, (p, _, _))| (Some(&***k), *p)),
            );

            list_packages(
                &mut doc,
                "Procedural macros",
                contents
                    .iter()
                    .filter(|(_, (p, _, _))| p.has_proc_macro())
                    .map(|(_, (p, _, _))| (None, *p)),
            );

            let notices = contents
                .iter()
                .filter(|(_, (p, _, _))| p.has_lib())
                .map(|(_, (p, _, _))| p)
                .filter(|lib_package| {
                    !authors
                        .iter()
                        .all(|author| lib_package.authors.contains(author))
                })
                .flat_map(|lib_package| {
                    if let Err(err) = shell.status(
                        "Reading",
                        format!("the license file of `{}`", lib_package.id),
                    ) {
                        return Some(Err(err.into()));
                    }
                    match lib_package.read_license_text(cache_dir) {
                        Ok(Some(license_text)) => Some(Ok((&lib_package.id, license_text))),
                        Ok(None) => None,
                        Err(err) => Some(Err(err)),
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;

            if !notices.is_empty() {
                doc += "\n # License and Copyright Notices\n";
                for (package_id, license_text) in notices {
                    doc += &format!("\n - `{}`\n\n", package_id);
                    let backquotes = {
                        let (mut n, mut m) = (2, None);
                        for c in license_text.chars() {
                            if c == '`' {
                                m = Some(m.unwrap_or(0) + 1);
                            } else if let Some(m) = m.take() {
                                n = cmp::max(n, m);
                            }
                        }
                        "`".repeat(cmp::max(n, m.unwrap_or(0)) + 1)
                    };
                    doc += &format!("     {}text\n", backquotes);
                    for line in license_text.lines() {
                        match line {
                            "" => doc += "\n",
                            line => doc += &format!("     {}\n", line),
                        }
                    }
                    doc += &format!("     {}\n", backquotes);
                }
            }

            doc
        })?;

        code += "\n";
        code += "// The following code was expanded by `cargo-equip`.\n";
        code += "\n";

        code += &match &*libs_with_local_inner_macros
            .values()
            .flatten()
            .unique()
            .map(|name| format!("{}::__macros::*", name))
            .collect::<Vec<_>>()
        {
            [] => "".to_owned(),
            [name] => format!("#[allow(unused_imports)]use crate::{};\n\n", name),
            names => format!(
                "#[allow(unused_imports)]use crate::{{{}}};\n\n",
                names.iter().join(","),
            ),
        };

        let macros = contents
            .iter()
            .flat_map(|(_, (_, _, contents))| contents)
            .collect::<BTreeMap<_, _>>();

        for macro_def in macros.values() {
            code += macro_def;
            code += "\n";
        }
        if !macros.is_empty() {
            code += "\n";
        }

        if minify == Minify::Libs {
            code += "\n";

            for (pseudo_extern_crate_name, (_, content, _)) in &contents {
                code += "#[rustfmt::skip]#[allow(unused)]pub mod ";
                code += &pseudo_extern_crate_name.to_string();
                code += "{";
                code += &minify_file(
                    content,
                    Some(&format!("crate::{}", pseudo_extern_crate_name)),
                    shell,
                )?;
                code += "}\n";
            }
        } else {
            for (pseudo_extern_crate_name, (_, content, _)) in &contents {
                code += "\n#[allow(unused)]\npub mod ";
                code += pseudo_extern_crate_name;
                code += " {\n";
                code += &rust::indent_code(content, 1);
                code += "}\n";
            }
        }
    }

    if minify == Minify::All {
        code = minify_file(&code, None, shell)?;
    }

    if rustfmt {
        code = rustfmt::rustfmt(&metadata.workspace_root, &code, &bin.edition)?;
    }

    Ok(code)
}
