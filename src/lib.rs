#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]
#![recursion_limit = "256"]

mod mod_dep;
mod process;
mod rust;
mod rustfmt;
pub mod shell;
mod workspace;

use crate::{
    rust::{LibContent, ModNames},
    shell::Shell,
    workspace::{MetadataExt as _, PackageExt as _},
};
use anyhow::Context as _;
use maplit::btreemap;
use quote::ToTokens as _;
use std::{iter, path::PathBuf, str::FromStr};
use structopt::{clap::AppSettings, StructOpt};
use url::Url;
use workspace::PackageMetadataCargoEquip;

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
    cargo equip [OPTIONS] --bin <NAME>"#,
        )
    )]
    Equip {
        /// Path the main source file of the bin target
        #[structopt(long, value_name("PATH"), conflicts_with("bin"))]
        src: Option<PathBuf>,

        /// Name of the bin target
        #[structopt(long, value_name("NAME"))]
        bin: Option<String>,

        /// Path to Cargo.toml
        #[structopt(long, value_name("PATH"))]
        manifest_path: Option<PathBuf>,

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
    TestItems,
    Docs,
    Comments,
}

impl Remove {
    const VARIANTS: &'static [&'static str] = &["test-items", "docs", "comments"];
}

impl FromStr for Remove {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, &'static str> {
        match s {
            "test-items" => Ok(Self::TestItems),
            "docs" => Ok(Self::Docs),
            "comments" => Ok(Self::Comments),
            _ => Err(r#"expected "test-items", "docs", or "comments""#),
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
    let Opt::Equip {
        src,
        bin,
        manifest_path,
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

    let Context { cwd, shell } = ctx;

    let manifest_path = if let Some(manifest_path) = manifest_path {
        cwd.join(manifest_path.strip_prefix(".").unwrap_or(&manifest_path))
    } else {
        workspace::locate_project(&cwd)?
    };

    let metadata = workspace::cargo_metadata(&manifest_path, &cwd)?;

    let (bin, bin_package) = if let Some(bin) = bin {
        metadata.bin_target_by_name(&bin)
    } else if let Some(src) = src {
        metadata.bin_target_by_src_path(&cwd.join(src))
    } else {
        metadata.exactly_one_bin_target()
    }?;

    shell.status("Bundling", "the code")?;

    let code = rust::expand_mods(&bin.src_path)?;

    let mut code = if let Some((code, uses)) = rust::find_uses(&code)? {
        let mut contents = rust::extract_names(&uses)
            .into_iter()
            .map(|(extern_crate_name, modules)| {
                let (target, package) = metadata.dep_lib_by_extern_crate_name(
                    &bin_package.id,
                    &extern_crate_name.to_string(),
                )?;

                let content = rust::expand_mods(&target.src_path)?;
                let content = rust::remove_toplevel_items_except_mods_and_extern_crates(&content)?;
                let content =
                    rust::replace_crate_paths(&content, &extern_crate_name.to_string(), shell)?;
                let content = rust::replace_extern_crates(&content, |dst| {
                    let (dst_target, dst_package) = metadata
                        .dep_lib_by_extern_crate_name(&package.id, &dst.to_string())
                        .ok()?;
                    metadata.extern_crate_name(&bin_package.id, &dst_package.id, dst_target)
                })?;
                let mut content = rust::modify_macros(&content, &extern_crate_name.to_string())?;
                if remove.contains(&Remove::TestItems) {
                    content = rust::erase_test_items(&content)?;
                }
                if remove.contains(&Remove::Docs) {
                    content = rust::erase_docs(&content)?;
                }
                if remove.contains(&Remove::Comments) {
                    content = rust::erase_comments(&content)?;
                }

                let modules = match modules {
                    ModNames::Scoped(modules) => modules,
                    ModNames::All => rust::list_mod_names(&content)?,
                };

                Ok(LibContent {
                    package_id: &package.id,
                    extern_crate_name: extern_crate_name.clone(),
                    content,
                    modules,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let used_mods = (|| {
            let mut graph = btreemap!();

            for package_id in contents
                .iter()
                .map(|&LibContent { package_id, .. }| package_id)
                .chain(iter::once(&bin_package.id))
            {
                if let PackageMetadataCargoEquip {
                    module_dependencies: Some(module_dependencies),
                } = metadata[package_id].parse_metadata()?
                {
                    graph.extend(mod_dep::assign_packages(
                        &module_dependencies,
                        package_id,
                        |extern_crate_name| {
                            let (_, to) = metadata.dep_lib_by_extern_crate_name(
                                package_id,
                                &extern_crate_name.to_string(),
                            )?;
                            Ok(&to.id)
                        },
                    )?);
                }
            }

            let start = contents
                .iter()
                .map(
                    |LibContent {
                         package_id,
                         modules,
                         ..
                     }| (*package_id, modules.clone()),
                )
                .collect();

            Ok::<_, anyhow::Error>(mod_dep::connect(&graph, &start))
        })()?;

        rust::remove_unused_modules(&mut contents, used_mods.as_ref())?;

        let mut code = rust::prepend_mod_doc(&code, &{
            let mut doc = " # Bundled libraries\n".to_owned();
            for LibContent {
                package_id,
                extern_crate_name,
                modules,
                ..
            } in &contents
            {
                let package = &metadata[&package_id];
                doc += "\n ## ";
                let link = if matches!(&package.source, Some(s) if s.is_crates_io()) {
                    format!("https://crates.io/{}/{}", package.name, package.version)
                        .parse::<Url>()
                        .ok()
                } else {
                    package.repository.as_ref().and_then(|s| s.parse().ok())
                };
                if let Some(link) = link {
                    doc += "[`";
                    doc += &package.name;
                    doc += "`](";
                    doc += link.as_str();
                    doc += ")";
                } else {
                    doc += "`";
                    doc += &package.name;
                    doc += "` (private)";
                }
                doc += "\n\n ### Modules\n\n";
                for module in modules {
                    doc += " - `::";
                    doc += &extern_crate_name.to_string();
                    doc += "::";
                    doc += module;
                    doc += "`\n";
                }
            }
            doc
        })?;

        code += "\n";
        code += "// The following code was expanded by `cargo-equip`.\n";
        code += "\n";

        for use_stmt in rust::shift_use_statements(&uses) {
            code += &use_stmt.to_token_stream().to_string();
            code += "\n";
        }

        if minify == Minify::Libs {
            code += "\n";

            for LibContent {
                extern_crate_name,
                content,
                ..
            } in &contents
            {
                code += "#[allow(clippy::deprecated_cfg_attr)]#[cfg_attr(rustfmt,rustfmt::skip)]";
                code += "#[allow(dead_code)]mod ";
                code += &extern_crate_name.to_string();
                code += "{";
                code += &rust::minify(
                    content,
                    shell,
                    Some(&format!("crate::{}", extern_crate_name)),
                )?;
                code += "}\n";
            }
        } else {
            for LibContent {
                extern_crate_name,
                content,
                ..
            } in &contents
            {
                code += "\n#[allow(dead_code)]\n mod ";
                code += &extern_crate_name.to_string();
                code += " {\n";
                code += &rust::indent_code(content, 1);
                code += "}\n";
            }
        }

        code
    } else {
        shell.warn("could not find `#![cfg_attr(cargo_equip, equip)]`. skipping expansion")?;
        code
    };

    if minify == Minify::All {
        code = rust::minify(&code, shell, None)?;
    }

    if rustfmt {
        code = rustfmt::rustfmt(&metadata.workspace_root, &code, &bin.edition)?;
    }

    if check {
        workspace::cargo_check_using_current_lockfile_and_cache(&metadata, &bin_package, &code)?;
    }

    if let Some(output) = output {
        let output = cwd.join(output);
        std::fs::write(&output, code)
            .with_context(|| format!("could not write `{}`", output.display()))?;
    } else {
        write!(shell.out(), "{}", code)?;
    }
    Ok(())
}
