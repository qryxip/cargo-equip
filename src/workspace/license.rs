use crate::workspace::{PackageExt as _, SourceExt as _};
use anyhow::{anyhow, bail, Context as _};
use cargo_metadata as cm;
use serde::Deserialize;
use std::path::Path;

pub(super) fn license_file(
    package: &cm::Package,
    cache_dir: &Path,
) -> anyhow::Result<Option<String>> {
    let license = package
        .license
        .as_deref()
        .with_context(|| format!("`{}`: missing `license`", package.id))?;

    let license = normalize(license);

    let license = spdx::Expression::parse(license).map_err(|e| {
        anyhow!("{}", e).context(format!("`{}`: could not parse `license`", package.id))
    })?;

    let is = |name| license.evaluate(|r| r.license.id() == spdx::license_id(name));

    if is("CC0-1.0") || is("Unlicense") {
        return Ok(None);
    }

    let file_names = if is("MIT") {
        &["LICENSE-MIT", "LICENSE"]
    } else if is("Apache-2.0") {
        &["LICENSE-APACHE", "LICENSE"]
    } else {
        bail!("`{}`: unsupported license: `{}`", package.id, license);
    };

    find(package.manifest_dir().as_ref(), file_names)
        .unwrap_or_else(|| {
            if let Some(source) = &package.source {
                let (repository, sha1) = if let Some((repository, sha1)) = source.rev_git() {
                    (repository, sha1.to_owned())
                } else {
                    let repository = package.repository.as_deref().with_context(|| {
                        format!(
                            "could not retrieve the license file of `{}`: missing `repository` \
                             field",
                            package.id,
                        )
                    })?;
                    let sha1 = read_git_sha1(package)?;
                    (repository, sha1)
                };

                let cache_path = &cache_dir
                    .join("license-files")
                    .join(&package.name)
                    .join(&sha1);

                if cache_path.exists() {
                    return xshell::read_file(cache_path).map_err(Into::into);
                }

                let content =
                    find_in_git_repos(repository, &sha1, file_names)?.with_context(|| {
                        format!(
                            "could not retrieve the license file of `{}`: could not find {:?}",
                            package.id, file_names,
                        )
                    })?;

                xshell::mkdir_p(cache_path.with_file_name(""))?;
                xshell::write_file(cache_path, &content)?;

                Ok(content)
            } else {
                let repository = package
                    .manifest_dir()
                    .ancestors()
                    .find(|p| p.join(".git").is_dir())
                    .with_context(|| {
                        format!(
                            "could not find a license file in `{}`",
                            package.manifest_dir()
                        )
                    })?;
                find(repository.as_ref(), file_names)
                    .with_context(|| format!("could not find a license file in `{}`", repository))?
            }
        })
        .map(Some)
}

fn normalize(license: &str) -> &str {
    if license == "MIT/Apache-2.0" {
        "MIT OR Apache-2.0"
    } else if license == "Apache-2.0/MIT" {
        "Apache-2.0 OR MIT"
    } else {
        license
    }
}

fn read_git_sha1(package: &cm::Package) -> anyhow::Result<String> {
    let json = &xshell::read_file(package.manifest_dir().join(".cargo_vcs_info.json"))?;
    let CargoVcsInfo {
        git: CargoVcsInfoGit { sha1 },
    } = serde_json::from_str(json).with_context(|| {
        format!(
            "could not retrieve the license file of `{}`: this package does not seem to come from \
             a Git repository",
            package.id,
        )
    })?;
    return Ok(sha1);

    #[derive(Deserialize)]
    struct CargoVcsInfo {
        git: CargoVcsInfoGit,
    }

    #[derive(Deserialize)]
    struct CargoVcsInfoGit {
        sha1: String,
    }
}

fn find_in_git_repos(url: &str, sha1: &str, file_names: &[&str]) -> anyhow::Result<Option<String>> {
    let tempdir = tempfile::Builder::new()
        .prefix("cargo-equip-git-clone-")
        .tempdir()?;

    crate::process::process("git")
        .args(&["clone", "--no-checkout", "--filter", "blob:none", url, "."])
        .cwd(tempdir.path())
        .exec()?;

    crate::process::process("git")
        .args(&["switch", "-d", sha1])
        .cwd(tempdir.path())
        .exec()?;

    let result = find(tempdir.path(), file_names).transpose();
    tempdir.close()?;
    result
}

fn find(dir: &Path, file_names: &[&str]) -> Option<anyhow::Result<String>> {
    let path = file_names
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())?;
    Some(xshell::read_file(path).map_err(Into::into))
}
