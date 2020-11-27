use crate::shell::Shell;
use anyhow::{anyhow, bail, Context as _};
use cargo_metadata as cm;
use itertools::Itertools as _;
use krates::PkgSpec;
use maplit::btreemap;
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

pub(crate) fn execute_build_scripts<'cm>(
    metadata: &'cm cm::Metadata,
    packages_to_bundle: impl IntoIterator<Item = &'cm cm::PackageId>,
    shell: &mut Shell,
) -> anyhow::Result<BTreeMap<&'cm cm::PackageId, PathBuf>> {
    let packages_to_bundle = packages_to_bundle
        .into_iter()
        .map(|id| &metadata[id])
        .filter(|package| {
            package
                .targets
                .iter()
                .any(|cm::Target { kind, .. }| *kind == ["custom-build".to_owned()])
        })
        .collect::<Vec<_>>();

    if packages_to_bundle.is_empty() {
        return Ok(btreemap!());
    }

    let cargo_exe = crate::process::cargo_exe()?;

    let messages = crate::process::process(&cargo_exe)
        .arg("check")
        .arg("--message-format")
        .arg("json")
        .arg("-p")
        .args(
            &packages_to_bundle
                .into_iter()
                .flat_map(|p| vec!["-p".to_owned(), format!("{}:{}", p.name, p.version)])
                .collect::<Vec<_>>(),
        )
        .cwd(&metadata.workspace_root)
        .read_with_status(true, shell)?;

    // TODO: check if ≧ 1.41.0

    let messages =
        cm::Message::parse_stream(Cursor::new(messages)).collect::<Result<Vec<_>, _>>()?;

    Ok(messages
        .into_iter()
        .flat_map(|message| match message {
            cm::Message::BuildScriptExecuted(cm::BuildScript {
                package_id,
                out_dir,
                ..
            }) => Some((&metadata[&package_id].id, out_dir)),
            _ => None,
        })
        .collect())
}

pub(crate) fn get_author(workspace_root: &Path) -> anyhow::Result<String> {
    #[derive(Deserialize)]
    struct Manifest {
        package: ManifestPackage,
    }

    #[derive(Deserialize)]
    struct ManifestPackage {
        authors: [String; 1],
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
    let author = toml::from_str::<Manifest>(&manifest)?.package.authors[0].clone();
    Ok(author)
}

pub(crate) fn cargo_check_using_current_lockfile_and_cache(
    metadata: &cm::Metadata,
    package: &cm::Package,
    exclude: &[PkgSpec],
    code: &str,
) -> anyhow::Result<()> {
    let name = {
        let mut rng = rand::thread_rng();
        let suf = (0..16)
            .map(|_| match rng.gen_range(0, 26 + 10) {
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
    if let toml_edit::Item::Table(dependencies) = &mut temp_manifest["dependencies"] {
        let renames = package
            .dependencies
            .iter()
            .flat_map(|cm::Dependency { rename, .. }| rename.as_ref())
            .collect::<HashSet<_>>();

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
            dependencies.remove(name_in_toml);
        }
    }

    std::fs::write(
        temp_pkg.path().join("Cargo.toml"),
        temp_manifest.to_string(),
    )?;

    std::fs::create_dir(temp_pkg.path().join("src").join("bin"))?;
    std::fs::write(
        temp_pkg
            .path()
            .join("src")
            .join("bin")
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
        .arg("--offline")
        .cwd(&metadata.workspace_root)
        .exec()?;

    temp_pkg.close()?;
    Ok(())
}

pub(crate) trait MetadataExt {
    fn exactly_one_bin_target(&self) -> anyhow::Result<(&cm::Target, &cm::Package)>;
    fn bin_target_by_name<'a>(
        &'a self,
        name: &str,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)>;
    fn bin_target_by_src_path<'a>(
        &'a self,
        src_path: &Path,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)>;
    fn libs_to_bundle<'a>(
        &'a self,
        package_id: &cm::PackageId,
        cargo_udeps_outcome: &HashSet<String>,
        exclude: &[PkgSpec],
    ) -> anyhow::Result<BTreeMap<&'a cm::PackageId, (&'a cm::Target, String)>>;
    fn dep_lib_by_extern_crate_name<'a>(
        &'a self,
        package_id: &cm::PackageId,
        extern_crate_name: &str,
    ) -> anyhow::Result<&cm::Package>;
    fn libs_with_extern_crate_names(
        &self,
        package_id: &cm::PackageId,
        only: &HashSet<&cm::PackageId>,
    ) -> anyhow::Result<BTreeMap<&cm::PackageId, String>>;
}

