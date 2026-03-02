use crate::compile::compiler::PathBufs;
use crate::compile::registry::{Key, KeyRegistry};
use crate::config::TypsiteConfig;
use crate::ir::article::dep::UpdatedIndex;
use crate::ir::article::Article;
use crate::util::error::{log_err, log_err_or_ok};
use crate::util::fs::{remove_file_ignore, write_into_file};
use crate::util::path::relative_path;
use crate::walk_glob;
use anyhow::*;
use glob::glob;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub struct RevDeps {
    deps_path: PathBuf,
    cache: HashMap<Arc<Path>, HashSet<Key>>,
    deps: HashMap<Key, HashMap<Arc<Path>, HashSet<UpdatedIndex>>>,
}

impl<'a> RevDeps {
    pub fn load(
        config: &'a TypsiteConfig<'a>,
        cache_path: &Path,
        deleted: &PathBufs,
        registry: &mut KeyRegistry,
    ) -> Self {
        let deps_path = cache_path.join("deps");
        // Remove deleted dep files
        deleted.iter().for_each(|path| {
            let path = PathBuf::from(format!("{}.dep", path.display()));
            remove_file_ignore(path);
        });

        let mut cache = HashMap::new();
        let deps = walk_glob!("{}/**/*.dep", deps_path.display())
            .par_bridge()
            .map(|dep_path| {
                std::fs::read_to_string(&dep_path)
                    .context("Failed to read dep file")
                    .and_then(|dep| {
                        let mut path = relative_path(&deps_path, &dep_path)
                            .context("Failed to convert path")?;
                        path.set_extension("");
                        serde_json::from_str::<HashSet<String>>(&dep)
                            .with_context(|| format!("Failed to parse dep file {dep_path:?}"))
                            .map(|dep| (path, dep))
                    })
            })
            .collect::<Vec<Result<(PathBuf, HashSet<String>)>>>()
            .into_iter()
            .filter_map(log_err_or_ok)
            .filter_map(|(path, dep)| {
                let path = config.path_ref(&path).or_else(|| {
                    config
                        .path_to_slug(&path)
                        .ok()
                        .and_then(|slug| registry.path(&slug))
                });
                let dep = dep
                    .into_iter()
                    .filter_map(|slug| registry.slug(slug.as_str()))
                    .collect();
                path.map(|path| (path, dep))
            });
        cache.extend(deps);
        Self {
            deps_path,
            deps: HashMap::new(),
            cache,
        }
    }
    pub fn refresh(
        &mut self,
        config: &'a TypsiteConfig,
        registry: &KeyRegistry,
        loaded_articles: &HashMap<Key, Article<'a>>,
    ) {
        type Slug = Key;
        type Dependency = HashSet<Arc<Path>>;
        type DependencyIndex = HashMap<Arc<Path>, HashSet<UpdatedIndex>>;

        let dependents: Vec<(Slug, Dependency, DependencyIndex)> = loaded_articles
            .iter()
            .map(|(slug, article)| {
                (
                    slug.clone(),
                    article.get_depending_components(config),
                    article.get_dependency(registry),
                )
            })
            .collect();

        let mut updated_path = HashSet::new();

        for (slug, depending_components, dependency) in dependents {
            for dep in depending_components {
                updated_path.insert(dep.clone());
                self.add(dep.clone(), slug.clone());
            }
            for (dep, _) in dependency.iter() {
                updated_path.insert(dep.clone());
                self.add(dep.clone(), slug.clone());
            }
            self.deps.insert(slug.clone(), dependency);
        }

        self.write_cache(updated_path);
    }

    fn write_cache(&self, updated_path: HashSet<Arc<Path>>) {
        updated_path
            .into_iter()
            .par_bridge()
            .filter_map(|path| Some((path.clone(), self.cache.get(&path)?)))
            .map(|(path, dep)| {
                let dep = dep
                    .iter()
                    .map(|slug| slug.to_string())
                    .collect::<HashSet<String>>();
                let content = serde_json::to_string(&dep).context("Failed to serialize dep")?;
                let dep_path = PathBuf::from(format!(
                    "{}.dep",
                    self.deps_path.join(path.as_ref()).display()
                ));
                write_into_file(dep_path, &content, "dependency")
            })
            .for_each(log_err);
    }

    fn add(&mut self, dep: Arc<Path>, dependent: Key) {
        self.cache.entry(dep).or_default().insert(dependent);
    }

    pub fn take_dependency(&mut self, slug: &str, path: &Path) -> Option<HashSet<UpdatedIndex>> {
        self.deps.get_mut(slug).map(|deps| deps.remove(path))?
    }

    pub fn get(&self, dep: &Path) -> Option<&HashSet<Key>> {
        self.cache.get(dep)
    }
}
