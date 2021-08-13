use crate::shell::Shell;
use anyhow::{anyhow, Context as _};
use camino::Utf8Path;
use cargo_metadata as cm;
use once_cell::sync::Lazy;
use semver::{Version, VersionReq};
use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
};

pub(crate) fn rustup_exe(cwd: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    which::which_in("rustup", env::var_os("PATH"), cwd).map_err(|_| anyhow!("`rustup` not found"))
}

pub(crate) fn active_toolchain(manifest_dir: &Utf8Path) -> anyhow::Result<String> {
    let output = crate::process::process(rustup_exe(manifest_dir)?)
        .args(&["show", "active-toolchain"])
        .cwd(manifest_dir)
        .read(true)?;
    Ok(output.split_whitespace().next().unwrap().to_owned())
}

pub(crate) fn find_toolchain_compatible_with_ra(
    manifest_dir: &Utf8Path,
    shell: &mut Shell,
) -> anyhow::Result<(String, cm::Version)> {
    let rustup_exe = &rustup_exe(manifest_dir)?;

    let cargo_version = |toolchain| -> _ {
        crate::process::process(rustup_exe)
            .args(&["run", toolchain, "cargo", "-V"])
            .cwd(manifest_dir)
            .read(true)
            .and_then(|o| extract_version(&o))
    };

    let active_toolchain = active_toolchain(manifest_dir)?;
    let active_version = cargo_version(&active_toolchain)?;
    if GEQ_1_47_0.matches(&active_version) {
        return Ok((active_toolchain, active_version));
    }

    let status = &mut |toolchain: &str| -> _ {
        shell.status(
            "Using",
            format!("`{}` for compiling `proc-macro` crates", toolchain),
        )
    };

    let output = crate::process::process(rustup_exe)
        .args(&["toolchain", "list"])
        .cwd(manifest_dir)
        .read(true)?;
    let toolchains = output
        .lines()
        .map(|s| s.split_whitespace().next().unwrap())
        .collect::<Vec<_>>();

    let compatible_toolchains = &mut output
        .lines()
        .map(|s| s.split_whitespace().next().unwrap())
        .flat_map(|toolchain| {
            let version = toolchain.split('-').next().unwrap().parse().ok()?;
            GEQ_1_47_0.matches(&version).then(|| (version, toolchain))
        })
        .collect::<BTreeMap<Version, _>>();

    if let Some((version, toolchain)) = compatible_toolchains
        .iter()
        .next()
        .filter(|(v, _)| v.to_string() == "1.47.0")
    {
        status(toolchain)?;
        return Ok(((*toolchain).to_owned(), version.clone()));
    }

    for toolchain in toolchains {
        if ["stable-", "beta-", "nightly-"]
            .iter()
            .any(|p| toolchain.starts_with(p))
        {
            let version = cargo_version(toolchain)?;
            if GEQ_1_47_0.matches(&version) {
                compatible_toolchains.insert(version, toolchain);
            }
        }
    }

    let (compatible_version, compatible_toolchain) = compatible_toolchains
        .iter()
        .next()
        .with_context(|| format!("no toolchain found that satisfies {}", *GEQ_1_47_0))?;
    status(compatible_toolchain)?;
    return Ok((
        (*compatible_toolchain).to_owned(),
        compatible_version.clone(),
    ));

    static GEQ_1_47_0: Lazy<VersionReq> = Lazy::new(|| ">=1.47.0".parse().unwrap());

    fn extract_version(output: &str) -> anyhow::Result<Version> {
        output
            .split_whitespace()
            .find_map(|s| s.parse::<Version>().ok())
            .with_context(|| format!("could not parse {:?}", output))
    }
}
