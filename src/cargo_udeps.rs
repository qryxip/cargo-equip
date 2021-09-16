use crate::{process::ProcessBuilderExt as _, shell::Shell, toolchain, workspace::TargetExt as _};
use cargo_metadata as cm;
use cargo_util::ProcessBuilder;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

pub(crate) fn cargo_udeps(
    package: &cm::Package,
    target: &cm::Target,
    toolchain: &str,
    shell: &mut Shell,
) -> Result<HashSet<String>, String> {
    let cwd = &package.manifest_path.with_file_name("");

    let rustup_exe = toolchain::rustup_exe(cwd).map_err(|e| e.to_string())?;

    let output = ProcessBuilder::new(rustup_exe)
        .arg("run")
        .arg(toolchain)
        .arg("cargo")
        .arg("udeps")
        .arg("--output")
        .arg("json")
        .arg("-p")
        .arg(&package.name)
        .args(&target.target_option())
        .cwd(cwd)
        .try_inspect(|this| shell.status("Running", this))
        .map_err(|e| e.to_string())?
        .read_stdout_unchecked::<String>()
        .map_err(|e| e.to_string())?;

    let Outcome { unused_deps } = serde_json::from_str(&output)
        .map_err(|e| format!("could not parse the output of `cargo-udeps`: {}", e))?;

    Ok(unused_deps
        .into_iter()
        .find(|(_, OutcomeUnusedDeps { manifest_path, .. })| {
            *manifest_path == package.manifest_path
        })
        .map(
            |(
                _,
                OutcomeUnusedDeps {
                    normal,
                    development,
                    ..
                },
            )| &normal | &development,
        )
        .unwrap_or_default())
}

#[derive(Deserialize)]
struct Outcome {
    unused_deps: HashMap<String, OutcomeUnusedDeps>,
}

#[derive(Deserialize)]
struct OutcomeUnusedDeps {
    manifest_path: PathBuf,
    normal: HashSet<String>,
    development: HashSet<String>,
}
