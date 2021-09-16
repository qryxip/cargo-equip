mod tt;

use crate::{process::ProcessBuilderExt as _, shell::Shell};
use anyhow::{anyhow, bail, Context as _};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata as cm;
use cargo_util::ProcessBuilder;
use maplit::btreemap;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    env,
    io::{self, BufRead as _, BufReader, Read as _, Write as _},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

pub(crate) fn dl_ra(
    dir: &Path,
    rustc_version: &cm::Version,
    shell: &mut Shell,
) -> anyhow::Result<PathBuf> {
    cargo_util::paths::create_dir_all(dir)?;

    let tag = if rustc_version.to_string() == "1.47.0" {
        &TAG_FOR_EQ_1_47
    } else {
        &TAG_FOR_GEQ_1_48
    };

    let ra_path = dir
        .join(format!("rust-analyzer-{}", tag))
        .with_extension(env::consts::EXE_EXTENSION);

    if ra_path.exists() {
        return Ok(ra_path);
    }

    let file_name = if cfg!(all(target_arch = "x86_64", target_os = "windows")) {
        "rust-analyzer-x86_64-pc-windows-msvc.gz"
    } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
        "rust-analyzer-x86_64-apple-darwin.gz"
    } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        "rust-analyzer-x86_64-unknown-linux-gnu.gz"
    } else if cfg!(all(target_arch = "aarch64", target_os = "windows")) {
        "rust-analyzer-aarch64-pc-windows-msvc.gz"
    } else if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
        "rust-analyzer-aarch64-apple-darwin.gz"
    } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
        "rust-analyzer-aarch64-unknown-linux-gnu.gz"
    } else {
        bail!("only x86_64/aarch64 Windows/macOS/Linux supported");
    };

    let url = format!(
        "https://github.com/rust-analyzer/rust-analyzer/releases/download/{}/{}",
        tag, file_name,
    );
    let gz = &curl(&url, dir, shell)?;
    let ra = decode_gz(gz).with_context(|| format!("could not decode {}", file_name))?;
    cargo_util::paths::write(&ra_path, ra)?;
    shell.status("Wrote", ra_path.display())?;
    chmod755(&ra_path)?;
    return Ok(ra_path);

    static TAG_FOR_EQ_1_47: &str = "2021-07-12";
    static TAG_FOR_GEQ_1_48: &str = "2021-08-09";

    fn curl(url: &str, cwd: &Path, shell: &mut Shell) -> anyhow::Result<Vec<u8>> {
        let curl_exe = which::which("curl").map_err(|_| anyhow!("command not found: curl"))?;
        ProcessBuilder::new(curl_exe)
            .args(&[url, "-L"])
            .cwd(cwd)
            .try_inspect(|this| shell.status("Running", this))?
            .read_stdout()
    }

    fn decode_gz(data: &[u8]) -> io::Result<Vec<u8>> {
        let mut buf = vec![];
        libflate::gzip::Decoder::new(data)?.read_to_end(&mut buf)?;
        Ok(buf)
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
) -> BTreeMap<&cm::PackageId, &Utf8Path> {
    cargo_messages
        .iter()
        .flat_map(|message| match message {
            cm::Message::CompilerArtifact(artifact) => Some(artifact),
            _ => None,
        })
        .filter(|cm::Artifact { target, .. }| *target.kind == ["proc-macro".to_owned()])
        .filter(|cm::Artifact { package_id, .. }| filter(package_id))
        .flat_map(
            |cm::Artifact {
                 package_id,
                 filenames,
                 ..
             }| filenames.get(0).map(|filename| (package_id, &**filename)),
        )
        .collect()
}

pub(crate) struct ProcMacroExpander<'msg> {
    ra: RaProcMacro,
    func_like: BTreeMap<String, (&'msg cm::PackageId, &'msg Utf8Path)>,
    attr: BTreeMap<String, (&'msg cm::PackageId, &'msg Utf8Path)>,
    custom_derive: BTreeMap<String, (&'msg cm::PackageId, &'msg Utf8Path)>,
}

impl<'msg> ProcMacroExpander<'msg> {
    pub(crate) fn new(
        rust_analyzer_exe: &Path,
        dll_paths: &BTreeMap<&'msg cm::PackageId, &'msg Utf8Path>,
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

        for (package_id, dll) in dll_paths {
            for (name, kind) in this.ra.list_macro(dll, |msg| {
                shell.warn(format!("error from RA: {}", msg))?;
                Ok(())
            })? {
                match kind {
                    ProcMacroKind::CustomDerive => {
                        if this
                            .custom_derive
                            .insert(name.clone(), (*package_id, *dll))
                            .is_some()
                        {
                            bail!("duplicated `#[derive({})]`", name);
                        }
                    }
                    ProcMacroKind::FuncLike => {
                        if this
                            .func_like
                            .insert(name.clone(), (*package_id, *dll))
                            .is_some()
                        {
                            bail!("duplicated `{}!`", name);
                        }
                    }
                    ProcMacroKind::Attr => {
                        if this
                            .attr
                            .insert(name.clone(), (*package_id, *dll))
                            .is_some()
                        {
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

    pub(crate) fn macro_names(
        &self,
    ) -> impl Iterator<Item = (&'msg cm::PackageId, BTreeSet<&str>)> {
        let mut names = BTreeMap::<_, BTreeSet<_>>::new();
        for (name, (pkg, _)) in self
            .func_like
            .iter()
            .chain(&self.attr)
            .chain(&self.custom_derive)
        {
            names.entry(*pkg).or_default().insert(&**name);
        }
        names.into_iter()
    }

    pub(crate) fn expand_func_like_macro(
        &mut self,
        name: &str,
        body: impl FnOnce() -> proc_macro2::TokenStream,
        on_error: impl FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<Option<proc_macro2::Group>> {
        if let Some((_, dll)) = self.func_like.get(name) {
            let dll = *dll;
            self.expand(dll, name, body(), None, on_error).map(Some)
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
        if let Some((_, dll)) = self.attr.get(name) {
            let dll = *dll;
            self.expand(dll, name, body(), Some(attr()), on_error)
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
        if let Some((_, dll)) = self.custom_derive.get(name) {
            let dll = *dll;
            self.expand(dll, name, body(), None, on_error).map(Some)
        } else {
            Ok(None)
        }
    }

    fn expand(
        &mut self,
        dll_path: &Utf8Path,
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
        dll_path: &Utf8Path,
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
        dll_path: &Utf8Path,
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
        match serde_json::from_str(res)
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

// https://github.com/rust-analyzer/rust-analyzer/blob/2021-03-29/crates/proc_macro_api/src/msg.rs

#[derive(Serialize)]
enum Request {
    ListMacro(ListMacrosTask),
    ExpansionMacro(ExpansionTask),
}

#[derive(Serialize)]
struct ListMacrosTask {
    lib: Utf8PathBuf,
}

#[derive(Serialize)]
struct ExpansionTask {
    macro_body: tt::Subtree,
    macro_name: String,
    attributes: Option<tt::Subtree>,
    lib: Utf8PathBuf,
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
