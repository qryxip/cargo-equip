mod license;

use crate::{shell::Shell, toolchain};
use anyhow::{bail, Context as _};
use arrayvec::ArrayVec;
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata as cm;
use if_chain::if_chain;
use itertools::Itertools as _;
use krates::PkgSpec;
use rand::Rng as _;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env,
    io::Cursor,
    path::{Path, PathBuf},
    str,
};

pub(crate) fn locate_project(cwd: &Path) -> anyhow::Result<PathBuf> {
    cwd.ancestors()
        .map(|p| p.join("Cargo.toml"))
        .find(|p| p.exists())
        .with_context(|| {
            format!(
                "could not find `Cargo.toml` in `{}` or any parent directory",
                cwd.display(),
            )
        })
}

pub(crate) fn cargo_metadata(manifest_path: &Path, cwd: &Path) -> cm::Result<cm::Metadata> {
    cm::MetadataCommand::new()
        .manifest_path(manifest_path)
        .current_dir(cwd)
        .exec()
}

pub(crate) fn cargo_check_message_format_json(
    toolchain: &str,
    metadata: &cm::Metadata,
    bin_package: &cm::Package,
    bin_target: &cm::Target,
    shell: &mut Shell,
) -> anyhow::Result<Vec<cm::Message>> {
    let messages = crate::process::process(toolchain::rustup_exe(bin_package.manifest_dir())?)
        .arg("run")
        .arg(toolchain)
        .arg("cargo")
        .arg("check")
        .arg("--message-format")
        .arg("json")
        .arg("-p")
        .arg(format!("{}:{}", bin_package.name, bin_package.version))
        .args(&bin_target.target_option())
        .cwd(&metadata.workspace_root)
        .read_with_status(true, shell)?;

    // TODO: check if ≧ 1.41.0

    cm::Message::parse_stream(Cursor::new(messages))
        .collect::<Result<_, _>>()
        .map_err(Into::into)
}

pub(crate) fn list_out_dirs<'cm>(
    metadata: &'cm cm::Metadata,
    messages: &[cm::Message],
) -> BTreeMap<&'cm cm::PackageId, Utf8PathBuf> {
    messages
        .iter()
        .flat_map(|message| match message {
            cm::Message::BuildScriptExecuted(cm::BuildScript {
                package_id,
                out_dir,
                ..
            }) => Some((&metadata[&package_id].id, out_dir.clone())),
            _ => None,
        })
        .collect()
}

pub(crate) fn attempt_get_author(workspace_root: &Utf8Path) -> anyhow::Result<Option<String>> {
    #[derive(Deserialize)]
    struct Manifest {
        package: ManifestPackage,
    }

    #[derive(Deserialize)]
    struct ManifestPackage {
        #[serde(default)]
        authors: ArrayVec<String, 1>,
    }

    let tempdir = tempfile::Builder::new()
        .prefix("cargo-equip-get-author-")
        .tempdir()?;

    crate::process::process(crate::process::cargo_exe()?)
        .args(&["new", "-q", "--vcs", "none"])
        .arg(tempdir.path().join("a"))
        .cwd(workspace_root)
        .exec()?;

    let manifest = xshell::read_file(tempdir.path().join("a").join("Cargo.toml"))?;
    let author = toml::from_str::<Manifest>(&manifest)?
        .package
        .authors
        .get(0)
        .cloned();
    Ok(author)
}

