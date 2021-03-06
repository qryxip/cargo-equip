use camino::Utf8Path;
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

    std::fs::write(&tempfile, code)?;

    let rustfmt_exe = crate::process::cargo_exe()?
        .with_file_name("rustfmt")
        .with_extension(env::consts::EXE_EXTENSION);

    crate::process::process(rustfmt_exe)
        .args(&["--edition", edition])
        .arg(&tempfile)
        .cwd(workspace_root)
        .exec()?;

    let formatted = std::fs::read_to_string(&tempfile)?;

    tempfile.close()?;

    Ok(formatted)
}
