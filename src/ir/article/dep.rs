use crate::compile::error::{TypError, TypResult};
use crate::compile::registry::{Key, KeyRegistry};
use crate::config::TypsiteConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Dependency {
    dependency: HashMap<Source, HashSet<UpdatedIndex>>,
}
impl Dependency {
    pub(super) fn from(
        self_slug: Key,
        pure: PureDependency,
        config: &TypsiteConfig<'_>,
        registry: &KeyRegistry,
    ) -> TypResult<Dependency> {
        let mut err = TypError::new(self_slug.clone());
        let dependency = pure
            .dependency
            .into_iter()
            .map(|(source, indexes)| {
                err.ok(Source::from(self_slug.as_ref(), source, config, registry))
                    .map(|source| (source, indexes))
            })
            .collect::<Vec<Option<_>>>();
        err.err_or(|| Dependency {
            dependency: dependency.into_iter().flatten().collect(),
        })
    }
    pub fn unwrap(&self, registry: &KeyRegistry) -> HashMap<Arc<Path>, HashSet<UpdatedIndex>> {
        self.dependency
            .clone()
            .into_iter()
            .filter_map(|(source, indexes)| match source {
                Source::Article(slug) => registry.path(slug.as_ref()).map(|a| (a, indexes)),
                Source::Path(path) => Some((path.clone(), indexes)),
            })
            .collect()
    }
    pub fn articles(&self) -> HashSet<Key> {
        self.dependency
            .iter()
            .filter_map(|(source, _)| source.article())
            .collect()
    }

    pub fn new(dependency: HashMap<Source, HashSet<UpdatedIndex>>) -> Self {
        Self { dependency }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PureDependency {
    #[serde(with = "dependency_serde")]
    dependency: HashMap<PureSource, HashSet<UpdatedIndex>>,
}

impl From<Dependency> for PureDependency {
    fn from(dep: Dependency) -> Self {
        let dependency = dep
            .dependency
            .into_iter()
            .map(|(k, v)| (PureSource::from(k), v))
            .collect();
        Self { dependency }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UpdatedIndex {
    BodyRewriter(usize),
    MetaRewriter(String, usize),
    Embed(usize),
}

#[derive(Debug, Clone)]
pub enum Indexes {
    All,
    Some(HashSet<usize>),
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum Source {
    Article(Key), // slug
    Path(Arc<Path>),
}

impl Source {
    pub(super) fn from(
        self_slug: &str,
        pure: PureSource,
        config: &TypsiteConfig,
        registry: &KeyRegistry,
    ) -> Result<Self> {
        match pure {
            PureSource::Article(slug) => {
                Ok(Source::Article(registry.know(slug, "Source", self_slug)?))
            }
            PureSource::Path(path) => {
                Ok(Source::Path(config.path_ref(&path).with_context(|| {
                    format!("Path {path:?} not found in {self_slug}")
                })?))
            }
        }
    }
    pub fn article(&self) -> Option<Key> {
        match self {
            Source::Article(key) => Some(key.clone()),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum PureSource {
    Article(String), // slug
    Path(PathBuf),
}

impl PureSource {
    pub fn from(source: Source) -> Self {
        match source {
            Source::Article(slug) => PureSource::Article(slug.to_string()),
            Source::Path(path) => PureSource::Path(path.to_path_buf()),
        }
    }
}

mod dependency_serde {
    use super::*;
    use serde::de::{Error, MapAccess, Visitor};
    use serde::ser::SerializeMap;
    use serde::{Deserializer, Serializer};
    use std::fmt;
    pub fn serialize<S>(
        map: &HashMap<PureSource, HashSet<UpdatedIndex>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map_serializer = serializer.serialize_map(Some(map.len()))?;
        for (k, v) in map {
            let key_str = match k {
                PureSource::Article(slug) => format!("Article::{slug}"),
                PureSource::Path(path) => {
                    format!("Path::{}", path.display().to_string().replace('\\', "/"))
                }
            };
            map_serializer.serialize_entry(&key_str, v)?;
        }
        map_serializer.end()
    }

    pub fn deserialize<'ce, D>(
        deserializer: D,
    ) -> Result<HashMap<PureSource, HashSet<UpdatedIndex>>, D::Error>
    where
        D: Deserializer<'ce>,
    {
        struct MapVisitor;
        impl<'ce> Visitor<'ce> for MapVisitor {
            type Value = HashMap<PureSource, HashSet<UpdatedIndex>>;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map from Source to HashSet<Index>")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'ce>,
            {
                let mut dependencies = HashMap::new();
                while let Some((key_str, value)) =
                    map.next_entry::<String, HashSet<UpdatedIndex>>()?
                {
                    let source = if let Some(slug) = key_str.strip_prefix("Article::") {
                        PureSource::Article(slug.to_string())
                    } else if let Some(path_str) = key_str.strip_prefix("Path::") {
                        PureSource::Path(PathBuf::from(
                            path_str.replace('/', std::path::MAIN_SEPARATOR_STR),
                        ))
                    } else {
                        return Err(Error::custom(format!(
                            "Invalid Source key format: {key_str}"
                        )));
                    };
                    dependencies.insert(source, value);
                }
                Ok(dependencies)
            }
        }
        deserializer.deserialize_map(MapVisitor)
    }
}
