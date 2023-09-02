use crate::{
    process::ProcessBuilderExt as _,
    workspace::{PackageExt as _, SourceExt as _},
    User,
};
use anyhow::{anyhow, Context as _};
use cargo_metadata as cm;
use cargo_util::ProcessBuilder;
use maplit::btreeset;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::{btree_map, BTreeMap, BTreeSet},
    path::Path,
};

pub(super) fn read_non_unlicense_license_file(
    package: &cm::Package,
    mine: &[User],
    cache_dir: &Path,
) -> anyhow::Result<Option<String>> {
    if !mine.is_empty() {
        let users = users(package, cache_dir)?;
        if mine.iter().any(|u| users.contains(u)) {
            return Ok(None);
        }
    }

    return read(package, cache_dir).map_err(|causes| {
        let err = anyhow!(
            "could not read the license file of `{}`.\n\
             note: cargo-equip no longer reads `package.authors` to skip Copyright and License \
             Notices.\n      \
             instead, add `--mine github.com/{{your username}}` to the arguments",
            package.id
        );
        let mut causes = causes.into_iter();
        if let Some(cause) = causes.next() {
            causes
                .fold(anyhow!("{}", cause), |e, s| e.context(s))
                .context(err)
        } else {
            err
        }
    });

    #[derive(Deserialize, Serialize)]
    struct Owners {
        #[serde(rename = "crates.io")]
        crates_io: BTreeMap<String, BTreeMap<Version, BTreeSet<String>>>,
        #[serde(rename = "github.com")]
        github_com: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
        #[serde(rename = "gitlab.com")]
        gitlab_com: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    }
}

fn users(package: &cm::Package, cache_dir: &Path) -> anyhow::Result<BTreeSet<User>> {
    let path = &cache_dir.join("owners.json");
    let cur_cache = if path.exists() {
        serde_json::from_str(&cargo_util::paths::read(path)?)?
    } else {
        CachedUsers::default()
    };
    let mut cache = cur_cache.clone();

    let mut users = if let Some(source) = &package.source {
        if source.is_crates_io() {
            match cache
                .crates_io
                .entry(package.name.clone())
                .or_default()
                .entry(package.version.clone())
            {
                btree_map::Entry::Vacant(entry) => {
                    let owners = retrieve_owner_urls(&package.name, cache_dir)?
                        .flat_map(|url| {
                            url.strip_prefix("https://github.com/")
                                .map(ToOwned::to_owned)
                        })
                        .collect();
                    github_users(entry.insert(owners))
                }
                entry @ btree_map::Entry::Occupied(_) => {
                    github_users(entry.or_insert_with(|| unreachable!()))
                }
            }
        } else if let Some([username, _, rev]) = source
            .repr
            .strip_prefix("git+https://github.com/")
            .map(|s| s.split(|c| ['/', '#'].contains(&c)).collect::<Vec<_>>())
            .as_deref()
        {
            cache
                .github_com
                .entry(package.name.clone())
                .or_default()
                .entry((*rev).to_owned())
                .or_default()
                .insert((*username).to_owned());
            btreeset!(User::Github((*username).to_owned()))
        } else if let Some([username, _, rev]) = source
            .repr
            .strip_prefix("git+https://gitlab.com/")
            .map(|s| s.split(|c| ['/', '#'].contains(&c)).collect::<Vec<_>>())
            .as_deref()
        {
            cache
                .gitlab_com
                .entry(package.name.clone())
                .or_default()
                .entry((*rev).to_owned())
                .or_default()
                .insert((*username).to_owned());
            btreeset!(User::GitlabCom((*username).to_owned()))
        } else {
            btreeset!()
        }
    } else {
        btreeset!()
    };

    if cache != cur_cache {
        cargo_util::paths::create_dir_all(cache_dir)?;
        cargo_util::paths::write(path, cache.to_json())?;
    }

    if users.is_empty() {
        if let Some(repository) = &package.repository {
            if let Some(username) = repository.strip_prefix("https://github.com/") {
                users.insert(User::Github(username.to_owned()));
            } else if let Some(username) = repository.strip_prefix("https://gitlab.com/") {
                users.insert(User::GitlabCom(username.to_owned()));
            }
        }
    }

    return Ok(users);

    #[derive(Default, Deserialize, Serialize, Clone, PartialEq)]
    struct CachedUsers {
        #[serde(rename = "crates.io")]
        crates_io: BTreeMap<String, BTreeMap<Version, BTreeSet<String>>>,
        #[serde(rename = "github.com")]
        github_com: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
        #[serde(rename = "gitlab.com")]
        gitlab_com: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    }

    impl CachedUsers {
        fn to_json(&self) -> String {
            serde_json::to_string(self).expect("should not fail")
        }
    }

    fn github_users(names: &BTreeSet<String>) -> BTreeSet<User> {
        names.iter().cloned().map(User::Github).collect()
    }

    fn retrieve_owner_urls(
        package_name: &str,
        cwd: &Path,
    ) -> anyhow::Result<impl Iterator<Item = String>> {
        let url = &format!("https://crates.io/api/v1/crates/{}/owners", package_name);
        let res = &curl(url, cwd)?;
        let KrateOwnersOwners { users } = serde_json::from_str(res)
            .with_context(|| format!("could not parse the output from {}", url))?;
        return Ok(users.into_iter().flat_map(|EncodableOwner { url }| url));

        #[derive(Deserialize)]
        struct KrateOwnersOwners {
            users: Vec<EncodableOwner>,
        }

        #[derive(Deserialize)]
        struct EncodableOwner {
            url: Option<String>,
        }

        fn curl(url: &str, cwd: &Path) -> anyhow::Result<String> {
            let curl_exe = which::which("curl").map_err(|_| anyhow!("command not found: curl"))?;
            ProcessBuilder::new(curl_exe)
                .args(&[url, "-L"])
                .cwd(cwd)
                .read_stdout()
        }
    }
}

