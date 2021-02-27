mod tt;

use anyhow::{anyhow, bail, Context as _};
use cargo_metadata as cm;
use itertools::Itertools as _;
use maplit::btreemap;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    env,
    io::{self, BufRead as _, BufReader, Write as _},
    ops::Deref,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use crate::shell::Shell;

pub(crate) fn dl_ra(dir: &Path, shell: &mut Shell) -> anyhow::Result<PathBuf> {
    xshell::mkdir_p(dir)?;

    let ra_path = dir
        .join("rust-analyzer")
        .with_extension(env::consts::EXE_EXTENSION);

    if ra_path.exists() {
        let output = crate::process::process(&ra_path)
            .arg("--version")
            .cwd(dir)
            .read(true)?;
        if output.split_ascii_whitespace().last() == Some(REV) {
            return Ok(ra_path);
        }
    }

    let file_name = if cfg!(all(target_arch = "x86_64", target_os = "windows")) {
        "rust-analyzer-windows.exe"
    } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
        "rust-analyzer-mac"
    } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        "rust-analyzer-linux"
    } else {
        bail!("only x86_64 Windows/macOS/Linux supported");
    };

    let url = format!(
        "https://github.com/rust-analyzer/rust-analyzer/releases/download/{}/{}",
        TAG, file_name,
    );
    curl(&url, &ra_path, dir, shell)?;
    chmod755(&ra_path)?;
    return Ok(ra_path);

    static REV: &str = "14de9e5";
    static TAG: &str = "2021-02-22";

    fn curl(url: &str, dst: &Path, cwd: &Path, shell: &mut Shell) -> anyhow::Result<()> {
        let curl_exe = which::which("curl").map_err(|_| anyhow!("command not found: curl"))?;
        crate::process::process(curl_exe)
            .args(&[url, "-Lo"])
            .arg(dst)
            .cwd(cwd)
            .exec_with_status(shell)
    }

    #[cfg(unix)]
    fn chmod755(path: &Path) -> io::Result<()> {
        use std::{fs::Permissions, os::unix::fs::PermissionsExt as _};
        std::fs::set_permissions(path, Permissions::from_mode(0o755))
    }

    #[cfg(not(unix))]
    #[allow(clippy::unnecessary_wraps)]
    fn chmod755(_: &Path) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn list_proc_macro_dlls<P: FnMut(&cm::PackageId) -> bool>(
    cargo_messages: &[cm::Message],
    mut filter: P,
) -> HashSet<&Path> {
    cargo_messages
        .iter()
        .flat_map(|message| match message {
            cm::Message::CompilerArtifact(artifact) => Some(artifact),
            _ => None,
        })
        .filter(|cm::Artifact { target, .. }| *target.kind == ["proc-macro".to_owned()])
        .filter(|cm::Artifact { package_id, .. }| filter(package_id))
        .flat_map(|cm::Artifact { filenames, .. }| filenames.get(0).map(Deref::deref))
        .collect()
}

pub(crate) struct ProcMacroExpander {
    ra: RaProcMacro,
    func_like: BTreeMap<String, PathBuf>,
    attr: BTreeMap<String, PathBuf>,
    custom_derive: BTreeMap<String, PathBuf>,
}

impl ProcMacroExpander {
    pub(crate) fn new(
        rust_analyzer_exe: &Path,
        dll_paths: &HashSet<&Path>,
        shell: &mut Shell,
    ) -> anyhow::Result<Self> {
        shell.status(
            "Spawning",
            format!("`{} proc-macro`", rust_analyzer_exe.display()),
        )?;

        let mut this = Self {
            ra: RaProcMacro::new(rust_analyzer_exe)?,
            func_like: btreemap!(),
            attr: btreemap!(),
            custom_derive: btreemap!(),
        };

        for dll in dll_paths.iter().copied().sorted() {
            for (name, kind) in this.ra.list_macro(dll, |msg| {
                shell.warn(format!("error from RA: {}", msg))?;
                Ok(())
            })? {
                match kind {
                    ProcMacroKind::CustomDerive => {
                        if this
                            .custom_derive
                            .insert(name.clone(), dll.to_owned())
                            .is_some()
                        {
                            bail!("duplicated `#[derive({})]`", name);
                        }
                    }
                    ProcMacroKind::FuncLike => {
                        if this
                            .func_like
                            .insert(name.clone(), dll.to_owned())
                            .is_some()
                        {
                            bail!("duplicated `{}!`", name);
                        }
                    }
                    ProcMacroKind::Attr => {
                        if this.attr.insert(name.clone(), dll.to_owned()).is_some() {
                            bail!("duplicated `#[{}]`", name);
                        }
                    }
                }
            }
        }

        for name in this.func_like.keys() {
            shell.status("Readied", format!("`{}!`", name))?;
        }
        for name in this.attr.keys() {
            shell.status("Readied", format!("`#[{}]`", name))?;
        }
        for name in this.custom_derive.keys() {
            shell.status("Readied", format!("`#[derive({})]`", name))?;
        }

        Ok(this)
    }

