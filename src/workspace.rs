use anyhow::{bail, Context as _};
use cargo_metadata as cm;
use easy_ext::ext;
use itertools::Itertools as _;
use serde::Deserialize;
use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
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
    let temp_pkg = tempfile::Builder::new().prefix("cargo-equip-").tempdir()?;

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
        .arg("cargo-equip-check-output")
        .arg(temp_pkg.path())
        .cwd(&metadata.workspace_root)
        .exec()?;

    let orig_manifest =
        std::fs::read_to_string(&package.manifest_path)?.parse::<toml_edit::Document>()?;

    let mut temp_manifest = std::fs::read_to_string(temp_pkg.path().join("Cargo.toml"))?
        .parse::<toml_edit::Document>()?;

    temp_manifest["dependencies"] = orig_manifest["dependencies"].clone();

    std::fs::write(
        temp_pkg.path().join("Cargo.toml"),
        temp_manifest.to_string(),
    )?;

    std::fs::write(temp_pkg.path().join("src").join("main.rs"), code)?;

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

    pub(crate) fn dep_lib_by_extern_crate_name<'a>(
        &'a self,
        package_id: &cm::PackageId,
        extern_crate_name: &str,
    ) -> anyhow::Result<(&cm::Target, &cm::Package)> {
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
            let package = &self[&node
                .deps
                .iter()
                .find(|cm::NodeDep { name, .. }| name == extern_crate_name)
                .expect("found the dep in `dependencies`, not in `resolve.deps`")
                .pkg];

            let lib = package
                .targets
                .iter()
                .find(|cm::Target { kind, .. }| *kind == ["lib".to_owned()])
                .with_context(|| {
                    format!(
                        "`{}` is resolved as `{}` but it has no `lib` target",
                        extern_crate_name, package.name,
                    )
                })?;

            Ok((lib, package))
        } else {
            node.dependencies
                .iter()
                .map(|dep_id| &self[dep_id])
                .flat_map(|p| p.targets.iter().map(move |t| (t, p)))
                .find(|(t, _)| t.name == extern_crate_name && *t.kind == ["lib".to_owned()])
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
    pub(crate) fn parse_lib_metadata(&self) -> anyhow::Result<LibPackageMetadata> {
        #[derive(Deserialize)]
        #[serde(rename_all = "kebab-case")]
        struct PackageMetadata {
            cargo_equip_lib: Option<LibPackageMetadata>,
        }

        let PackageMetadata { cargo_equip_lib } = serde_json::from_value(self.metadata.clone())
            .with_context(|| {
                format!(
                    "could not parse `package.metadata.cargo-equip-lib` at `{}`",
                    self.manifest_path.display(),
                )
            })?;

        if let Some(cargo_equip_lib) = cargo_equip_lib {
            Ok(cargo_equip_lib)
        } else {
            bail!(
                "missing `package.metadata.cargo-equip-lib` in `{}`",
                self.manifest_path.display(),
            );
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct LibPackageMetadata {
    pub(crate) mod_dependencies: HashMap<String, BTreeSet<String>>,
}