fn read(package: &cm::Package, cache_dir: &Path) -> Result<Option<String>, Vec<String>> {
    let license = package
        .license
        .as_deref()
        .ok_or_else(|| vec![format!("`{}`: missing `license`", package.id)])?;

    let license = normalize(license);

    let license = spdx::Expression::parse(license).map_err(|err| {
        vec![
            err.to_string(),
            format!("`{}`: could not parse `license`", package.id),
        ]
    })?;

    let is = |name| license.evaluate(|r| r.license.id() == spdx::license_id(name));

    if is("0BSD") || is("CC0-1.0") || is("Unlicense") {
        return Ok(None);
    }

    let file_names = if is("MIT") {
        &["LICENSE-MIT", "LICENSE"]
    } else if is("Apache-2.0") {
        &["LICENSE-APACHE", "LICENSE"]
    } else {
        return Err(vec![format!(
            "`{}`: unsupported license: `{}`",
            package.id, license,
        )]);
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
                    return cargo_util::paths::read(cache_path);
                }

                let content =
                    find_in_git_repos(repository, &sha1, file_names)?.with_context(|| {
                        format!(
                            "could not retrieve the license file of `{}`: could not find {:?}",
                            package.id, file_names,
                        )
                    })?;

                cargo_util::paths::create_dir_all(cache_path.with_file_name(""))?;
                cargo_util::paths::write(cache_path, &content)?;

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
        .map_err(|e| vec![e.to_string()])
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
    let json =
        &cargo_util::paths::read(package.manifest_dir().join(".cargo_vcs_info.json").as_ref())?;
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

    ProcessBuilder::new("git")
        .args(&["clone", "--no-checkout", "--filter", "blob:none", url, "."])
        .cwd(tempdir.path())
        .exec()?;

    ProcessBuilder::new("git")
        .args(&["switch", "-d", sha1])
        .cwd(tempdir.path())
        .exec()?;

    let result = find(tempdir.path(), file_names).transpose();
    tempdir.close()?;
    result
}

fn find(dir: &Path, file_names: &[&str]) -> Option<anyhow::Result<String>> {
    let path = &file_names
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())?;
    Some(cargo_util::paths::read(path))
}
