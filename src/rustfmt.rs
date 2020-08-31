use anyhow::{bail, Context as _};
use std::{env, path::Path, process::Command};

pub(crate) fn rustfmt(code: &str, edition: &str) -> anyhow::Result<String> {
    let tempfile = tempfile::Builder::new()
        .prefix("cargo-equip-")
        .suffix(".rs")
        .tempfile()?
        .into_temp_path();

    std::fs::write(&tempfile, code)?;

    let cargo_exe = env::var_os("CARGO").with_context(|| {
        "missing `$CARGO`. run this program with `cargo equip`, not `cargo-equip equip`"
    })?;

    let rustfmt_exe = Path::new(&cargo_exe)
        .with_file_name("rustfmt")
        .with_extension(env::consts::EXE_EXTENSION);

    let status = Command::new(&rustfmt_exe)
        .args(&["--edition", edition])
        .arg(&tempfile)
        .status()
        .with_context(|| format!("could not execute `{}`", rustfmt_exe.display()))?;

    if !status.success() {
        bail!("`{}` failed ({})", rustfmt_exe.display(), status);
    }

    let formatted = std::fs::read_to_string(&tempfile)?;

    tempfile.close()?;

    Ok(formatted)
}
