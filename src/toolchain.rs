use crate::shell::Shell;
use anyhow::{anyhow, Context as _};
use once_cell::sync::Lazy;
use semver::{Version, VersionReq};
use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
};

pub(crate) fn rustup_exe(cwd: &Path) -> anyhow::Result<PathBuf> {
    which::which_in("rustup", env::var_os("PATH"), cwd).map_err(|_| anyhow!("`rustup` not found"))
}

pub(crate) fn active_toolchain(manifest_dir: &Path) -> anyhow::Result<String> {
    let output = crate::process::process(rustup_exe(manifest_dir)?)
        .args(&["show", "active-toolchain"])
        .cwd(manifest_dir)
        .read(true)?;
    Ok(output.split_whitespace().next().unwrap().to_owned())
}

pub(crate) fn find_toolchain_compatible_with_ra(
    manifest_dir: &Path,
    shell: &mut Shell,
) -> anyhow::Result<String> {
    let rustup_exe = &rustup_exe(manifest_dir)?;

    let cargo_version = |toolchain| -> _ {
        crate::process::process(rustup_exe)
            .args(&["run", toolchain, "cargo", "-V"])
            .cwd(manifest_dir)
            .read(true)
            .and_then(|o| extract_version(&o))
    };

    let active_toolchain = active_toolchain(manifest_dir)?;
    if REQ.matches(&cargo_version(&active_toolchain)?) {
        return Ok(active_toolchain);
    }

    let with_status = &mut |toolchain: &str| -> anyhow::Result<_> {
        shell.status(
            "Using",
            format!("`{}` for compiling `proc-macro` crates", toolchain),
        )?;
        Ok(toolchain.to_owned())
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
            REQ.matches(&version).then(|| (version, toolchain))
        })
        .collect::<BTreeMap<Version, _>>();

    if let Some((_, v_1_47_0)) = compatible_toolchains
        .iter()
        .next()
        .filter(|(v, _)| v.to_string() == "1.47.0")
    {
        return with_status(v_1_47_0);
    }

    for toolchain in toolchains {
        if ["stable-", "beta-", "nightly-"]
            .iter()
            .any(|p| toolchain.starts_with(p))
        {
            let version = cargo_version(toolchain)?;
            if REQ.matches(&version) {
                compatible_toolchains.insert(version, toolchain);
            }
        }
    }

    let compatible_toolchain = compatible_toolchains
        .values()
        .next()
        .with_context(|| format!("no toolchain found that satisfies {}", *REQ))?;
    return with_status(compatible_toolchain);

    static REQ: Lazy<VersionReq> = Lazy::new(|| ">=1.47.0".parse().unwrap());

    fn extract_version(output: &str) -> anyhow::Result<Version> {
        output
            .split_whitespace()
            .find_map(|s| s.parse::<Version>().ok())
            .with_context(|| format!("could not parse {:?}", output))
    }
}
