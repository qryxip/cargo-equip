use crate::shell::Shell;
use anyhow::{bail, Context as _};
use itertools::Itertools as _;
use std::{
    env,
    ffi::{OsStr, OsString},
    fmt,
    path::{Path, PathBuf},
};

pub(crate) fn process(program: impl AsRef<OsStr>) -> ProcessBuilder<NotPresent> {
    ProcessBuilder {
        program: program.as_ref().to_owned(),
        args: vec![],
        cwd: (),
    }
}

pub(crate) fn cargo_exe() -> anyhow::Result<PathBuf> {
    env::var_os("CARGO")
        .with_context(|| {
            "missing `$CARGO`. run this program with `cargo equip`, not `cargo-equip equip`"
        })
        .map(Into::into)
}

#[derive(Debug)]
pub(crate) struct ProcessBuilder<C: Presence<PathBuf>> {
    program: OsString,
    args: Vec<OsString>,
    cwd: C::Value,
}

impl<C: Presence<PathBuf>> ProcessBuilder<C> {
    pub(crate) fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    pub(crate) fn args(mut self, args: &[impl AsRef<OsStr>]) -> Self {
        self.args.extend(args.iter().map(|s| s.as_ref().to_owned()));
        self
    }

    pub(crate) fn cwd(self, cwd: impl AsRef<Path>) -> ProcessBuilder<Present> {
        ProcessBuilder {
            program: self.program,
            args: self.args,
            cwd: cwd.as_ref().to_owned(),
        }
    }
}

impl ProcessBuilder<Present> {
    pub(crate) fn exec(&self) -> anyhow::Result<()> {
        let status = std::process::Command::new(&self.program)
            .args(&self.args)
            .current_dir(&self.cwd)
            .status()?;

        if !status.success() {
            bail!("{} didn't exit successfully: {}", self, status);
        }
        Ok(())
    }

    pub(crate) fn exec_with_shell_status(&self, shell: &mut Shell) -> anyhow::Result<()> {
        shell.status("Running", self)?;
        self.exec()
    }
}

impl fmt::Display for ProcessBuilder<Present> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            fmt,
            "`{}{}`",
            shell_escape::escape(self.program.to_string_lossy()),
            self.args.iter().format_with("", |arg, f| f(&format_args!(
                " {}",
                shell_escape::escape(arg.to_string_lossy()),
            ))),
        )
    }
}

pub(crate) trait Presence<T> {
    type Value;
}

#[derive(Debug)]
pub(crate) enum NotPresent {}

impl<T> Presence<T> for NotPresent {
    type Value = ();
}

#[derive(Debug)]
pub(crate) enum Present {}

impl<T> Presence<T> for Present {
    type Value = T;
}
