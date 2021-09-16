use camino::Utf8Path;
use cargo_util::ProcessBuilder;
use std::env;

pub(crate) fn rustfmt(
    workspace_root: &Utf8Path,
    code: &str,
    edition: &str,
) -> anyhow::Result<String> {
    let tempfile = tempfile::Builder::new()
        .prefix("cargo-equip-")
        .suffix(".rs")
        .tempfile()?
        .into_temp_path();

    cargo_util::paths::write(&tempfile, code)?;

    let rustfmt_exe = crate::process::cargo_exe()?
        .with_file_name("rustfmt")
        .with_extension(env::consts::EXE_EXTENSION);

    ProcessBuilder::new(rustfmt_exe)
        .args(&["--edition", edition])
        .arg(&tempfile)
        .cwd(workspace_root)
        .exec()?;

    let formatted = cargo_util::paths::read(&tempfile)?;

    tempfile.close()?;

    Ok(formatted)
}
