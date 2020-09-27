#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]
#![recursion_limit = "256"]

mod process;
mod rust;
mod rustfmt;
pub mod shell;
mod workspace;

use crate::{
    rust::Equipments,
    shell::Shell,
    workspace::{MetadataExt as _, PackageExt as _, PseudoModulePath},
};
use anyhow::Context as _;
use quote::ToTokens as _;
use std::collections::HashMap;
use std::{
    collections::{BTreeSet, VecDeque},
    path::PathBuf,
    str::FromStr,
};
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
    Mods,
    All,
}

impl Minify {
    const VARIANTS: &'static [&'static str] = &["none", "mods", "all"];
}

impl FromStr for Minify {
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
    let package_metadata = bin_package.parse_metadata(shell)?;

    shell.status("Bundling", "code")?;

    let code = &std::fs::read_to_string(&bin.src_path)?;

    if syn::parse_file(code)?.shebang.is_some() {
        todo!("shebang is currently not supported");
    }

    let Equipments {
        span,
        uses,
        directly_used_mods,
        mut contents,
    } = rust::equipments(&syn::parse_file(&code)?, |extern_crate_name| {
        let (lib, lib_package) = metadata
            .dep_lib_by_extern_crate_name(&bin_package.id, &extern_crate_name.to_string())?;
        Ok((lib_package.id.clone(), lib.src_path.clone()))
    })?;

    let used_mods = {
        let mut used_mods = directly_used_mods
            .into_iter()
            .flat_map(|(extern_crate_name, module_names)| {
                module_names
                    .into_iter()
                    .map(move |module_name| PseudoModulePath::new(&extern_crate_name, &module_name))
            })
            .collect::<BTreeSet<_>>();
        let mut queue = used_mods.iter().cloned().collect::<VecDeque<_>>();
        loop {
            if let Some(from) = queue.pop_front() {
                if let Some(to) = package_metadata.module_dependencies.get(&from) {
                    for to in to {
                        if used_mods.insert(to.clone()) {
                            queue.push_back(to.clone());
                        }
                    }
                } else {
                    shell.warn(format!(
                        "missing `package.metadata.cargo-equip.module-dependencies.{}`. \
                         including all of the modules",
                        from,
                    ))?;
                    break None;
                }
            } else {
                break Some(used_mods);
            }
        }
    };

    if let Some(used_mods) = used_mods {
        for (key, contents) in &mut contents {
            let (_, extern_crate_name) = key;
            for (mod_name, content) in contents {
                if !used_mods.contains(&PseudoModulePath::new(extern_crate_name, mod_name)) {
                    *content = None;
                }
            }
        }
    }

    let extern_crate_names_by_package_id = contents
        .keys()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<HashMap<_, _>>();

    for (key, contents) in &mut contents {
        let (package_id, extern_crate_name) = key;
        for content in contents.values_mut() {
            if let Some(content) = content {
                *content = rust::replace_extern_crates(content, |dst| {
                    let (_, dst) = metadata
                        .dep_lib_by_extern_crate_name(package_id, &dst.to_string())
                        .ok()?;
                    let dst = extern_crate_names_by_package_id.get(&dst.id)?;
                    Some(dst.to_string())
                })?;
                *content = rust::modify_macros(content, &extern_crate_name.to_string())?;
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

    if minify == Minify::Mods {
        for ((_, extern_crate_name), mod_contents) in contents {
            code += "\npub mod ";
            code += &extern_crate_name.to_string();
            code += " {\n";
            for (mod_name, mod_content) in mod_contents {
                if let Some(mod_content) = mod_content {
                    code += "    #[allow(clippy::deprecated_cfg_attr)]";
                    code += "#[cfg_attr(rustfmt,rustfmt::skip)]pub mod ";
                    code += &mod_name.to_string();
                    code += "{";
                    code += &rust::minify(&mod_content, shell, Some(&mod_name.to_string()))?;
                    code += "}\n";
                }
            }
            code += "}\n";
        }
    } else {
        for ((_, extern_crate_name), mod_contents) in contents {
            code += "\npub mod ";
            code += &extern_crate_name.to_string();
            code += " {";
            for (mod_name, mod_content) in mod_contents {
                if let Some(mod_content) = mod_content {
                    code += "\n    pub mod ";
                    code += &mod_name.to_string();
                    code += " {\n";
                    for line in mod_content.lines() {
                        if !line.is_empty() {
                            code += "        ";
                        }
                        code += line;
                        code += "\n";
                    }
                    code += "    }\n";
                }
            }
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
