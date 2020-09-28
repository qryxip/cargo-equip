use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    str::FromStr,
};

use cargo_metadata as cm;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{de::Error as _, Deserialize, Deserializer};

type PackageModulePath<'cm> = (&'cm cm::PackageId, String);

pub(crate) fn assign_packages<'cm>(
    module_deps: &BTreeMap<ModulePath, BTreeSet<ModulePath>>,
    package_id: &'cm cm::PackageId,
    mut extern_crate_from_name: impl FnMut(&str) -> anyhow::Result<&'cm cm::PackageId>,
) -> anyhow::Result<BTreeMap<PackageModulePath<'cm>, BTreeSet<PackageModulePath<'cm>>>> {
    let mut translate = |node: &_| -> anyhow::Result<_> {
        Ok(match node {
            ModulePath::Crate(CrateModulePath { module_name }) => (package_id, module_name.clone()),
            ModulePath::ExternCrate(ExternCrateModulePath {
                extern_crate_name,
                module_name,
            }) => (
                extern_crate_from_name(extern_crate_name)?,
                module_name.clone(),
            ),
        })
    };

    let mut ret = BTreeMap::<_, BTreeSet<_>>::new();

    for (from, to) in module_deps {
        let ret = ret.entry(translate(from)?).or_default();
        for to in to {
            ret.insert(translate(to)?);
        }
    }

    Ok(ret)
}

pub(crate) fn connect<'cm>(
    graph: &BTreeMap<PackageModulePath<'cm>, BTreeSet<PackageModulePath<'cm>>>,
    start: &BTreeMap<&'cm cm::PackageId, BTreeSet<syn::Ident>>,
) -> Option<BTreeSet<PackageModulePath<'cm>>> {
    let mut ret = start
        .iter()
        .flat_map(|(k, v)| v.iter().map(move |v| (*k, v.to_string())))
        .collect::<BTreeSet<_>>();
    let mut queue = ret.iter().cloned().collect::<VecDeque<_>>();
    loop {
        if let Some(from) = queue.pop_front() {
            if let Some(to) = graph.get(&from) {
                for to in to {
                    if ret.insert(to.clone()) {
                        queue.push_back(to.clone());
                    }
                }
            } else {
                break None;
            }
        } else {
            break Some(ret);
        }
    }
}

#[derive(Debug, Clone, Ord, Eq, PartialOrd, PartialEq, Hash)]
pub(crate) enum ModulePath {
    Crate(CrateModulePath),
    ExternCrate(ExternCrateModulePath),
}

impl<'de> Deserialize<'de> for ModulePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl FromStr for ModulePath {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        static CRATE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"\Acrate::([a-zA-Z0-9_]+)\z").unwrap());
        static EXTERN_CRATE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"\A::([a-zA-Z0-9_]+)::([a-zA-Z0-9_]+)\z").unwrap());

        if let Some(caps) = CRATE.captures(s) {
            Ok(Self::Crate(CrateModulePath {
                module_name: caps[1].to_owned(),
            }))
        } else if let Some(caps) = EXTERN_CRATE.captures(s) {
            Ok(Self::ExternCrate(ExternCrateModulePath {
                extern_crate_name: caps[1].to_owned(),
                module_name: caps[2].to_owned(),
            }))
        } else {
            Err(format!(
                "expected `{}` or `{}`",
                CRATE.as_str(),
                EXTERN_CRATE.as_str(),
            ))
        }
    }
}

impl fmt::Display for ModulePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Crate(CrateModulePath { module_name }) => write!(f, "\"crate::{}\"", module_name),
            Self::ExternCrate(ExternCrateModulePath {
                extern_crate_name,
                module_name,
            }) => write!(f, "\"::{}::{}\"", extern_crate_name, module_name),
        }
    }
}

#[derive(Debug, Clone, Ord, Eq, PartialOrd, PartialEq, Hash)]
pub(crate) struct CrateModulePath {
    module_name: String,
}

impl<'de> Deserialize<'de> for CrateModulePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl FromStr for CrateModulePath {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        static CRATE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"\Acrate::([a-zA-Z0-9_]+)\z").unwrap());

        if let Some(caps) = CRATE.captures(s) {
            Ok(Self {
                module_name: caps[1].to_owned(),
            })
        } else {
            Err(format!("expected `{}`", CRATE.as_str()))
        }
    }
}

#[derive(Debug, Clone, Ord, Eq, PartialOrd, PartialEq, Hash)]
pub(crate) struct ExternCrateModulePath {
    extern_crate_name: String,
    module_name: String,
}

#[cfg(test)]
mod tests {
    use crate::mod_dep::ModulePath;

    #[test]
    fn parse_module_path() {
        fn parse(s: &str) -> Result<(), ()> {
            s.parse::<ModulePath>().map(|_| ()).map_err(|_| ())
        }

        assert!(parse("::library::module").is_ok());
        assert!(parse("::library::module::module").is_err());
        assert!(parse("library::module").is_err());
    }
}
