use crate::config::THEMES_DIR;
use crate::util::error::log_err_or_ok;
use crate::util::path::file_stem;
use crate::walk_glob;
use anyhow::Context;
use glob::glob;
use metadata::{LoadMetadata, RawMetadataEntry};
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::{collections::BTreeMap, path::PathBuf};
use syntect::dumps::from_binary;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{Metadata, SyntaxDefinition, SyntaxReference, SyntaxSet, SyntaxSetBuilder};

use super::SYNTAXES_DIR;

mod metadata;
mod settings;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

fn default_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(|| from_binary(include_bytes!("syntax_newlines.packdump")))
}

type PathMap<T> = BTreeMap<String, (Arc<Path>, T)>;
type ThemePaths = PathMap<Theme>;

struct Syntaxes {
    syntax_set: SyntaxSet,
    syntax_paths_by_name: BTreeMap<String, Arc<Path>>,
    syntax_paths_by_stem: BTreeMap<String, Arc<Path>>,
    metadata_paths_by_stem: BTreeMap<String, Arc<Path>>,
}

pub struct CodeHightlightConfig {
    themes: ThemePaths,
    syntaxes: Syntaxes,
}

impl CodeHightlightConfig {
    pub fn load(config_path: &Path) -> Self {
        let themes_path = config_path.join(THEMES_DIR);
        let themes = load_themes(themes_path);
        let syntaxes_path = config_path.join(SYNTAXES_DIR);
        let syntaxes = Syntaxes::load(syntaxes_path);
        Self { themes, syntaxes }
    }

    fn find_theme_pair(&self, name: &str, scheme: ColorScheme) -> Option<&(Arc<Path>, Theme)> {
        let name_with_suffix = if name.ends_with(scheme.suffix()) {
            name.to_string()
        } else {
            format!("{name}{}", scheme.suffix())
        };
        self.themes.get(&name_with_suffix).or_else(|| {
            if scheme != ColorScheme::None {
                self.find_theme_pair(name, ColorScheme::None)
            } else {
                None
            }
        })
    }

    // light & dark theme
    pub fn find_theme(&self, name: &str) -> Option<(&Theme, &Theme)> {
        use ColorScheme::*;
        let dark = &self.find_theme_pair(name, Dark)?.1;
        let light = &self.find_theme_pair(name, Light)?.1;
        Some((light, dark))
    }
    pub fn find_theme_path(&self, name: &str) -> Option<Arc<Path>> {
        self.themes.get(name).map(|(path, _)| path).cloned()
    }

    // light & dark theme
    pub fn find_theme_path_pair(&self, name: &str) -> Option<(&Arc<Path>, &Arc<Path>)> {
        use ColorScheme::*;
        let dark = &self.find_theme_pair(name, Dark)?.0;
        let light = &self.find_theme_pair(name, Light)?.0;
        Some((light, dark))
    }

    pub fn find_syntax(&self, token: &str) -> &SyntaxReference {
        self.syntaxes
            .find(token)
            .unwrap_or(default_syntax_set().find_syntax_plain_text())
    }

    pub fn find_syntax_path(&self, token: &str) -> Option<Arc<Path>> {
        self.syntaxes.find_path(token)
    }
    pub fn find_syntax_path_by_stem(&self, stem: &str) -> Option<Arc<Path>> {
        self.syntaxes.find_syntax_path_by_stem(stem)
    }
    pub fn find_metadata_path_by_stem(&self, stem: &str) -> Option<Arc<Path>> {
        self.syntaxes.find_metadata_path_by_stem(stem)
    }

    pub fn syntax_set(&self, token: &str) -> &SyntaxSet {
        if !self.is_syntax_by_default(token) {
            &self.syntaxes.syntax_set
        } else {
            default_syntax_set()
        }
    }

    pub fn metadata_paths(&self) -> Vec<Arc<Path>> {
        self.syntaxes
            .metadata_paths_by_stem
            .values()
            .cloned()
            .collect()
    }

    pub fn is_syntax_by_default(&self, token: &str) -> bool {
        self.syntaxes.is_by_default(token)
    }
    fn syntax_set_by_extension(&self) -> &SyntaxSet {
        &self.syntaxes.syntax_set
    }
}

impl Display for CodeHightlightConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Syntaxes by deffault:")?;
        for syntax in default_syntax_set().syntaxes() {
            writeln!(f, "    - {}", syntax.name)?;
        }

        writeln!(f, "Syntaxes by extension:")?;
        for syntax in self.syntax_set_by_extension().syntaxes() {
            writeln!(f, "    - {}", syntax.name)?;
        }

        writeln!(f, "Themes")?;
        for theme in self.themes.keys() {
            writeln!(f, "    - {theme}")?;
        }
        Ok(())
    }
}

