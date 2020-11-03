use crate::shell::Shell;
use cargo_metadata as cm;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    env,
    path::PathBuf,
};

pub(crate) fn cargo_udeps(
    package: &cm::Package,
    bin: &str,
    toolchain: &str,
    shell: &mut Shell,
) -> Result<HashSet<String>, String> {
    let cwd = &package.manifest_path.with_file_name("");

    let rustup_exe = which::which_in("rustup", env::var_os("PATH"), cwd)
        .map_err(|_| "could not find `rustup`".to_owned())?;

    let output = crate::process::process(rustup_exe)
        .arg("run")
        .arg(toolchain)
        .arg("cargo")
        .arg("udeps")
        .arg("--output")
        .arg("json")
        .arg("-p")
        .arg(&package.name)
        .arg("--bin")
        .arg(bin)
        .cwd(cwd)
        .read_with_status(false, shell)
        .map_err(|e| e.to_string())?;

    let Outcome { unused_deps } = serde_json::from_str(&output)
        .map_err(|e| format!("could not parse the output of `cargo-udeps`: {}", e))?;

    Ok(unused_deps
        .into_iter()
        .find(|(_, OutcomeUnusedDeps { manifest_path, .. })| {
            *manifest_path == package.manifest_path
        })
        .map(|(_, OutcomeUnusedDeps { normal, .. })| normal)
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
}
