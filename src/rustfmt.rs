use crate::shell::Shell;
use anyhow::Context as _;
use std::{env, path::Path};

pub(crate) fn rustfmt(
    shell: &mut Shell,
    workspace_root: &Path,
    code: &str,
    edition: &str,
) -> anyhow::Result<String> {
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

    crate::process::process(rustfmt_exe)
        .args(&["--edition", edition])
        .arg(&tempfile)
        .cwd(workspace_root)
        .exec_with_shell_status(shell)?;

    let formatted = std::fs::read_to_string(&tempfile)?;

    tempfile.close()?;

    Ok(formatted)
}