    pub(crate) fn expand_func_like_macro(
        &mut self,
        name: &str,
        body: impl FnOnce() -> proc_macro2::TokenStream,
        on_error: impl FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<Option<proc_macro2::Group>> {
        if let Some(dll) = self.func_like.get(name).cloned() {
            self.expand(&dll, name, body(), None, on_error).map(Some)
        } else {
            Ok(None)
        }
    }

    pub(crate) fn expand_attr_macro(
        &mut self,
        name: &str,
        body: impl FnOnce() -> proc_macro2::TokenStream,
        attr: impl FnOnce() -> proc_macro2::Group,
        on_error: impl FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<Option<proc_macro2::Group>> {
        if let Some(dll) = self.attr.get(name).cloned() {
            self.expand(&dll, name, body(), Some(attr()), on_error)
                .map(Some)
        } else {
            Ok(None)
        }
    }

    pub(crate) fn expand_derive_macro(
        &mut self,
        name: &str,
        body: impl FnOnce() -> proc_macro2::TokenStream,
        on_error: impl FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<Option<proc_macro2::Group>> {
        if let Some(dll) = self.custom_derive.get(name).cloned() {
            self.expand(&dll, name, body(), None, on_error).map(Some)
        } else {
            Ok(None)
        }
    }

    fn expand(
        &mut self,
        dll_path: &Path,
        macro_name: &str,
        macro_body: proc_macro2::TokenStream,
        attributes: Option<proc_macro2::Group>,
        on_error: impl FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<proc_macro2::Group> {
        self.ra
            .expansion_macro(
                dll_path,
                macro_name,
                proc_macro2::Group::new(proc_macro2::Delimiter::None, macro_body).into(),
                attributes.map(Into::into),
                on_error,
            )
            .map(Into::into)
    }
}

pub(crate) struct RaProcMacro {
    process_status: Child,
    process_stdin: ChildStdin,
    process_stdout: BufReader<ChildStdout>,
    list_macro_responses: VecDeque<ListMacrosResult>,
    expansion_macro_responses: VecDeque<ExpansionResult>,
}

impl RaProcMacro {
    pub(crate) fn new(rust_analyzer_exe: &Path) -> anyhow::Result<Self> {
        let mut process = Command::new(rust_analyzer_exe)
            .arg("proc-macro")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("could not execute `{}`", rust_analyzer_exe.display()))?;

        let process_stdin = process.stdin.take().expect("specified `Stdio::piped()`");
        let process_stdout = process.stdout.take().expect("specified `Stdio::piped()`");

        Ok(Self {
            process_status: process,
            process_stdin,
            process_stdout: BufReader::new(process_stdout),
            list_macro_responses: VecDeque::new(),
            expansion_macro_responses: VecDeque::new(),
        })
    }

    pub(crate) fn list_macro(
        &mut self,
        dll_path: &Path,
        mut on_error: impl FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<(String, ProcMacroKind)>> {
        self.request(Request::ListMacro(ListMacrosTask {
            lib: dll_path.to_owned(),
        }))?;

        loop {
            if let Some(ListMacrosResult { macros }) = self.list_macro_responses.pop_front() {
                break Ok(macros);
            }
            self.wait_response(&mut on_error)?;
        }
    }

    fn expansion_macro(
        &mut self,
        dll_path: &Path,
        macro_name: &str,
        macro_body: tt::Subtree,
        attributes: Option<tt::Subtree>,
        mut on_error: impl FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<tt::Subtree> {
        self.request(Request::ExpansionMacro(ExpansionTask {
            macro_body,
            macro_name: macro_name.to_owned(),
            attributes,
            lib: dll_path.to_owned(),
            env: vec![],
        }))?;

        loop {
            if let Some(ExpansionResult { expansion }) = self.expansion_macro_responses.pop_front()
            {
                break Ok(expansion);
            }
            self.wait_response(&mut on_error)?;
        }
    }

    fn request(&mut self, req: Request) -> io::Result<()> {
        let mut req = serde_json::to_string(&req).expect("should not fail");
        req += "\n";
        self.process_stdin.write_all(req.as_ref())?;
        self.process_stdin.flush()
    }

    fn wait_response(
        &mut self,
        mut on_error: impl FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let res = &mut "".to_owned();
        self.process_stdout.read_line(res)?;
        if res.is_empty() {
            if let Some(status) = self.process_status.try_wait()? {
                bail!("rust-analyzer unexpectedly terminated ({})", status);
            }
        }
        match serde_json::from_str(&res)
            .with_context(|| "could not deserialize values from rust-analyzer")?
        {
            Response::Error(err) => {
                on_error(&serde_json::to_string(&err).expect("should not fail"))?
            }
            Response::ListMacro(res) => self.list_macro_responses.push_back(res),
            Response::ExpansionMacro(res) => self.expansion_macro_responses.push_back(res),
        }
        Ok(())
    }
}

#[derive(Serialize)]
enum Request {
    ListMacro(ListMacrosTask),
    ExpansionMacro(ExpansionTask),
}

#[derive(Serialize)]
struct ListMacrosTask {
    lib: PathBuf,
}

#[derive(Serialize)]
struct ExpansionTask {
    macro_body: tt::Subtree,
    macro_name: String,
    attributes: Option<tt::Subtree>,
    lib: PathBuf,
    env: Vec<(String, String)>,
}

#[derive(Deserialize)]
enum Response {
    Error(ResponseError),
    ListMacro(ListMacrosResult),
    ExpansionMacro(ExpansionResult),
}

#[derive(Deserialize, Serialize)]
struct ResponseError {
    code: ErrorCode,
    message: String,
}

#[derive(Copy, Clone, Deserialize, Serialize)]
enum ErrorCode {
    ServerErrorEnd,
    ExpansionError,
}

#[derive(Deserialize)]
struct ListMacrosResult {
    macros: Vec<(String, ProcMacroKind)>,
}

#[derive(Copy, Clone, Deserialize, Debug)]
pub(crate) enum ProcMacroKind {
    CustomDerive,
    FuncLike,
    Attr,
}

#[derive(Deserialize)]
struct ExpansionResult {
    expansion: tt::Subtree,
}
