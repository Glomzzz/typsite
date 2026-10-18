use crate::config::THEMES_DIR;
use crate::util::fs::files_with_extension;
use crate::util::path::file_stem;
use anyhow::{Context, Result, ensure};
use metadata::{LoadMetadata, RawMetadataEntry};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};
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

#[derive(Serialize, Deserialize)]
struct Syntaxes {
    syntax_set: SyntaxSet,
    syntax_paths_by_name: BTreeMap<String, Arc<Path>>,
    syntax_paths_by_stem: BTreeMap<String, Arc<Path>>,
    metadata_paths_by_stem: BTreeMap<String, Arc<Path>>,
}

#[derive(Serialize, Deserialize)]
pub struct CodeHightlightConfig {
    themes: ThemePaths,
    syntaxes: Syntaxes,
}

impl CodeHightlightConfig {
    pub fn load(config_path: &Path) -> Result<Self> {
        let (themes, syntaxes) = rayon::join(
            || load_themes(&config_path.join(THEMES_DIR)),
            || Syntaxes::load(&config_path.join(SYNTAXES_DIR)),
        );
        Ok(Self {
            themes: themes?,
            syntaxes: syntaxes?,
        })
    }

    /// Cache only validated resource data, keyed by content hashes and engine
    /// identity. SyntaxSet skips its metadata in serde, so preserve it explicitly.
    pub(crate) fn load_cached(config: &Path, cache: &Path, key: &str) -> Result<Self> {
        #[derive(Serialize, Deserialize)]
        struct Cached {
            key: String,
            config: CodeHightlightConfig,
            // Syntect metadata contains serde_json values; encode it separately
            // because bincode does not implement deserialize_any.
            metadata_json: Vec<u8>,
        }
        let path = crate::compile::project::managed_path(cache, Path::new("highlight.bin"))?;
        if let Ok(bytes) = fs::read(&path)
            && bytes.starts_with(b"TYPSITE-HIGHLIGHT-1\0")
            && let Some(rest) = bytes.get(b"TYPSITE-HIGHLIGHT-1\0".len()..)
            && let Some((checksum, payload)) = rest.split_at_checked(32)
            && blake3::hash(payload).as_bytes() == checksum
            && let Ok(mut cached) = bincode::deserialize::<Cached>(payload)
            && cached.key == key
            && let Ok(metadata) = serde_json::from_slice::<Metadata>(&cached.metadata_json)
        {
            cached.config.syntaxes.syntax_set.set_metadata(metadata);
            return Ok(cached.config);
        }
        let config = Self::load(config)?;
        let cached = Cached {
            key: key.to_owned(),
            metadata_json: serde_json::to_vec(config.syntaxes.syntax_set.metadata())?,
            config,
        };
        let payload =
            bincode::serialize(&cached).context("Cannot serialize highlighting resources")?;
        let mut bytes = b"TYPSITE-HIGHLIGHT-1\0".to_vec();
        bytes.extend_from_slice(blake3::hash(&payload).as_bytes());
        bytes.extend_from_slice(&payload);
        crate::util::atomic::write_cache(&path, &bytes)?;
        Ok(cached.config)
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

fn load_themes(themes_path: &Path) -> Result<ThemePaths> {
    let mut themes = BTreeMap::new();
    let paths = files_with_extension(themes_path, "tmTheme")?;
    let loaded: Vec<_> = paths
        .par_iter()
        .map(|path| {
            ThemeSet::get_theme(path)
                .with_context(|| format!("Failed to load theme {}", path.display()))
        })
        .collect();
    for (path, theme) in paths.into_iter().zip(loaded) {
        let name = file_stem(&path).context("Theme has no name")?.to_string();
        let theme = theme?;
        ensure!(
            !themes.contains_key(&name),
            "Duplicate theme {name} in {}",
            path.display()
        );
        themes.insert(name, (Arc::from(path), theme));
    }
    Ok(themes)
}

impl Syntaxes {
    fn load(syntaxes_path: &Path) -> Result<Self> {
        let mut builder = SyntaxSetBuilder::new();
        let mut syntax_paths_by_name = BTreeMap::new();
        let mut syntax_paths_by_stem = BTreeMap::new();
        for path in files_with_extension(syntaxes_path, "sublime-syntax")? {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to load syntax {}", path.display()))?;
            let syntax = SyntaxDefinition::load_from_str(&content, true, None)
                .with_context(|| format!("Failed to load syntax {}", path.display()))?;
            let stem = file_stem(&path).context("Syntax has no name")?.to_string();
            ensure!(
                !syntax_paths_by_name.contains_key(&syntax.name),
                "Duplicate syntax {} in {}",
                syntax.name,
                path.display()
            );
            ensure!(
                !syntax_paths_by_stem.contains_key(&stem),
                "Duplicate syntax filename {stem}"
            );
            let path: Arc<Path> = Arc::from(path);
            syntax_paths_by_name.insert(syntax.name.clone(), path.clone());
            syntax_paths_by_stem.insert(stem, path);
            builder.add(syntax);
        }
        let mut syntax_set = builder.build();
        let mut raw_metadata = LoadMetadata::default();
        let mut metadata_paths_by_stem = BTreeMap::new();
        for path in files_with_extension(syntaxes_path, "tmPreferences")? {
            let metadata = RawMetadataEntry::load(&path)
                .with_context(|| format!("Failed to load tmPreferences {}", path.display()))?;
            let stem = file_stem(&path)
                .context("Syntax metadata has no name")?
                .to_string();
            ensure!(
                !metadata_paths_by_stem.contains_key(&stem),
                "Duplicate syntax metadata filename {stem}"
            );
            metadata_paths_by_stem.insert(stem, Arc::from(path));
            raw_metadata.add_raw(metadata);
        }
        syntax_set.set_metadata(Metadata::try_from(raw_metadata)?);
        Ok(Self {
            syntax_set,
            syntax_paths_by_name,
            syntax_paths_by_stem,
            metadata_paths_by_stem,
        })
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

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn cached_syntax_metadata_roundtrips_without_reloading_or_losing_settings() {
        let root = std::env::temp_dir().join(format!(
            "typsite-highlight-metadata-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let config = root.join("config");
        fs::create_dir_all(config.join("syntaxes")).unwrap();
        fs::write(config.join("syntaxes/valid.tmPreferences"), r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>scope</key><string>source.rust</string><key>settings</key><dict><key>shellVariables</key><array><dict><key>name</key><string>TM_COMMENT_START</string><key>value</key><string>// </string></dict></array><key>increaseIndentPattern</key><string>[{]</string></dict></dict></plist>"#).unwrap();
        let cache = root.join("cache");
        let first = CodeHightlightConfig::load_cached(&config, &cache, "generation").unwrap();
        let expected = serde_json::to_value(first.syntaxes.syntax_set.metadata()).unwrap();
        assert!(
            !first
                .syntaxes
                .syntax_set
                .metadata()
                .scoped_metadata
                .is_empty()
        );
        let before = fs::metadata(cache.join("highlight.bin"))
            .unwrap()
            .modified()
            .unwrap();
        let second = CodeHightlightConfig::load_cached(&config, &cache, "generation").unwrap();
        assert_eq!(
            expected,
            serde_json::to_value(second.syntaxes.syntax_set.metadata()).unwrap()
        );
        assert_eq!(
            before,
            fs::metadata(cache.join("highlight.bin"))
                .unwrap()
                .modified()
                .unwrap(),
            "cache hit must not rewrite or rebuild resources"
        );
        fs::remove_dir_all(root).unwrap();
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