impl MetadataExt for cm::Metadata {
    fn exactly_one_bin_target(&self) -> anyhow::Result<(&cm::Target, &cm::Package)> {
        match &*bin_targets(self).collect::<Vec<_>>() {
            [] => bail!("no bin target in this workspace"),
            [bin] => Ok(*bin),
            [bins @ ..] => bail!(
                "could not determine which binary to choose. Use the `--bin` option or \
                 `--src` option to specify a binary.\n\
                 available binaries: {}\n\
                 note: currently `cargo-equip` does not support the `default-run` manifest key.",
                bins.iter()
                    .map(|(cm::Target { name, .. }, _)| name)
                    .format(", "),
            ),
        }
    }

    fn bin_target_by_name<'a>(
        &'a self,
        name: &str,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)> {
        match *bin_targets(self)
            .filter(|(t, _)| t.name == name)
            .collect::<Vec<_>>()
        {
            [] => bail!("no bin target named `{}`", name),
            [bin] => Ok(bin),
            [..] => bail!("multiple bin targets named `{}` in this workspace", name),
        }
    }

    fn bin_target_by_src_path<'a>(
        &'a self,
        src_path: &Path,
    ) -> anyhow::Result<(&'a cm::Target, &'a cm::Package)> {
        match *bin_targets(self)
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
        package_id: &cm::PackageId,
        cargo_udeps_outcome: &HashSet<String>,
        exclude: &[PkgSpec],
    ) -> anyhow::Result<BTreeMap<&'a cm::PackageId, (&'a cm::Target, String)>> {
        let package = &self[package_id];

        let renames = package
            .dependencies
            .iter()
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
            cfg_expr::Expression::parse(&format!("all({})", preds.lines().format(",")))?
        };
        let preds = preds.predicates().collect::<Vec<_>>();

        let cm::Resolve { nodes, .. } = self
            .resolve
            .as_ref()
            .with_context(|| "`resolve` is `null`")?;
        let nodes = nodes.iter().map(|n| (&n.id, n)).collect::<HashMap<_, _>>();

        let satisfies = |node_dep: &cm::NodeDep| -> _ {
            if exclude.iter().any(|s| s.matches(&self[&node_dep.pkg])) {
                return false;
            }

            let cm::Node { features, .. } = &nodes[&node_dep.pkg];
            let features = features.iter().map(|s| &**s).collect::<HashSet<_>>();

            node_dep
                .dep_kinds
                .iter()
                .any(|cm::DepKindInfo { kind, target, .. }| {
                    *kind == cm::DependencyKind::Normal
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
            .filter(|node_dep| satisfies(node_dep))
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
                    (lib_target.name.replace('-', "_"), &lib_package.name)
                };
                if cargo_udeps_outcome.contains(lib_name_in_toml) {
                    return None;
                }
                Some((&lib_package.id, (lib_target, lib_extern_crate_name)))
            })
            .collect::<BTreeMap<_, _>>();

        let all_package_ids = &mut deps.keys().copied().collect::<HashSet<_>>();
        let all_extern_crate_names = &mut deps
            .values()
            .map(|(_, s)| s.clone())
            .collect::<HashSet<_>>();

        while {
            let next = deps
                .keys()
                .map(|package_id| nodes[package_id])
                .flat_map(|cm::Node { deps, .. }| deps)
                .filter(|node_dep| satisfies(node_dep) && all_package_ids.insert(&node_dep.pkg))
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
    ) -> anyhow::Result<&cm::Package> {
        // https://docs.rs/cargo/0.47.0/src/cargo/core/resolver/resolve.rs.html#323-352

        let package = &self[package_id];

        let node = self
            .resolve
            .as_ref()
            .into_iter()
            .flat_map(|cm::Resolve { nodes, .. }| nodes)
            .find(|cm::Node { id, .. }| id == package_id)
            .with_context(|| format!("`{}` not found in the dependency graph", package_id))?;

        let found_explicitly_renamed_one = package
            .dependencies
            .iter()
            .flat_map(|cm::Dependency { rename, .. }| rename)
            .any(|rename| rename == extern_crate_name);

        if found_explicitly_renamed_one {
            Ok(&self[&node
                .deps
                .iter()
                .find(|cm::NodeDep { name, .. }| name == extern_crate_name)
                .expect("found the dep in `dependencies`, not in `resolve.deps`")
                .pkg])
        } else {
            node.dependencies
                .iter()
                .map(|dep_id| &self[dep_id])
                .flat_map(|p| p.targets.iter().map(move |t| (t, p)))
                .find(|(t, _)| {
                    t.name == extern_crate_name && *t.kind == ["lib".to_owned()]
                        || *t.kind == ["proc-macro".to_owned()]
                })
                .map(|(_, p)| p)
                .with_context(|| {
                    format!(
                        "no external library found which `extern_crate_name` is `{}`",
                        extern_crate_name,
                    )
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
                        .name
                        .replace('-', "_")
                };
                Some((pkg, extern_crate_name))
            })
            .collect())
    }
}