pub(crate) fn cargo_check_using_current_lockfile_and_cache(
    metadata: &cm::Metadata,
    package: &cm::Package,
    need_dev_deps: bool,
    exclude: &[PkgSpec],
    code: &str,
) -> anyhow::Result<()> {
    let name = {
        let mut rng = rand::thread_rng();
        let suf = (0..16)
            .map(|_| match rng.gen_range(0..=35) {
                n @ 0..=25 => b'a' + n,
                n @ 26..=35 => b'0' + n - 26,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        let suf = str::from_utf8(&suf).expect("should be valid ASCII");
        format!("cargo-equip-check-output-{}", suf)
    };

    let temp_pkg = tempfile::Builder::new()
        .prefix(&name)
        .rand_bytes(0)
        .tempdir()?;

    let cargo_exe = crate::process::cargo_exe()?;

    crate::process::process(&cargo_exe)
        .arg("init")
        .arg("-q")
        .arg("--vcs")
        .arg("none")
        .arg("--bin")
        .arg("--edition")
        .arg(&package.edition)
        .arg("--name")
        .arg(&name)
        .arg(temp_pkg.path())
        .cwd(&metadata.workspace_root)
        .exec()?;

    let orig_manifest =
        std::fs::read_to_string(&package.manifest_path)?.parse::<toml_edit::Document>()?;

    let mut temp_manifest = std::fs::read_to_string(temp_pkg.path().join("Cargo.toml"))?
        .parse::<toml_edit::Document>()?;

    temp_manifest["dependencies"] = orig_manifest["dependencies"].clone();
    temp_manifest["dev-dependencies"] = orig_manifest["dev-dependencies"].clone();

    let renames = package
        .dependencies
        .iter()
        .filter(|cm::Dependency { kind, .. }| {
            [cm::DependencyKind::Normal, cm::DependencyKind::Development].contains(kind)
        })
        .flat_map(|cm::Dependency { rename, .. }| rename)
        .collect::<HashSet<_>>();

    let remove_excluded = |table: &mut toml_edit::Table| {
        for name_in_toml in metadata
            .resolve
            .as_ref()
            .expect("`resolve` is `null`")
            .nodes
            .iter()
            .find(|cm::Node { id, .. }| *id == package.id)
            .expect("should contain")
            .deps
            .iter()
            .filter(|cm::NodeDep { pkg, .. }| !exclude.iter().any(|s| s.matches(&metadata[pkg])))
            .map(|cm::NodeDep { name, pkg, .. }| {
                if renames.contains(&name) {
                    name
                } else {
                    &metadata[pkg].name
                }
            })
        {
            table.remove(name_in_toml);
        }
    };

    if let toml_edit::Item::Table(table) = &mut temp_manifest["dependencies"] {
        remove_excluded(table);
    }
    if let toml_edit::Item::Table(table) = &mut temp_manifest["dev-dependencies"] {
        remove_excluded(table);
    }

    std::fs::write(
        temp_pkg.path().join("Cargo.toml"),
        temp_manifest.to_string(),
    )?;

    std::fs::create_dir(temp_pkg.path().join("src").join("bin"))?;
    std::fs::create_dir(temp_pkg.path().join("examples"))?;
    std::fs::write(
        if need_dev_deps {
            temp_pkg.path().join("examples")
        } else {
            temp_pkg.path().join("src").join("bin")
        }
        .join(name)
        .with_extension("rs"),
        code,
    )?;

    std::fs::remove_file(temp_pkg.path().join("src").join("main.rs"))?;

    std::fs::copy(
        metadata.workspace_root.join("Cargo.lock"),
        temp_pkg.path().join("Cargo.lock"),
    )?;

    crate::process::process(cargo_exe)
        .arg("check")
        .arg("--target-dir")
        .arg(&metadata.target_directory)
        .arg("--manifest-path")
        .arg(temp_pkg.path().join("Cargo.toml"))
        .arg("--all-targets")
        .arg("--offline")
        .cwd(&metadata.workspace_root)
        .exec()?;

    temp_pkg.close()?;
    Ok(())
}

pub(crate) trait MetadataExt {
    fn exactly_one_bin_like_target(&self) -> anyhow::Result<(&cm::Target, &cm::Package)>;
    fn bin_target_by_name<'a>(
        &'a self,
        name: &str,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)>;
    fn example_target_by_name<'a>(
        &'a self,
        name: &str,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)>;
    fn bin_like_target_by_src_path<'a>(
        &'a self,
        src_path: &Path,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)>;
    fn libs_to_bundle<'a>(
        &'a self,
        package_id: &'a cm::PackageId,
        need_dev_deps: bool,
        cargo_udeps_outcome: &HashSet<String>,
        exclude: &[PkgSpec],
    ) -> anyhow::Result<BTreeMap<&'a cm::PackageId, (&'a cm::Target, String)>>;
    fn dep_lib_by_extern_crate_name<'a>(
        &'a self,
        package_id: &cm::PackageId,
        extern_crate_name: &str,
    ) -> Option<&cm::Package>;
    fn libs_with_extern_crate_names(
        &self,
        package_id: &cm::PackageId,
        only: &HashSet<&cm::PackageId>,
    ) -> anyhow::Result<BTreeMap<&cm::PackageId, String>>;
}

