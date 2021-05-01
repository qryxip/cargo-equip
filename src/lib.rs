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
use anyhow::Context as _;
use cargo_metadata as cm;
use either::Either;
use itertools::{iproduct, Itertools as _};
use krates::PkgSpec;
use maplit::hashset;
use prettytable::{cell, format::FormatBuilder, row, Table};
use std::{cmp, collections::BTreeMap, path::PathBuf, str::FromStr};
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

    let Context { cwd, shell } = ctx;

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
                &ra_proc_macro::dl_ra(
                    &dirs_next::cache_dir()
                        .with_context(|| "could not find the cache directory")?
                        .join("cargo-equip"),
                    shell,
                )?,
                proc_macro_crate_dlls,
                shell,
            )
        })
        .transpose()?;

    let code = xshell::read_file(&bin.src_path)?;

    if rust::find_skip_attribute(&code)? {
        shell.status("Found", "`#![cfg_attr(cargo_equip, cargo_equip::skip)]`")?;
        return Ok(code);
    }

    shell.status("Bundling", "the code")?;

    let mut code = rust::expand_mods(&bin.src_path)?;
    if let Some(mut macro_expander) = macro_expander {
        code = rust::expand_proc_macros(&code, &mut macro_expander, shell)?;
    }
    let mut code = rust::process_extern_crate_in_bin(&code, |extern_crate_name| {
        matches!(
            metadata.dep_lib_by_extern_crate_name(&bin_package.id, extern_crate_name),
            Some(lib_package) if libs_to_bundle.contains_key(&lib_package.id)
        )
    })?;

    let contents = libs_to_bundle
        .iter()
        .map(|(lib_package, (lib_target, pseudo_extern_crate_name))| {
            let lib_package = &metadata[lib_package];

            if *lib_target.kind == ["proc-macro".to_owned()] {
                return Ok((pseudo_extern_crate_name, Either::Right(lib_package)));
            }

            let cm::Node { features, .. } = metadata
                .resolve
                .as_ref()
                .map(|cm::Resolve { nodes, .. }| &nodes[..])
                .unwrap_or(&[])
                .iter()
                .find(|cm::Node { id, .. }| *id == lib_package.id)
                .with_context(|| "could not find the data in metadata")?;

            let content = rust::expand_mods(&lib_target.src_path)?;
            let content = match out_dirs.get(&lib_package.id) {
                Some(out_dir) => rust::expand_includes(&content, out_dir)?,
                None => content,
            };
            let content = rust::replace_crate_paths(&content, &pseudo_extern_crate_name, shell)?;
            let content = rust::process_extern_crates_in_lib(shell, &content, |dst| {
                let dst_package =
                    metadata.dep_lib_by_extern_crate_name(&lib_package.id, &dst.to_string())?;
                let (_, dst_pseudo_extern_crate_name) =
                    libs_to_bundle.get(&dst_package.id).unwrap_or_else(|| {
                        panic!(
                            "missing `extern_crate_name` for `{}`. generated one should be given \
                             beforehead. this is a bug",
                            dst_package.id,
                        );
                    });
                Some(dst_pseudo_extern_crate_name.clone())
            })?;
            let content = rust::modify_macros(&content, &pseudo_extern_crate_name)?;
            let mut content = rust::insert_pseudo_extern_preludes(&content, &{
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
            })?;
            if resolve_cfgs {
                content = rust::resolve_cfgs(&content, features)?;
            }
            if remove.contains(&Remove::Docs) {
                content = rust::erase_docs(&content)?;
            }
            if remove.contains(&Remove::Comments) {
                content = rust::erase_comments(&content)?;
            }

            Ok((
                pseudo_extern_crate_name,
                Either::Left((lib_package, content)),
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    if !contents.is_empty() {
        let authors = if bin_package.authors.is_empty() {
            vec![workspace::get_author(&metadata.workspace_root)?]
        } else {
            bin_package.authors.clone()
        };

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
                    .flat_map(|(k, v)| v.as_ref().left().map(|(p, _)| (Some(&***k), *p))),
            );

            list_packages(
                &mut doc,
                "Procedural macros",
                contents
                    .iter()
                    .flat_map(|(_, v)| v.as_ref().right().map(|p| (None, *p))),
            );

            let notices = contents
                .iter()
                .flat_map(|(_, contents)| contents.as_ref().left())
                .map(|(p, _)| p)
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
                    match lib_package.read_license_text() {
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

        if minify == Minify::Libs {
            code += "\n";

            for (pseudo_extern_crate_name, contents) in &contents {
                code += "#[allow(clippy::deprecated_cfg_attr)]#[cfg_attr(rustfmt,rustfmt::skip)]";
                code += "#[allow(unused)]pub mod ";
                code += &pseudo_extern_crate_name.to_string();
                code += "{";
                code += &if let Either::Left((_, content)) = contents {
                    rust::minify(
                        content,
                        shell,
                        Some(&format!("crate::{}", pseudo_extern_crate_name)),
                    )?
                } else {
                    "".to_owned()
                };
                code += "}\n";
            }
        } else {
            for (pseudo_extern_crate_name, contents) in &contents {
                code += "\n#[allow(unused)]\npub mod ";
                code += pseudo_extern_crate_name;
                code += " {\n";
                code += &if let Either::Left((_, content)) = contents {
                    rust::indent_code(content, 1)
                } else {
                    "    // This is a `proc-macro`.\n".to_owned()
                };
                code += "}\n";
            }
        }
    }

    if minify == Minify::All {
        code = rust::minify(&code, shell, None)?;
    }

    if rustfmt {
        code = rustfmt::rustfmt(&metadata.workspace_root, &code, &bin.edition)?;
    }

    Ok(code)
}
