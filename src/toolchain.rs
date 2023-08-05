use crate::process::ProcessBuilderExt as _;
use anyhow::{anyhow, ensure};
use camino::Utf8Path;
use cargo_util::ProcessBuilder;
use ra_ap_paths::AbsPathBuf;
use semver::Version;
use std::{
    env,
    path::{Path, PathBuf},
};
use tap::Pipe as _;

pub(crate) fn rustup_exe(cwd: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    which::which_in("rustup", env::var_os("PATH"), cwd).map_err(|_| anyhow!("`rustup` not found"))
}

pub(crate) fn active_toolchain(manifest_dir: &Utf8Path) -> anyhow::Result<String> {
    let output = ProcessBuilder::new(rustup_exe(manifest_dir)?)
        .args(&["show", "active-toolchain"])
        .cwd(manifest_dir)
        .read_stdout::<String>()?;
    Ok(output.split_whitespace().next().unwrap().to_owned())
}

pub(crate) fn find_rust_analyzer_proc_macro_srv(
    manifest_dir: &Utf8Path,
    toolchain: &str,
) -> anyhow::Result<AbsPathBuf> {
    use crate::ra_proc_macro::MSRV;

    let rustup_exe = &rustup_exe(manifest_dir)?;

    let version = ProcessBuilder::new(rustup_exe)
        .args(&["run", toolchain, "rustc", "-V"])
        .cwd(manifest_dir)
        .read_stdout::<String>()?
        .pipe(|output| {
            output
                .split_ascii_whitespace()
                .nth(1)
                .and_then(|output| output.parse::<Version>().ok())
                .ok_or_else(|| anyhow!("Could not parse {output:?}"))
        })?;

    ensure!(
        version >= MSRV,
        "Rust ≧{MSRV} is required for expanding procedural macros. Specify one with \
         `--toolchain-for-proc-macro-srv`",
    );

    let rust_analyzer_proc_macro_srv = ProcessBuilder::new(rustup_exe)
        .args(&["run", toolchain, "rustc", "--print", "sysroot"])
        .cwd(manifest_dir)
        .read_stdout::<String>()?
        .pipe_deref(str::trim_end)
        .pipe(Path::new)
        .join("libexec")
        .join("rust-analyzer-proc-macro-srv")
        .with_extension(env::consts::EXE_EXTENSION)
        .pipe(AbsPathBuf::assert);

    if !Path::new(rust_analyzer_proc_macro_srv.as_os_str()).try_exists()? {
        anyhow::bail!(
            "{} does not exist. Run `rustup component add rust-analyzer --toolchain {toolchain}`",
            Path::new(rust_analyzer_proc_macro_srv.as_os_str()).display(),
        );
    }

    Ok(rust_analyzer_proc_macro_srv)
}