fn bin_targets(metadata: &cm::Metadata) -> impl Iterator<Item = (&cm::Target, &cm::Package)> {
    metadata
        .packages
        .iter()
        .filter(move |cm::Package { id, .. }| metadata.workspace_members.contains(id))
        .flat_map(|p| p.targets.iter().map(move |t| (t, p)))
        .filter(|(cm::Target { kind, .. }, _)| *kind == ["bin".to_owned()])
}

pub(crate) trait PackageExt {
    fn read_license_text(&self) -> anyhow::Result<Option<String>>;
}

impl PackageExt for cm::Package {
    fn read_license_text(&self) -> anyhow::Result<Option<String>> {
        let mut license = self
            .license
            .as_deref()
            .with_context(|| format!("`{}`: missing `license`", self.id))?;

        if license == "MIT/Apache-2.0" {
            license = "MIT OR Apache-2.0";
        }
        if license == "Apache-2.0/MIT" {
            license = "Apache-2.0 OR MIT";
        }

        let license = spdx::Expression::parse(license).map_err(|e| {
            anyhow!("{}", e).context(format!("`{}`: could not parse `license`", self.id))
        })?;

        let is = |name| license.evaluate(|r| r.license.id() == spdx::license_id(name));

        let read_license_file = |file_names: &[&str]| -> _ {
            let path = file_names
                .iter()
                .map(|name| self.manifest_path.with_file_name(name))
                .find(|path| path.exists())
                .with_context(|| format!("`{}`: could not find the license file", self.id))?;
            xshell::read_file(path).map_err(anyhow::Error::from)
        };

        if is("CC0-1.0") || is("Unlicense") {
            Ok(None)
        } else if is("MIT") {
            read_license_file(&["LICENSE-MIT", "LICENSE"]).map(Some)
        } else if is("Apache-2.0") {
            read_license_file(&["LICENSE-APACHE", "LICENSE"]).map(Some)
        } else {
            bail!("`{}`: unsupported license: `{}`", self.id, license);
        }
    }
}
