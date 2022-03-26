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
    rust::CodeEdit,
    shell::Shell,
    workspace::{MetadataExt as _, PackageExt as _, PackageIdExt as _, TargetExt as _},
};
use anyhow::Context as _;
use cargo_metadata as cm;
use indoc::indoc;
use itertools::{iproduct, Itertools as _};
use krates::PkgSpec;
use maplit::{btreeset, hashmap, hashset};
use petgraph::{
    graph::{Graph, NodeIndex},
    visit::Dfs,
};
use prettytable::{cell, format::FormatBuilder, row, Table};
use quote::quote;
use ra_ap_paths::{AbsPath, AbsPathBuf};
use ra_ap_proc_macro_srv as proc_macro_srv;
use std::{
    cmp,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt::Debug,
    path::{Path, PathBuf},
    str::FromStr,
};
use structopt::{clap::AppSettings, StructOpt};

// We need to prepend " " to `long_help`s.
// https://github.com/BurntSushi/ripgrep/blob/9eddb71b8e86a04d7048b920b9b50a2e97068d03/crates/core/app.rs#L533-L539

#[allow(clippy::large_enum_variant)]
#[derive(StructOpt, Debug)]
#[structopt(
    author("Ryo Yamashita <qryxip@gmail.com>"),
    about("Please run as `cargo equip`, not `cargo-equip`."),
    bin_name("cargo"),
    global_settings(&[AppSettings::DeriveDisplayOrder, AppSettings::UnifiedHelpMessage])
)]
pub enum Opt {
    #[structopt(
        about(indoc! {r#"

            A Cargo subcommand to bundle your code into one `.rs` file for competitive programming.

            Use -h for short descriptions and --help for more detials.
        "#}),
        author("Ryo Yamashita <qryxip@gmail.com>"),
        usage(
            r#"cargo equip [OPTIONS]
    cargo equip [OPTIONS] --lib
    cargo equip [OPTIONS] --bin <NAME>
    cargo equip [OPTIONS] --example <NAME>
    cargo equip [OPTIONS] --src <PATH>"#,
        )
    )]
    Equip(OptEquip),
    #[structopt(setting(AppSettings::Hidden))]
    RustAnalyzerProcMacro {},
}

#[derive(StructOpt, Debug)]
pub struct OptEquip {
    /// Bundle the lib/bin/example target and its dependencies
    #[structopt(
        long,
        value_name("PATH"),
        conflicts_with_all(&["lib", "bin", "example"]),
        long_help(indoc! {r#"
            Bundle the lib/bin/example target and its dependencies.

            This option is intended to be used from editors such as VSCode. Use `--lib`, `--bin` or `--example` for normal usage.
        "#})
    )]
    src: Option<PathBuf>,

    /// Bundle the library and its dependencies
    #[structopt(long, conflicts_with_all(&["bin", "example"]))]
    lib: bool,

    /// Bundle the binary and its dependencies
    #[structopt(long, value_name("NAME"), conflicts_with("example"))]
    bin: Option<String>,

    /// Bundle the binary example and its dependencies
    #[structopt(long, value_name("NAME"))]
    example: Option<String>,

    /// Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    manifest_path: Option<PathBuf>,

    /// Exclude library crates from bundling
    #[structopt(long, value_name("SPEC"))]
    exclude: Vec<PkgSpec>,