impl MetadataExt for cm::Metadata {
    fn exactly_one_bin_like_target(&self) -> anyhow::Result<(&cm::Target, &cm::Package)> {
        match &*targets_in_ws(self)
            .filter(|(cm::Target { kind, .. }, _)| {
                [&["bin".to_owned()][..], &["example".to_owned()][..]].contains(&&**kind)
            })
            .collect::<Vec<_>>()
        {
            [] => bail!("no bin/example target in this workspace"),
            [t] => Ok(*t),
            [ts @ ..] => bail!(
                "could not determine which binary to choose. Use the `--bin` option, `--example` \
                 option, or `--src` option to specify a binary.\n\
                 available binaries: {}\n\
                 note: currently `cargo-equip` does not support the `default-run` manifest key.",
                ts.iter()
                    .map(|(cm::Target { name, .. }, _)| name)
                    .format(", "),
            ),
        }
    }

    fn bin_target_by_name<'a>(
        &'a self,
        name: &str,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)> {
        target_by_kind_and_name(self, "bin", name)
    }

    fn example_target_by_name<'a>(
        &'a self,
        name: &str,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)> {
        target_by_kind_and_name(self, "example", name)
    }

    fn bin_like_target_by_src_path<'a>(
        &'a self,
        src_path: &Path,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)> {
        match *targets_in_ws(self)
            .filter(|(t, _)| t.src_path == src_path)
            .collect::<Vec<_>>()
        {
            [] => bail!(
                "`{}` is not the main source file of any bin targets in this workspace ",
                src_path.display(),
            ),
            [bin] => Ok(bin),
            [..] => bail!(
                "multiple bin targets which `src_path` is `{}`",
                src_path.display(),
            ),
        }
    }

    fn libs_to_bundle<'a>(
        &'a self,
        package_id: &'a cm::PackageId,
        need_dev_deps: bool,
        cargo_udeps_outcome: &HashSet<String>,
        exclude: &[PkgSpec],
    ) -> anyhow::Result<BTreeMap<&'a cm::PackageId, (&'a cm::Target, String)>> {
        let package = &self[package_id];

        let renames = package
            .dependencies
            .iter()
            .filter(|cm::Dependency { kind, .. }| {
                [cm::DependencyKind::Normal, cm::DependencyKind::Development].contains(kind)
            })
            .flat_map(|cm::Dependency { rename, .. }| rename)
            .collect::<HashSet<_>>();

        let preds = {
            let rustc_exe = crate::process::cargo_exe()?
                .with_file_name("rustc")
                .with_extension(env::consts::EXE_EXTENSION);

            let preds = crate::process::process(rustc_exe)
                .args(&["--print", "cfg"])
                .cwd(package.manifest_path.with_file_name(""))
                .read(true)?;

            preds
                .lines()
                .flat_map(cfg_expr::Expression::parse) // https://github.com/EmbarkStudios/cfg-expr/blob/25290dba689ce3f3ab589926ba545875f048c130/src/expr/parser.rs#L180-L195
                .collect::<Vec<_>>()
        };
        let preds = preds
            .iter()
            .flat_map(cfg_expr::Expression::predicates)
            .collect::<Vec<_>>();

        let cm::Resolve { nodes, .. } = self
            .resolve
            .as_ref()
            .with_context(|| "`resolve` is `null`")?;
        let nodes = nodes.iter().map(|n| (&n.id, n)).collect::<HashMap<_, _>>();

        let satisfies = |node_dep: &cm::NodeDep, accepts_dev: bool| -> _ {
            if exclude.iter().any(|s| s.matches(&self[&node_dep.pkg])) {
                return false;
            }

            let cm::Node { features, .. } = &nodes[&node_dep.pkg];
            let features = features.iter().map(|s| &**s).collect::<HashSet<_>>();

            node_dep
                .dep_kinds
                .iter()
                .any(|cm::DepKindInfo { kind, target, .. }| {
                    (*kind == cm::DependencyKind::Normal
                        || accepts_dev && *kind == cm::DependencyKind::Development)
                        && target
                            .as_ref()
                            .and_then(|target| {
                                cfg_expr::Expression::parse(&target.to_string()).ok()
                            })
                            .map_or(true, |target| {
                                target.eval(|pred| match pred {
                                    cfg_expr::Predicate::Feature(feature) => {
                                        features.contains(feature)
                                    }
                                    pred => preds.contains(pred),
                                })
                            })
                })
        };

        if nodes[package_id]
            .deps
            .iter()
            .any(|cm::NodeDep { dep_kinds, .. }| dep_kinds.is_empty())
        {
            bail!("this tool requires Rust 1.41+ for calculating dependencies");
        }

        let mut deps = nodes[package_id]
            .deps
            .iter()
            .filter(|node_dep| satisfies(node_dep, need_dev_deps))
            .flat_map(|node_dep| {
                let lib_package = &self[&node_dep.pkg];
                let lib_target =
                    lib_package.targets.iter().find(|cm::Target { kind, .. }| {
                        *kind == ["lib".to_owned()] || *kind == ["proc-macro".to_owned()]
                    })?;
                let (lib_extern_crate_name, lib_name_in_toml) = if renames.contains(&node_dep.name)
                {
                    (node_dep.name.clone(), &node_dep.name)
                } else {
                    (lib_target.crate_name(), &lib_package.name)
                };
                if cargo_udeps_outcome.contains(lib_name_in_toml) {
                    return None;
                }
                Some((&lib_package.id, (lib_target, lib_extern_crate_name)))
            })
            .chain(
                package
                    .lib_like_target()
                    .map(|lib_target| (package_id, (lib_target, lib_target.crate_name()))),
            )
            .collect::<BTreeMap<_, _>>();

        let all_package_ids = &mut deps.keys().copied().collect::<HashSet<_>>();
        let all_extern_crate_names = &mut deps
            .values()
            .map(|(_, s)| s.clone())
            .collect::<HashSet<_>>();

        while {
            let next = deps
                .iter()
                .filter(|(_, (cm::Target { kind, .. }, _))| *kind == ["lib".to_owned()])
                .map(|(package_id, _)| nodes[package_id])
                .flat_map(|cm::Node { deps, .. }| deps)
                .filter(|node_dep| {
                    satisfies(node_dep, false) && all_package_ids.insert(&node_dep.pkg)
                })
                .flat_map(|cm::NodeDep { pkg, .. }| {
                    let package = &self[pkg];
                    let target = package.targets.iter().find(|cm::Target { kind, .. }| {
                        *kind == ["lib".to_owned()] || *kind == ["proc-macro".to_owned()]
                    })?;
                    let mut extern_crate_name = format!(
                        "__{}_{}",
                        package.name.replace('-', "_"),
                        package
                            .version
                            .to_string()
                            .replace(|c| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9'), "_"),
                    );
                    while !all_extern_crate_names.insert(extern_crate_name.clone()) {
                        extern_crate_name += "_";
                    }
                    Some((&package.id, (target, extern_crate_name)))
                })
                .collect::<Vec<_>>();
            let next_is_empty = next.is_empty();
            deps.extend(next);
            !next_is_empty
        } {}

        Ok(deps)
    }

    fn dep_lib_by_extern_crate_name<'a>(
        &'a self,
        package_id: &cm::PackageId,
        extern_crate_name: &str,
    ) -> Option<&cm::Package> {
        // https://docs.rs/cargo/0.47.0/src/cargo/core/resolver/resolve.rs.html#323-352

        let package = &self[package_id];

        let node = self
            .resolve
            .as_ref()
            .into_iter()
            .flat_map(|cm::Resolve { nodes, .. }| nodes)
            .find(|cm::Node { id, .. }| id == package_id)?;

        let found_explicitly_renamed_one = package
            .dependencies
            .iter()
            .flat_map(|cm::Dependency { rename, .. }| rename)
            .any(|rename| rename == extern_crate_name);

        if found_explicitly_renamed_one {
            Some(
                &self[&node
                    .deps
                    .iter()
                    .find(|cm::NodeDep { name, .. }| name == extern_crate_name)
                    .expect("found the dep in `dependencies`, not in `resolve.deps`")
                    .pkg],
            )
        } else {
            node.dependencies
                .iter()
                .map(|dep_id| &self[dep_id])
                .flat_map(|p| p.targets.iter().map(move |t| (t, p)))
                .find(|(t, _)| {
                    t.crate_name() == extern_crate_name
                        && (*t.kind == ["lib".to_owned()] || *t.kind == ["proc-macro".to_owned()])
                })
                .map(|(_, p)| p)
                .or_else(|| {
                    matches!(package.lib_like_target(), Some(t) if t.crate_name() == extern_crate_name)
                        .then(|| package)
                })
        }
    }

    fn libs_with_extern_crate_names(
        &self,
        package_id: &cm::PackageId,
        only: &HashSet<&cm::PackageId>,
    ) -> anyhow::Result<BTreeMap<&cm::PackageId, String>> {
        let package = &self[package_id];

        let renames = package
            .dependencies
            .iter()
            .flat_map(|cm::Dependency { rename, .. }| rename)
            .collect::<HashSet<_>>();

        let cm::Resolve { nodes, .. } =
            self.resolve.as_ref().with_context(|| "`resolve` is null")?;

        let cm::Node { deps, .. } = nodes
            .iter()
            .find(|cm::Node { id, .. }| id == package_id)
            .with_context(|| "could not find the node")?;

        Ok(deps
            .iter()
            .filter(|cm::NodeDep { pkg, dep_kinds, .. }| {
                matches!(
                    &**dep_kinds,
                    [cm::DepKindInfo {
                        kind: cm::DependencyKind::Normal,
                        ..
                    }]
                ) && only.contains(pkg)
            })
            .flat_map(|cm::NodeDep { name, pkg, .. }| {
                let extern_crate_name = if renames.contains(name) {
                    name.clone()
                } else {
                    self[pkg]
                        .targets
                        .iter()
                        .find(|cm::Target { kind, .. }| {
                            *kind == ["lib".to_owned()] || *kind == ["proc-macro".to_owned()]
                        })?
                        .crate_name()
                };
                Some((pkg, extern_crate_name))
            })
            .collect())
    }
}

