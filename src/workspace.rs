use anyhow::{bail, Context as _};
use cargo_metadata as cm;
use easy_ext::ext;
use itertools::Itertools as _;
use maplit::hashset;
use once_cell::sync::Lazy;
use rand::Rng as _;
use std::{
    collections::{BTreeMap, HashSet},
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

pub(crate) fn cargo_check_using_current_lockfile_and_cache(
    metadata: &cm::Metadata,
    package: &cm::Package,
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
            .filter(|cm::NodeDep { pkg, .. }| !metadata[pkg].is_available_on_atcoder_or_codingame())
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

#[ext(MetadataExt)]
impl cm::Metadata {
    pub(crate) fn exactly_one_bin_target(&self) -> anyhow::Result<(&cm::Target, &cm::Package)> {
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

    pub(crate) fn bin_target_by_name<'a>(
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

    pub(crate) fn bin_target_by_src_path<'a>(
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

    pub(crate) fn deps_to_bundle<'a>(
        &'a self,
        package_id: &cm::PackageId,
        cargo_udeps_outcome: &HashSet<String>,
    ) -> anyhow::Result<BTreeMap<&'a cm::PackageId, (&'a cm::Target, String)>> {
        let package = &self[package_id];

        let renames = package
            .dependencies
            .iter()
            .flat_map(|cm::Dependency { rename, .. }| rename)
            .collect::<HashSet<_>>();

        let cm::Resolve { nodes, .. } = self
            .resolve
            .as_ref()
            .with_context(|| "`resolve` is `null`")?;

        let mut deps = nodes
            .iter()
            .find(|cm::Node { id, .. }| id == package_id)
            .with_context(|| format!("`{}` not found in the dependency graph", package_id))?
            .deps
            .iter()
            .map(
                |cm::NodeDep {
                     name,
                     pkg,
                     dep_kinds,
                     ..
                 }| {
                    if dep_kinds.is_empty() {
                        bail!("`dep_kind` is empty. this tool requires Rust 1.41+");
                    }
                    if dep_kinds
                        .iter()
                        .any(|cm::DepKindInfo { kind, .. }| *kind == cm::DependencyKind::Normal)
                    {
                        let lib_package = &self[pkg];
                        let lib_target = lib_package
                            .targets
                            .iter()
                            .find(|cm::Target { kind, .. }| *kind == ["lib".to_owned()])
                            .with_context(|| format!("`{}` has no `lib` target", pkg))?;
                        let (lib_extern_crate_name, lib_name_in_toml) = if renames.contains(name) {
                            (name.clone(), name)
                        } else {
                            (lib_target.name.replace('-', "_"), &lib_package.name)
                        };
                        Ok(
                            if cargo_udeps_outcome.contains(lib_name_in_toml)
                                || lib_package.is_available_on_atcoder_or_codingame()
                            {
                                None
                            } else {
                                Some((&lib_package.id, (lib_target, lib_extern_crate_name)))
                            },
                        )
                    } else {
                        Ok(None)
                    }
                },
            )
            .flat_map(Result::transpose)
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        let all_package_ids = &mut deps.keys().copied().collect::<HashSet<_>>();
        let all_extern_crate_names = &mut deps
            .values()
            .map(|(_, s)| s.clone())
            .collect::<HashSet<_>>();

        while {
            let next = deps
                .keys()
                .flat_map(|package_id| {
                    nodes
                        .iter()
                        .filter(move |cm::Node { id, .. }| id == *package_id)
                })
                .flat_map(|cm::Node { deps, .. }| deps)
                .filter(|cm::NodeDep { pkg, dep_kinds, .. }| {
                    matches!(
                        &**dep_kinds,
                        [cm::DepKindInfo {
                            kind: cm::DependencyKind::Normal,
                            ..
                        }]
                    ) && !self[pkg].is_available_on_atcoder_or_codingame()
                        && all_package_ids.insert(pkg)
                })
                .flat_map(|cm::NodeDep { pkg, .. }| {
                    let package = &self[pkg];
                    let target = package
                        .targets
                        .iter()
                        .find(|cm::Target { kind, .. }| *kind == ["lib".to_owned()])?;
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

    pub(crate) fn dep_lib_by_extern_crate_name<'a>(
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
                .find(|(t, _)| t.name == extern_crate_name && *t.kind == ["lib".to_owned()])
                .map(|(_, p)| p)
                .with_context(|| {
                    format!(
                        "no external library found which `extern_crate_name` is `{}`",
                        extern_crate_name,
                    )
                })
        }
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

#[ext(PackageExt)]
impl cm::Package {
    pub(crate) fn is_available_on_atcoder_or_codingame(&self) -> bool {
        pub(crate) static NAMES: Lazy<HashSet<&str>> = Lazy::new(|| {
            hashset!(
                "alga",
                "ascii",
                "bitset-fixed",
                "chrono",
                "either",
                "fixedbitset",
                "getrandom",
                "im-rc",
                "indexmap",
                "itertools",
                "itertools-num",
                "lazy_static",
                "libc",
                "libm",
                "maplit",
                "nalgebra",
                "ndarray",
                "num",
                "num-bigint",
                "num-complex",
                "num-derive",
                "num-integer",
                "num-iter",
                "num-rational",
                "num-traits",
                "ordered-float",
                "permutohedron",
                "petgraph",
                "proconio",
                "rand",
                "rand_chacha",
                "rand_core",
                "rand_distr",
                "rand_hc",
                "rand_pcg",
                "regex",
                "rustc-hash",
                "smallvec",
                "superslice",
                "text_io",
                "time",
                "whiteread",
            )
        });

        matches!(&self.source, Some(source) if source.is_crates_io())
            && NAMES.contains(&&*self.name)
    }
}