    /// Alias for `--exclude {crates available on AtCoder}`
    #[structopt(
        long,
        long_help(Box::leak(
            format!(
                "Alias for:\n--exclude {}\n ",
                ATCODER_CRATES.iter().format("\n          "),
            )
            .into_boxed_str(),
        ))
    )]
    exclude_atcoder_crates: bool,

    /// Alias for `--exclude {crates available on CodinGame}`
    #[structopt(
        long,
        long_help(Box::leak(
            format!(
                "Alias for:\n--exclude {}\n ",
                CODINGAME_CRATES.iter().format("\n          "),
            )
            .into_boxed_str(),
        ))
    )]
    exclude_codingame_crates: bool,

    /// Do not include license and copyright notices for the users
    #[structopt(
        long,
        value_name("DOMAIN_AND_USERNAME"),
        long_help(
            concat!(
                indoc! {r#"
                    Do not include license and copyright notices for the users.

                    Supported formats:
                    * github.com/{username}
                    * gitlab.com/{username}
                "#},
                ' ',
            )
        )
    )]
    mine: Vec<User>,

    /// `nightly` toolchain for `cargo-udeps`
    #[structopt(long, value_name("TOOLCHAIN"), default_value("nightly"))]
    toolchain: String,

    /// Expand the libraries to the module
    #[structopt(long, value_name("MODULE_PATH"), default_value("crate::__cargo_equip"))]
    mod_path: CrateSinglePath,

    /// Remove some part [possible values: docs, comments]
    #[structopt(
        long,
        value_name("REMOVE"),
        possible_values(Remove::VARIANTS),
        hide_possible_values(true),
        long_help(concat!(
            indoc! {r#"
                Removes
                * doc comments (`//! ..`, `/// ..`, `/** .. */`, `#[doc = ".."]`) with `--remove docs`.
                * comments (`// ..`, `/* .. */`) with `--remove comments`.

                ```
                #[allow(dead_code)]
                pub mod a {
                    //! A.

                    /// A.
                    pub struct A; // aaaaa
                }
                ```

                ↓

                ```
                #[allow(dead_code)]
                pub mod a {
                    pub struct A;
                }
                ```
            "#},
            ' ',
        ))
    )]
    remove: Vec<Remove>,

    /// Minify part of the output before emitting [default: none]  [possible values: none, libs, all]
    #[structopt(
        long,
        value_name("MINIFY"),
        possible_values(Minify::VARIANTS),
        hide_possible_values(true),
        default_value("none"),
        hide_default_value(true),
        long_help(concat!(
            indoc! {r#"
                Minifies
                - each expaned library with `--minify lib`.
                - the whole code with `--minify all`.

                Not that the minification function is incomplete. Unnecessary spaces may be inserted.
            "#},
            ' ',
        ))
    )]
    minify: Minify,

    /// Do not resolve `cfg(..)`s
    #[structopt(long)]
    no_resolve_cfgs: bool,

    /// Do not format the output before emitting
    #[structopt(long)]
    no_rustfmt: bool,

    /// Do not check the output before emitting
    #[structopt(long)]
    no_check: bool,

    /// Write to the file instead of STDOUT
    #[structopt(short, long, value_name("PATH"))]
    output: Option<PathBuf>,

    /// [Deprecated] Alias for `--minify`
    #[structopt(
        long,
        value_name("MINIFY"),
        possible_values(Minify::VARIANTS),
        default_value("none")
    )]
    oneline: Minify,

    /// [Deprecated] No-op
    #[structopt(long, conflicts_with("no_resolve_cfgs"))]
    resolve_cfgs: bool,

    /// [Deprecated] No-op
    #[structopt(long, conflicts_with("no_rustfmt"))]
    rustfmt: bool,

    /// [Deprecated] No-op
    #[structopt(long, conflicts_with("no_check"))]
    check: bool,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum User {
    Github(String),
    GitlabCom(String),
}

impl FromStr for User {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, &'static str> {
        return if let Some(username) = s.strip_prefix("github.com/") {
            Ok(Self::Github(username.to_owned()))
        } else if let Some(username) = s.strip_prefix("gitlab.com/") {
            Ok(Self::GitlabCom(username.to_owned()))
        } else {
            Err(MSG)
        };

        static MSG: &str = indoc! {r"
            Supported formats:
            * github.com/{username}
            * gitlab.com/{username}
        "};
    }
}

#[derive(Debug, derive_more::Display)]
#[display(fmt = "crate::{}", _0)]
pub struct CrateSinglePath(syn::Ident);

impl FromStr for CrateSinglePath {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, &'static str> {
        (|| {
            let syn::Path {
                leading_colon,
                segments,
            } = syn::parse_str(s).map_err(|_| ())?;
            match (leading_colon, &*segments.into_iter().collect::<Vec<_>>()) {
                (None, [p1, p2]) if p1.ident == "crate" => Ok(Self(p2.ident.clone())),
                _ => Err(()),
            }
        })()
        .map_err(|()| "expected `crate::$ident`")
    }
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
    pub cargo_equip_exe: AbsPathBuf,
    pub cache_dir: PathBuf,
    pub shell: &'a mut Shell,
}

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
    "https://github.com/rust-lang/crates.io-index#proconio:0.3.8",
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
    "https://github.com/rust-lang/crates.io-index#chrono:0.4.19",
    "https://github.com/rust-lang/crates.io-index#itertools:0.10.0",
    "https://github.com/rust-lang/crates.io-index#libc:0.2.93",
    "https://github.com/rust-lang/crates.io-index#rand:0.8.3",
    "https://github.com/rust-lang/crates.io-index#regex:1.4.5",
    "https://github.com/rust-lang/crates.io-index#time:0.2.26",
];