fn target_by_kind_and_name<'a>(
    metadata: &'a cm::Metadata,
    kind: &str,
    name: &str,
) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)> {
    match *targets_in_ws(metadata)
        .filter(|(t, _)| t.name == name && t.kind == [kind.to_owned()])
        .collect::<Vec<_>>()
    {
        [] => bail!("no {} target named `{}`", kind, name),
        [target] => Ok(target),
        [..] => bail!(
            "multiple {} targets named `{}` in this workspace",
            kind,
            name,
        ),
    }
}

fn targets_in_ws(metadata: &cm::Metadata) -> impl Iterator<Item = (&cm::Target, &cm::Package)> {
    metadata
        .packages
        .iter()
        .filter(move |cm::Package { id, .. }| metadata.workspace_members.contains(id))
        .flat_map(|p| p.targets.iter().map(move |t| (t, p)))
}

pub(crate) trait PackageExt {
    fn has_custom_build(&self) -> bool;
    fn has_lib(&self) -> bool;
    fn has_proc_macro(&self) -> bool;
    fn lib_like_target(&self) -> Option<&cm::Target>;
    fn manifest_dir(&self) -> &Utf8Path;
    fn read_license_text(&self, cache_dir: &Path) -> anyhow::Result<Option<String>>;
}

impl PackageExt for cm::Package {
    fn has_custom_build(&self) -> bool {
        self.targets.iter().any(TargetExt::is_custom_build)
    }

