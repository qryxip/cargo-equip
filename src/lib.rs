#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]

mod process;
mod rust;
mod rustfmt;
pub mod shell;
mod workspace;

use crate::rust::Equipments;
use crate::shell::Shell;
use crate::workspace::{LibPackageMetadata, MetadataExt as _, PackageExt as _};
use anyhow::{anyhow, Context as _};
use quote::ToTokens as _;
use std::{collections::BTreeMap, path::PathBuf, str::FromStr};
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

        /// Remove some part
        #[structopt(long, value_name("REMOVE"), possible_values(Remove::VARIANTS))]
        remove: Vec<Remove>,

        /// Fold part of the output before emitting
        #[structopt(
            long,
            value_name("ONELINE"),
            possible_values(Oneline::VARIANTS),
            default_value("none")
        )]
        oneline: Oneline,

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
pub enum Oneline {
    None,
    Mods,
    All,
}

impl Oneline {
    const VARIANTS: &'static [&'static str] = &["none", "mods", "all"];
}

impl FromStr for Oneline {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, &'static str> {
        match s {
            "none" => Ok(Self::None),
            "mods" => Ok(Self::Mods),
            "all" => Ok(Self::All),
            _ => Err(r#"expected "none", "mods", or "all""#),
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
        oneline,
        rustfmt,
        check,
        output,
    } = opt;

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

    shell.status("Bundling", "code")?;

    let code = &std::fs::read_to_string(&bin.src_path)?;

    if syn::parse_file(code)?.shebang.is_some() {
        todo!("shebang is currently not supported");
    }

    let Equipments {
        span,
        uses,
        mut contents,
    } = rust::equipments(&syn::parse_file(&code)?, shell, |extern_crate_name| {
        let (lib, lib_package) = metadata
            .dep_lib_by_extern_crate_name(&bin_package.id, &extern_crate_name.to_string())?;
        let LibPackageMetadata { mod_dependencies } = lib_package.parse_lib_metadata()?;
        Ok((
            lib_package.id.clone(),
            lib.src_path.clone(),
            mod_dependencies,
        ))
    })?;

    for content in contents
        .values_mut()
        .flat_map(BTreeMap::values_mut)
        .flat_map(Option::as_mut)
    {
        if remove.contains(&Remove::TestItems) {
            *content = rust::erase_test_items(content)?;
        }
        if remove.contains(&Remove::Docs) {
            *content = rust::erase_docs(content)?;
        }
        if remove.contains(&Remove::Comments) {
            *content = rust::erase_comments(content)?;
        }
    }

    let mut code = if let Some(span) = span {
        let mut edit = "".to_owned();
        for (i, s) in code.lines().enumerate() {
            if i + 1 == span.start().line && i + 1 == span.end().line {
                edit += &s[..span.start().column];
                edit += "/*";
                edit += &s[span.start().column..span.end().column];
                edit += "*/";
                edit += &s[span.end().column..];
            } else if i + 1 == span.start().line && i + 1 < span.end().line {
                edit += &s[..span.start().column];
                edit += "/*";
                edit += &s[span.start().column..];
            } else if i + 1 > span.start().line && i + 1 == span.end().line {
                edit += &s[..span.end().column];
                edit += "*/";
                edit += &s[span.end().column..];
            } else {
                edit += s;
            }
            edit += "\n";
        }
        edit
    } else {
        code.clone()
    };

    code = rust::prepend_mod_doc(&code, &{
        let mut doc = " # Bundled libraries\n".to_owned();
        for ((package, extern_crate_name), contents) in &contents {
            let package = &metadata[&package];
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
            for (name, content) in contents {
                if content.is_some() {
                    doc += " - `::";
                    doc += &extern_crate_name.to_string();
                    doc += "::";
                    doc += &name.to_string();
                    doc += "` → `$crate::";
                    doc += &name.to_string();
                    doc += "`\n";
                }
            }
        }
        doc
    })?;

    code += "\n";
    code += "// The following code was expanded by `cargo-equip`.\n";
    code += "\n";

    for item_use in uses {
        code += &item_use.into_token_stream().to_string();
        code += "\n";
    }

    if oneline == Oneline::Mods {
        code += "\n";
        for mod_contents in contents.values() {
            for (mod_name, mod_content) in mod_contents {
                if let Some(mod_content) = mod_content {
                    code += "#[allow(clippy::deprecated_cfg_attr)] ";
                    code += "#[cfg_attr(rustfmt, rustfmt::skip)] ";
                    code += "pub mod ";
                    code += &mod_name.to_string();
                    code += " { ";
                    code += &mod_content
                        .parse::<proc_macro2::TokenStream>()
                        .map_err(|e| anyhow!("{:?}", e))?
                        .to_string();
                    code += " }\n";
                }
            }
        }
    } else {
        for mod_contents in contents.values() {
            for (mod_name, mod_content) in mod_contents {
                if let Some(mod_content) = mod_content {
                    code += "\npub mod ";
                    code += &mod_name.to_string();
                    code += " {\n";
                    for line in mod_content.lines() {
                        if !line.is_empty() {
                            code += "    ";
                        }
                        code += line;
                        code += "\n";
                    }
                    code += "}\n";
                }
            }
        }
    }

    if oneline == Oneline::All {
        code = code
            .parse::<proc_macro2::TokenStream>()
            .map_err(|e| anyhow!("{:?}", e))?
            .to_string();
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