pub fn run(opt: Opt, ctx: Context<'_>) -> anyhow::Result<()> {
    let opt = match opt {
        Opt::Equip(opt) => opt,
        Opt::RustAnalyzerProcMacro {} => return proc_macro_srv::cli::run().map_err(Into::into),
    };
    let OptEquip {
        src,
        lib,
        bin,
        example,
        manifest_path,
        exclude,
        exclude_atcoder_crates,
        exclude_codingame_crates,
        mine,
        toolchain,
        mod_path: CrateSinglePath(cargo_equip_mod_name),
        remove,
        minify,
        no_resolve_cfgs,
        no_rustfmt,
        no_check,
        output,
        oneline: deprecated_oneline_opt,
        resolve_cfgs: deprecated_resolve_cfgs_flag,
        rustfmt: deprecated_rustfmt_flag,
        check: deprecated_check_flag,
    } = opt;

    let minify = match (minify, deprecated_oneline_opt) {
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
        cargo_equip_exe,
        cache_dir,
        shell,
    } = ctx;

    if deprecated_resolve_cfgs_flag {
        shell.warn("`--resolve-cfgs` is deprecated. `#[cfg(..)]`s are resolved by default")?;
    }
    if deprecated_rustfmt_flag {
        shell.warn("`--rustfmt` is deprecated. the output is formatted by default")?;
    }
    if deprecated_check_flag {
        shell.warn("`--check` is deprecated. the output is checked by default")?;
    }

    let manifest_path = if let Some(manifest_path) = manifest_path {
        cwd.join(manifest_path.strip_prefix(".").unwrap_or(&manifest_path))
    } else {
        workspace::locate_project(&cwd)?
    };

    let metadata = workspace::cargo_metadata(&manifest_path, &cwd)?;

    let (root, root_package) = if lib {
        metadata.lib_target()
    } else if let Some(bin) = bin {
        metadata.bin_target_by_name(&bin)
    } else if let Some(example) = example {
        metadata.example_target_by_name(&example)
    } else if let Some(src) = src {
        metadata.target_by_src_path(&cwd.join(src))
    } else {
        metadata.exactly_one_target()
    }?;

    let libs_to_bundle = {
        let unused_deps = &if root.is_lib() {
            hashset!()
        } else {
            match cargo_udeps::cargo_udeps(root_package, root, &toolchain, shell) {
                Ok(unused_deps) => unused_deps,
                Err(warning) => {
                    shell.warn(warning)?;
                    hashset!()
                }
            }
        };
        let mut libs_to_bundle =
            metadata.libs_to_bundle(&root_package.id, root.is_example(), unused_deps, &exclude)?;
        if root.is_lib() {
            libs_to_bundle.insert(&root_package.id, (root, root.crate_name()));
        }
        libs_to_bundle
    };

    let error_message = |head: &str| {
        let mut msg = head.to_owned();

        msg += "\n\n";
        msg += &libs_to_bundle
            .iter()
            .map(|(package_id, (_, pseudo_extern_crate_name))| {
                format!(
                    "- `{}` as `crate::{}::crates::{}`\n",
                    package_id, cargo_equip_mod_name, pseudo_extern_crate_name,
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
        if root.is_lib() {
            RootCrate::Lib(root_package, root)
        } else {
            RootCrate::BinLike(root_package, root)
        },
        &libs_to_bundle,
        &mine,
        &cargo_equip_mod_name,
        !no_resolve_cfgs,
        &remove,
        minify,
        !no_rustfmt,
        &cargo_equip_exe,
        &cache_dir,
        shell,
    )
    .with_context(|| error_message("could not bundle the code"))?;

    if !no_check {
        workspace::cargo_check_using_current_lockfile_and_cache(
            &metadata,
            root_package,
            root,
            &exclude,
            &code,
        )
        .with_context(|| error_message("the bundled code was not valid"))?;
    }

    if let Some(output) = output {
        let output = cwd.join(output);
        cargo_util::paths::write(&output, code)
    } else {
        write!(shell.out(), "{}", code)?;
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn bundle(
    metadata: &cm::Metadata,
    root_crate: RootCrate<'_>,
    libs_to_bundle: &BTreeMap<&cm::PackageId, (&cm::Target, String)>,
    mine: &[User],
    cargo_equip_mod_name: &syn::Ident,
    resolve_cfgs: bool,
    remove: &[Remove],
    minify: Minify,
    rustfmt: bool,
    cargo_equip_exe: &AbsPath,
    cache_dir: &Path,
    shell: &mut Shell,
) -> anyhow::Result<String> {
    let cargo_check_message_format_json = |toolchain: &str, shell: &mut Shell| -> _ {
        let (package, krate) = root_crate.split();
        workspace::cargo_check_message_format_json(toolchain, metadata, package, krate, shell)
    };

    let cargo_messages_for_out_dirs = &libs_to_bundle
        .keys()
        .any(|p| metadata[p].has_custom_build())
        .then(|| {
            let toolchain = &toolchain::active_toolchain(root_crate.package().manifest_dir())?;
            cargo_check_message_format_json(toolchain, shell)
        })
        .unwrap_or_else(|| Ok(vec![]))?;

    let has_proc_macro = libs_to_bundle.keys().any(|p| metadata[p].has_proc_macro());

    let cargo_messages_for_proc_macro_dll_paths = &has_proc_macro
        .then(|| {
            let toolchain = toolchain::find_toolchain_compatible_with_ra(
                root_crate.package().manifest_dir(),
                shell,
            )?;
            let msgs = cargo_check_message_format_json(&toolchain, shell)?;
            Ok::<_, anyhow::Error>(msgs)
        })
        .unwrap_or_else(|| Ok(vec![]))?;

    let out_dirs = workspace::list_out_dirs(metadata, cargo_messages_for_out_dirs);
    let proc_macro_crate_dylibs =
        &ra_proc_macro::list_proc_macro_dylibs(cargo_messages_for_proc_macro_dll_paths, |p| {
            libs_to_bundle.contains_key(p)
        });

    let macro_expander = has_proc_macro
        .then(|| ProcMacroExpander::spawn(cargo_equip_exe, proc_macro_crate_dylibs))
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

    let mut code = if let Some((_, bin_target)) = root_crate.bin_like() {
        let code = cargo_util::paths::read(bin_target.src_path.as_ref())?;
        if rust::find_skip_attribute(&code)? {
            shell.status("Found", "`#![cfg_attr(cargo_equip, cargo_equip::skip)]`")?;
            return Ok(code);
        }
        code
    } else {
        "".to_owned()
    };

    shell.status("Bundling", "the code")?;

    if let Some((bin_package, bin_target)) = root_crate.bin_like() {
        code = rust::process_bin(
            cargo_equip_mod_name,
            &bin_target.src_path,
            { macro_expander }.as_mut(),
            |extern_crate_name| {
                metadata
                    .dep_lib_by_extern_crate_name(&bin_package.id, extern_crate_name)
                    .map(|_| extern_crate_name.to_owned())
            },
            |extern_crate_name| {
                matches!(
                    metadata.dep_lib_by_extern_crate_name(&bin_package.id, extern_crate_name),
                    Some(lib_package) if libs_to_bundle.contains_key(&lib_package.id)
                )
            },
            || (bin_target.crate_name(), &bin_package.id.repr),
        )?;
    }

    let libs = libs_to_bundle
        .iter()
        .map(|(pkg, (krate, pseudo_extern_crate_name))| {
            let mut edit = CodeEdit::new(cargo_equip_mod_name, &krate.src_path, || {
                (krate.crate_name(), &pkg.repr)
            })?;
            if let Some(out_dir) = out_dirs.get(pkg) {
                edit.expand_includes(out_dir)?;
            }
            Ok((*pkg, (*krate, &**pseudo_extern_crate_name, edit)))
        })
        .collect::<anyhow::Result<BTreeMap<_, _>>>()?;

    let (graph, indices) = normal_non_host_dep_graph(&resolve_nodes, libs_to_bundle);

    let libs_using_proc_macros = {
        let mut crates_using_proc_macros = BTreeMap::<_, HashSet<_>>::new();
        for (pkg, names) in &proc_macro_names {
            let (_, pseudo_extern_crate_name) = &libs_to_bundle[pkg];
            for name in names {
                crates_using_proc_macros
                    .entry(&**name)
                    .or_default()
                    .insert(&**pseudo_extern_crate_name);
            }
        }
        for goal in libs_to_bundle.keys() {
            if metadata[goal].has_proc_macro() {
                let mut dfs = Dfs::new(&graph, indices[goal]);
                while let Some(next) = dfs.next(&graph) {
                    let (_, pseudo_extern_crate_name) = &libs_to_bundle[graph[next]];
                    for name in &proc_macro_names[goal] {
                        crates_using_proc_macros
                            .entry(name)
                            .or_default()
                            .insert(pseudo_extern_crate_name);
                    }
                }
            }
        }
        crates_using_proc_macros
    };

    let libs_with_local_inner_macros = {
        let mut libs_with_local_inner_macros = libs
            .keys()
            .map(|pkg| (*pkg, btreeset!()))
            .collect::<HashMap<_, _>>();
        for (goal, (_, pseudo_extern_crate_name, edit)) in &libs {
            if edit.has_local_inner_macros_attr() {
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

    let libs = libs
        .into_iter()
        .map(
            |(lib_package, (lib_target, pseudo_extern_crate_name, mut edit))| {
                let lib_package: &cm::Package = &metadata[lib_package];

                if let Some(names) = proc_macro_names.get(&lib_package.id) {
                    debug_assert_eq!(["proc-macro".to_owned()], *lib_target.kind);
                    let names = names
                        .iter()
                        .map(|name| {
                            let rename = format!(
                                "{}_macro_def_{}_{}",
                                cargo_equip_mod_name, pseudo_extern_crate_name, name,
                            );
                            (name, rename)
                        })
                        .collect::<Vec<_>>();
                    let crate_mod_content = format!(
                        "pub use crate::{}::macros::{}::*;{}",
                        cargo_equip_mod_name,
                        pseudo_extern_crate_name,
                        names
                            .iter()
                            .map(|(name, rename)| {
                                let msg = format!(
                                    "`{}` from `{} {}` should have been expanded",
                                    name, lib_package.name, lib_package.version,
                                );
                                format!(
                                    "#[macro_export]macro_rules!{}\
                                     {{($(_:tt)*)=>(::std::compile_error!({});)}}",
                                    rename,
                                    quote!(#msg),
                                )
                            })
                            .join("")
                    );
                    let macro_mod_content = format!(
                        "pub use crate::{}{}{};",
                        if names.len() == 1 { " " } else { "{" },
                        names
                            .iter()
                            .map(|(name, rename)| format!("{} as {}", rename, name))
                            .format(","),
                        if names.len() == 1 { "" } else { "}" },
                    );
                    return Ok((
                        pseudo_extern_crate_name,
                        (
                            lib_package,
                            crate_mod_content,
                            macro_mod_content,
                            "".to_owned(),
                        ),
                    ));
                }

                let cm::Node { features, .. } = resolve_nodes[&lib_package.id];

                let translate_extern_crate_name = |dst: &_| -> _ {
                    let dst_package =
                        metadata.dep_lib_by_extern_crate_name(&lib_package.id, dst)?;
                    let (_, dst_pseudo_extern_crate_name) =
                        libs_to_bundle.get(&dst_package.id).unwrap_or_else(|| {
                            panic!(
                                "missing `extern_crate_name` for `{}`. generated one should be \
                                 given beforehead. this is a bug",
                                dst_package.id,
                            );
                        });
                    Some(dst_pseudo_extern_crate_name.clone())
                };

                edit.translate_crate_path(pseudo_extern_crate_name)?;
                edit.translate_extern_crate_paths(translate_extern_crate_name)?;
                edit.process_extern_crates_in_lib(translate_extern_crate_name, shell)?;
                let macro_mod_content = edit.modify_declarative_macros(pseudo_extern_crate_name)?;
                let prelude_mod_content = edit.resolve_pseudo_prelude(
                    pseudo_extern_crate_name,
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
                    edit.resolve_cfgs(features)?;
                }
                if remove.contains(&Remove::Docs) {
                    edit.allow_missing_docs();
                    edit.erase_docs()?;
                }
                if remove.contains(&Remove::Comments) {
                    edit.erase_comments()?;
                }

                let crate_mod_content = edit.finish()?;

                Ok((
                    pseudo_extern_crate_name,
                    (
                        lib_package,
                        crate_mod_content,
                        macro_mod_content,
                        prelude_mod_content,
                    ),
                ))
            },
        )
        .collect::<anyhow::Result<Vec<(&str, (&cm::Package, String, String, String))>>>()?;

    if !libs.is_empty() {
        if !root_crate.package().authors.is_empty() {
            shell.warn(
                "`package.authors` are no longer used to skip Copyright and License Notices",
            )?;
            shell.warn("instead, add `--mine github.com/{your username}` to the arguments")?;
        }

        code = rust::insert_prelude_for_main_crate(&code, cargo_equip_mod_name)?;

        code =
            rust::allow_unused_imports_for_seemingly_proc_macros(&code, |mod_name, item_name| {
                matches!(
                    libs_using_proc_macros.get(item_name), Some(pseudo_extern_crate_names)
                    if pseudo_extern_crate_names.contains(mod_name)
                )
            })?;

        let doc = &{
            fn list_packages<'a>(
                doc: &mut String,
                title: &str,
                cargo_equip_mod_name: &syn::Ident,
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
                        row.add_cell(cell!(format!(
                            "as `crate::{}::crates::{}`",
                            cargo_equip_mod_name, pseudo_extern_crate_name,
                        )));
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
                cargo_equip_mod_name,
                libs.iter()
                    .filter(|(_, (p, _, _, _))| p.has_lib())
                    .map(|(k, (p, _, _, _))| (Some(*k), *p)),
            );

            list_packages(
                &mut doc,
                "Procedural macros",
                cargo_equip_mod_name,
                libs.iter()
                    .filter(|(_, (p, _, _, _))| p.has_proc_macro())
                    .map(|(_, (p, _, _, _))| (None, *p)),
            );

            let notices = libs
                .iter()
                .filter(|(_, (p, _, _, _))| p.has_lib())
                .map(|(_, (p, _, _, _))| p)
                .flat_map(|lib_package| {
                    if let Err(err) =
                        shell.status("Checking", format!("the license of `{}`", lib_package.id))
                    {
                        return Some(Err(err.into()));
                    }
                    match lib_package.read_license_text(mine, cache_dir) {
                        Ok(Some(license_text)) => Some(Ok((&lib_package.id, license_text))),
                        Ok(None) => None,
                        Err(err) => Some(Err(err)),
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;

            if !notices.is_empty() {
                doc += "\n # License and Copyright Notices\n";
                for (package_id, license_text) in notices {
                    doc += &format!("\n - `{}`\n\n", package_id.mask_path());
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
        };

        code += "\n";
        code += &match root_crate {
            RootCrate::BinLike(..) => {
                "// The following code was expanded by `cargo-equip`.\n".to_owned()
            }
            RootCrate::Lib(..) => format!("use {}::prelude::*;\n", cargo_equip_mod_name),
        };
        code += "\n";

        let crate_mods = libs
            .iter()
            .map(|(name, (_, content, _, _))| (*name, &**content))
            .collect::<Vec<_>>();

        let macro_mods = libs
            .iter()
            .map(|(name, (_, _, content, _))| (*name, &**content))
            .collect::<Vec<_>>();

        let prelude_mods = libs
            .iter()
            .map(|(name, (_, _, _, content))| (*name, &**content))
            .collect::<Vec<_>>();

        let render_mods = |code: &mut String, mods: &[(&str, &str)]| -> anyhow::Result<()> {
            if minify == Minify::Libs {
                for (pseudo_extern_crate_name, mod_content) in mods {
                    *code += "        pub mod ";
                    *code += &pseudo_extern_crate_name.to_string();
                    *code += " {";
                    *code += &rustminify::minify_file(&rust::parse_file(mod_content)?);
                    *code += "}\n";
                }
            } else {
                for (i, (pseudo_extern_crate_name, mod_content)) in mods.iter().enumerate() {
                    if i > 0 {
                        *code += "\n";
                    }
                    *code += "        pub mod ";
                    *code += pseudo_extern_crate_name;
                    *code += " {\n";
                    *code += &rust::indent_code(mod_content, 3);
                    *code += "    }\n";
                }
            }
            Ok(())
        };

        for doc in doc.lines() {
            code += "///";
            if !code.is_empty() {
                code += " ";
            }
            code += doc;
            code += "\n";
        }
        if minify == Minify::Libs {
            code += "#[cfg_attr(any(), rustfmt::skip)]\n";
        }
        code += "#[allow(unused)]\n";
        code += &format!("mod {} {{\n", cargo_equip_mod_name);
        code += "    pub(crate) mod crates {\n";
        render_mods(&mut code, &crate_mods)?;
        code += "    }\n";
        code += "\n";
        code += "    pub(crate) mod macros {\n";
        render_mods(&mut code, &macro_mods)?;
        code += "    }\n";
        code += "\n";
        code += "    pub(crate) mod prelude {";
        match root_crate {
            RootCrate::BinLike(..) => {
                let prelude_for_main = {
                    let local_macro_uses_in_main_crate = libs_with_local_inner_macros
                        .values()
                        .flatten()
                        .unique()
                        .sorted()
                        .map(|name| format!("{}::*", name))
                        .collect::<Vec<_>>();

                    let local_macro_uses_in_main_crate = match &*local_macro_uses_in_main_crate {
                        [] => None,
                        [part] => Some(part.clone()),
                        parts => Some(format!("{{{}}}", parts.iter().format(","))),
                    };

                    format!(
                        "pub use crate::{}::{};",
                        cargo_equip_mod_name,
                        if let Some(local_macro_uses_in_main_crate) = local_macro_uses_in_main_crate
                        {
                            format!("{{crates::*,macros::{}}}", local_macro_uses_in_main_crate)
                        } else {
                            "crates::*".to_owned()
                        }
                    )
                };
                code += &if minify == Minify::Libs {
                    prelude_for_main
                } else {
                    format!("\n    {}\n    ", prelude_for_main)
                };
            }
            RootCrate::Lib(_, krate) => {
                code += &format!("pub use crate::{}::crates::", cargo_equip_mod_name);
                code += &krate.crate_name();
                code += ";";
            }
        }
        code += "}\n";
        code += "\n";
        code += "    mod preludes {\n";
        render_mods(&mut code, &prelude_mods)?;
        code += "    }\n";
        code += "}\n";
    }

    if minify == Minify::All {
        code = rustminify::minify_file(&rust::parse_file(&code)?);
    }

    if rustfmt {
        code = rustfmt::rustfmt(
            &metadata.workspace_root,
            &code,
            &root_crate.package().edition,
        )?;
    }

    Ok(code)
}

fn normal_non_host_dep_graph<'cm>(
    resolve_nodes: &HashMap<&'cm cm::PackageId, &cm::Node>,
    libs_to_bundle: &BTreeMap<&'cm cm::PackageId, (&cm::Target, String)>,
) -> (
    Graph<&'cm cm::PackageId, ()>,
    HashMap<&'cm cm::PackageId, NodeIndex>,
) {
    let mut graph = Graph::new();
    let mut indices = hashmap!();
    for pkg in libs_to_bundle.keys() {
        indices.insert(*pkg, graph.add_node(*pkg));
    }
    for (from_pkg, (from_crate, _)) in libs_to_bundle {
        if from_crate.is_lib() {
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
    (graph, indices)
}

#[derive(Clone, Copy)]
enum RootCrate<'cm> {
    BinLike(&'cm cm::Package, &'cm cm::Target),
    Lib(&'cm cm::Package, &'cm cm::Target),
}

impl<'cm> RootCrate<'cm> {
    fn split(self) -> (&'cm cm::Package, &'cm cm::Target) {
        match self {
            RootCrate::BinLike(p, t) | RootCrate::Lib(p, t) => (p, t),
        }
    }

    fn package(self) -> &'cm cm::Package {
        match self {
            RootCrate::BinLike(p, _) | RootCrate::Lib(p, _) => p,
        }
    }

    fn bin_like(self) -> Option<(&'cm cm::Package, &'cm cm::Target)> {
        match self {
            RootCrate::BinLike(p, t) => Some((p, t)),
            RootCrate::Lib(..) => None,
        }
    }
}