    fn has_lib(&self) -> bool {
        self.targets.iter().any(TargetExt::is_lib)
    }

    fn has_proc_macro(&self) -> bool {
        self.targets.iter().any(TargetExt::is_proc_macro)
    }

    fn lib_like_target(&self) -> Option<&cm::Target> {
        self.targets.iter().find(|cm::Target { kind, .. }| {
            [&["lib".to_owned()][..], &["proc-macro".to_owned()][..]].contains(&&**kind)
        })
    }

    fn manifest_dir(&self) -> &Utf8Path {
        self.manifest_path.parent().expect("should not be empty")
    }

    fn read_license_text(&self, cache_dir: &Path) -> anyhow::Result<Option<String>> {
        license::license_file(self, cache_dir)
    }
}

pub(crate) trait PackageIdExt {
    fn mask_path(&self) -> String;
}

impl PackageIdExt for cm::PackageId {
    fn mask_path(&self) -> String {
        if_chain! {
            if let [s1, s2] = *self.repr.split(" (path+").collect::<Vec<_>>();
            if s2.ends_with(')');
            then {
                format!(
                    "{} (path+{})",
                    s1,
                    s2.chars().map(|_| '█').collect::<String>(),
                )
            } else {
                self.repr.clone()
            }
        }
    }
}

pub(crate) trait TargetExt {
    fn is_example(&self) -> bool;
    fn is_custom_build(&self) -> bool;
    fn is_lib(&self) -> bool;
    fn is_proc_macro(&self) -> bool;
    fn crate_name(&self) -> String;
    fn target_option(&self) -> [&str; 2];
}

impl TargetExt for cm::Target {
    fn is_example(&self) -> bool {
        self.kind == ["example".to_owned()]
    }

    fn is_custom_build(&self) -> bool {
        self.kind == ["custom-build".to_owned()]
    }

    fn is_lib(&self) -> bool {
        self.kind == ["lib".to_owned()]
    }

    fn is_proc_macro(&self) -> bool {
        self.kind == ["proc-macro".to_owned()]
    }

    fn crate_name(&self) -> String {
        self.name.replace('-', "_")
    }

    fn target_option(&self) -> [&str; 2] {
        if self.is_example() {
            ["--example", &self.name]
        } else {
            ["--bin", &self.name]
        }
    }
}