fn load_themes(themes_path: PathBuf) -> ThemePaths {
    walk_glob!("{}/**/*.tmTheme", themes_path.display())
        .filter(|entry| entry.is_file())
        .map(|path| {
            let name = file_stem(&path)
                .map(|s| s.to_string())
                .with_context(|| format!("Failed to load theme {}", path.display()));
            let theme = ThemeSet::get_theme(&path)
                .with_context(|| format!("Failed to load theme {}", path.display()));
            name.and_then(|name| theme.map(|theme| (name, (Arc::from(path), theme))))
        })
        .filter_map(log_err_or_ok)
        .collect()
}

impl Syntaxes {
    fn load(syntaxes_path: PathBuf) -> Self {
        let mut syntax_set = SyntaxSetBuilder::new();
        let mut syntax_paths_by_name = BTreeMap::new();
        let mut syntax_paths_by_stem = BTreeMap::new();
        walk_glob!("{}/**/*.sublime-syntax", syntaxes_path.display())
            .par_bridge()
            .filter(|entry| entry.is_file())
            .map(|path| {
                fs::read_to_string(&path)
                    .with_context(|| format!("Failed to load syntax {}", path.display()))
                    .and_then(|content| {
                        SyntaxDefinition::load_from_str(&content, true, None)
                            .with_context(|| format!("Failed to load syntax {}", path.display()))
                    })
                    .map(|syntax| (syntax.name.clone(), (path, syntax)))
            })
            .filter_map(log_err_or_ok)
            .collect::<Vec<(String, (PathBuf, SyntaxDefinition))>>()
            .into_iter()
            .for_each(|(name, (path, syntax))| {
                syntax_set.add(syntax);
                let path: Arc<Path> = Arc::from(path);
                syntax_paths_by_name.insert(name, path.clone());
                let stem = file_stem(&path).expect(
                    "syntax files matched by *.sublime-syntax should always have a file stem",
                );
                syntax_paths_by_stem.insert(stem.to_string(), path.clone());
            });
        let mut syntax_set = syntax_set.build();
        let mut raw_metadata = LoadMetadata::default();
        // if entry.path().extension() == Some("tmPreferences".as_ref()) {
        //     match RawMetadataEntry::load(entry.path()) {
        //         Ok(meta) => self.raw_metadata.add_raw(meta),
        //         Err(_err) => (),
        //     }
        // }
        let mut metadata_paths_by_stem = BTreeMap::new();
        walk_glob!("{}/**/*.tmPreferences", syntaxes_path.display())
            .par_bridge()
            .filter(|entry| entry.is_file())
            .map(|path| {
                RawMetadataEntry::load(&path)
                    .with_context(|| format!("Failed to load tmPreferences {}", path.display()))
                    .map(|metadata| (path, metadata))
            })
            .filter_map(log_err_or_ok)
            .collect::<Vec<(PathBuf, RawMetadataEntry)>>()
            .into_iter()
            .for_each(|(path, metadata)| {
                raw_metadata.add_raw(metadata);
                let path: Arc<Path> = Arc::from(path);
                let stem = file_stem(&path).expect(
                    "metadata files matched by *.tmPreferences should always have a file stem",
                );
                metadata_paths_by_stem.insert(stem.to_string(), path);
            });
        syntax_set.set_metadata(Metadata::from(raw_metadata));

        Self {
            syntax_set,
            syntax_paths_by_name,
            syntax_paths_by_stem,
            metadata_paths_by_stem,
        }
    }
    fn find(&self, token: &str) -> Option<&SyntaxReference> {
        default_syntax_set()
            .find_syntax_by_token(token)
            .or_else(|| self.syntax_set.find_syntax_by_token(token))
    }

    fn find_path(&self, token: &str) -> Option<Arc<Path>> {
        self.find(token)
            .and_then(|syntax| self.syntax_paths_by_name.get(&syntax.name))
            .cloned()
    }

    fn find_syntax_path_by_stem(&self, stem: &str) -> Option<Arc<Path>> {
        self.syntax_paths_by_stem.get(stem).cloned()
    }
    fn find_metadata_path_by_stem(&self, stem: &str) -> Option<Arc<Path>> {
        self.metadata_paths_by_stem.get(stem).cloned()
    }

    fn is_by_default(&self, token: &str) -> bool {
        matches!(token, "text" | "shell")
            || (self.syntax_set.find_syntax_by_token(token).is_none()
                && default_syntax_set().find_syntax_by_token(token).is_some())
    }
}

#[derive(Debug, PartialEq)]
enum ColorScheme {
    Dark,
    Light,
    None,
}
impl ColorScheme {
    pub fn suffix(&self) -> &str {
        match self {
            ColorScheme::Dark => "_dark",
            ColorScheme::Light => "_light",
            ColorScheme::None => "",
        }
    }
}
