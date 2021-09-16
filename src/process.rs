use anyhow::Context as _;
use cargo_util::ProcessError;
use std::{env, fmt, io, path::PathBuf, process::Stdio};

pub(crate) fn cargo_exe() -> anyhow::Result<PathBuf> {
    env::var_os("CARGO")
        .with_context(|| {
            "missing `$CARGO`. run this program with `cargo equip`, not `cargo-equip equip`"
        })
        .map(Into::into)
}

pub(crate) trait ProcessBuilderExt: fmt::Display {
    fn try_inspect(&mut self, f: impl FnOnce(&Self) -> io::Result<()>) -> io::Result<&mut Self> {
        f(self)?;
        Ok(self)
    }

    fn read_stdout<O: StdoutOutput>(&self) -> anyhow::Result<O>;
    fn read_stdout_unchecked<O: StdoutOutput>(&self) -> anyhow::Result<O>;
}

impl ProcessBuilderExt for cargo_util::ProcessBuilder {
    fn read_stdout<O: StdoutOutput>(&self) -> anyhow::Result<O> {
        O::read_stdout(self, true)
    }

    fn read_stdout_unchecked<O: StdoutOutput>(&self) -> anyhow::Result<O> {
        O::read_stdout(self, false)
    }
}

pub(crate) trait StdoutOutput: Sized {
    fn from_bytes(bytes: Vec<u8>, proc: impl fmt::Display) -> anyhow::Result<Self>;

    fn read_stdout(proc: &cargo_util::ProcessBuilder, check: bool) -> anyhow::Result<Self> {
        let output = proc
            .build_command()
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .with_context(|| {
                ProcessError::new(&format!("could not execute process {}", proc), None, None)
            })?;

        if check && !output.status.success() {
            return Err(ProcessError::new(
                &format!("process didn't exit successfully: {}", proc),
                Some(output.status),
                Some(&output),
            )
            .into());
        }

        Self::from_bytes(output.stdout, proc)
    }
}

impl StdoutOutput for String {
    fn from_bytes(bytes: Vec<u8>, proc: impl fmt::Display) -> anyhow::Result<Self> {
        String::from_utf8(bytes).with_context(|| format!("invalid utf-8 output from {}", proc))
    }
}

impl StdoutOutput for Vec<u8> {
    fn from_bytes(bytes: Vec<u8>, _: impl fmt::Display) -> anyhow::Result<Self> {
        Ok(bytes)
    }
}
