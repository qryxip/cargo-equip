#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]
#![recursion_limit = "256"]

mod cargo_udeps;
mod process;
mod rust;
mod rustfmt;
pub mod shell;
mod workspace;

use crate::{
    shell::Shell,
    workspace::{MetadataExt as _, PackageExt as _},
};
use anyhow::Context as _;
use cargo_metadata as cm;
use maplit::hashset;
use std::{path::PathBuf, str::FromStr};
use structopt::{clap::AppSettings, StructOpt};
use url::Url;

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
    let Opt::Equip {
        src,
        bin,
        manifest_path,
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

    let unused_deps = match cargo_udeps::cargo_udeps(&bin_package, &bin.name, &toolchain, shell) {
        Ok(unused_deps) => unused_deps,
        Err(warning) => {
            shell.warn(warning)?;
            hashset!()
        }
    };

    shell.status("Bundling", "the code")?;

    let code = rust::expand_mods(&bin.src_path)?;
    let code = rust::comment_out_macro_uses(&code, |extern_crate_name| {
        matches!(
            metadata.dep_lib_by_extern_crate_name(&bin_package.id, extern_crate_name),
            Ok((_, lib_package)) if lib_package.is_available_on_atcoder_or_codingame()
        )
    })?;

    let contents = metadata
        .normal_deps_except_available_on_platforms(&bin_package.id)?
        .into_iter()
        .filter(|(extern_crate_name, _, _)| !unused_deps.contains(extern_crate_name))
        .map(|(extern_crate_name, lib_target, lib_package)| {
            let cm::Node { features, .. } = metadata
                .resolve
                .as_ref()
                .map(|cm::Resolve { nodes, .. }| &nodes[..])
                .unwrap_or(&[])
                .iter()
                .find(|cm::Node { id, .. }| *id == lib_package.id)
                .with_context(|| "could not find the data in metadata")?;

            let content = rust::expand_mods(&lib_target.src_path)?;
            let content = rust::replace_crate_paths(&content, &extern_crate_name, shell)?;
            let content = rust::replace_extern_crates(&content, |dst| {
                let (dst_target, dst_package) =
                    metadata.dep_lib_by_extern_crate_name(&lib_package.id, &dst.to_string())?;
                metadata.extern_crate_name(&bin_package.id, &dst_package.id, dst_target)
            })?;
            let mut content = rust::modify_macros(&content, &extern_crate_name)?;
            if resolve_cfgs {
                content = rust::resolve_cfgs(&content, features)?;
            }
            if remove.contains(&Remove::Docs) {
                content = rust::erase_docs(&content)?;
            }
            if remove.contains(&Remove::Comments) {
                content = rust::erase_comments(&content)?;
            }

            Ok((extern_crate_name, lib_target, lib_package, content))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut code = rust::prepend_mod_doc(&code, &{
        let mut doc = " # Bundled libraries\n".to_owned();
        for (extern_crate_name, _, lib_package, _) in &contents {
            doc += "\n ## ";
            let link = if matches!(&lib_package.source, Some(s) if s.is_crates_io()) {
                format!(
                    "https://crates.io/{}/{}",
                    lib_package.name, lib_package.version
                )
                .parse::<Url>()
                .ok()
            } else {
                lib_package.repository.as_ref().and_then(|s| s.parse().ok())
            };
            if let Some(link) = link {
                doc += "[`";
                doc += &lib_package.name;
                doc += "`](";
                doc += link.as_str();
                doc += ")";
            } else {
                doc += "`";
                doc += &lib_package.name;
                doc += "` (private)";
            }
            doc += "\n\n ### `extern_crate_name`\n\n `";
            doc += extern_crate_name;
            doc += "`\n";
        }
        doc
    })?;

    code += "\n";
    code += "// The following code was expanded by `cargo-equip`.\n";
    code += "\n";

    if minify == Minify::Libs {
        code += "\n";

        for (extern_crate_name, _, _, content) in &contents {
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
        for (extern_crate_name, _, _, content) in &contents {
            code += "\n#[allow(dead_code)]\n mod ";
            code += extern_crate_name;
            code += " {\n";
            code += &rust::indent_code(content, 1);
            code += "}\n";
        }
    }

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
