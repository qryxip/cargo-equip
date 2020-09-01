mod rust;
mod rustfmt;
pub mod shell;
mod workspace;

use crate::rust::Equipment;
use crate::shell::Shell;
use crate::workspace::{MetadataExt as _, PackageExt as _, PackageMetadataCargoEquip};
use anyhow::anyhow;
use quote::ToTokens as _;
use std::{collections::BTreeSet, path::PathBuf, str::FromStr};
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

        /// Format the output
        #[structopt(long)]
        rustfmt: bool,

        /// Fold part of the output
        #[structopt(long, possible_values(Oneline::VARIANTS), default_value("none"))]
        oneline: Oneline,

        /// Path to Cargo.toml
        #[structopt(long, value_name("PATH"))]
        manifest_path: Option<PathBuf>,
    },
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
        rustfmt,
        oneline,
        manifest_path,
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

    if let Some(Equipment {
        extern_crate_name,
        mods,
        uses,
        span,
    }) = rust::parse_exactly_one_use(&syn::parse_file(&code)?)
        .map_err(|e| anyhow!("{} (span: {:?})", e, e.span()))?
    {
        let (lib, lib_package) = metadata
            .dep_lib_by_extern_crate_name(&bin_package.id, &extern_crate_name.to_string())?;

        let PackageMetadataCargoEquip { mod_dependencies } = lib_package.parse_metadata()?;

        let mod_names = mods.map(|mods| {
            mods.iter()
                .map(ToString::to_string)
                .collect::<BTreeSet<_>>()
        });

        let mod_names = if let Some(mut cur) = mod_names {
            'search: loop {
                let mut next = cur.clone();
                for mod_name in &cur {
                    if let Some(mod_dependencies) = mod_dependencies.get(mod_name) {
                        next.extend(mod_dependencies.iter().cloned());
                    } else {
                        shell.warn(format!(
                            "missing `package.metadata.cargo-equip.mod-dependencies.\"{}\"`. \
                             including all of the modules",
                            mod_name
                        ))?;
                        break 'search None;
                    }
                }
                if next.len() == cur.len() {
                    break Some(next);
                }
                cur = next;
            }
        } else {
            None
        };

        let mod_contents = rust::read_mods(&lib.src_path, mod_names.as_ref())?;

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

        edit += "\n";
        edit += "// The following code was expanded by `cargo-equip`.\n";
        edit += "\n";

        for item_use in uses {
            edit += &item_use.into_token_stream().to_string();
            edit += "\n";
        }

        if oneline == Oneline::Mods {
            edit += "\n";
            for (mod_name, mod_content) in mod_contents {
                edit += "#[allow(clippy::deprecated_cfg_attr)] #[cfg_attr(rustfmt, rustfmt::skip)]";
                edit += " pub mod ";
                edit += &mod_name.to_string();
                edit += " { ";
                edit += &mod_content
                    .parse::<proc_macro2::TokenStream>()
                    .map_err(|e| anyhow!("{:?}", e))?
                    .to_string();
                edit += " }\n";
            }
        } else {
            for (mod_name, mod_content) in mod_contents {
                edit += "\npub mod ";
                edit += &mod_name.to_string();
                edit += " {\n";
                for line in mod_content.lines() {
                    if !line.is_empty() {
                        edit += "    ";
                    }
                    edit += line;
                    edit += "\n";
                }
                edit += "}\n";
            }
        }

        if oneline == Oneline::All {
            edit = edit
                .parse::<proc_macro2::TokenStream>()
                .map_err(|e| anyhow!("{:?}", e))?
                .to_string();
        }

        if rustfmt {
            edit = rustfmt::rustfmt(&edit, &bin.edition)?;
        }

        write!(shell.out(), "{}", edit)?;
    } else {
        shell.warn(format!(
            "could not find `#[::cargo_equip::equip]` attribute in `{}`. returning the file \
             content as-is",
            bin.src_path.display(),
        ))?;
        write!(shell.out(), "{}", code)?;
    }
    Ok(())
}
