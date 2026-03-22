use std::{collections::BTreeMap, fs::File, io::BufReader, path::PathBuf};

use super::settings::*;
use serde::{Deserialize, Serialize};
use syntect::parsing::{Metadata, MetadataSet};

type Dict = serde_json::Map<String, Settings>;

/// A String representation of a [`ScopeSelectors`] instance.
///
/// [`ScopeSelectors`]: ../../highlighting/struct.ScopeSelectors.html
type SelectorString = String;

/// From `syntect`
/// A type that can be deserialized from a `.tmPreferences` file.
/// Since multiple files can refer to the same scope, we merge them while loading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RawMetadataEntry {
    path: PathBuf,
    scope: SelectorString,
    settings: Dict,
}

impl RawMetadataEntry {
    pub fn load<P: Into<PathBuf>>(path: P) -> anyhow::Result<Self> {
        let path: PathBuf = path.into();
        let file = File::open(&path)?;
        let file = BufReader::new(file);
        let mut contents = read_plist(file)?;
        // we stash the path because we use it to determine parse order
        // when generating the final metadata object; to_string_lossy
        // is adequate for this purpose.
        let object = contents.as_object_mut().ok_or_else(|| {
            anyhow::anyhow!(
                "tmPreferences {} did not parse into an object",
                path.display()
            )
        })?;
        object.insert("path".into(), path.to_string_lossy().into());
        Ok(serde_json::from_value(contents)?)
    }
}

/// Convenience type for loading heterogeneous metadata.
#[derive(Debug, Default, Clone)]
pub(crate) struct LoadMetadata {
    loaded: Vec<RawMetadataEntry>,
}

// all of these are optional, but we don't want to deserialize if
// we don't have at least _one_ of them present
const KEYS_WE_USE: &[&str] = &[
    "increaseIndentPattern",
    "decreaseIndentPattern",
    "bracketIndentNextLinePattern",
    "disableIndentNextLinePattern",
    "unIndentedLinePattern",
    "indentParens",
    "shellVariables",
];

impl From<LoadMetadata> for Metadata {
    fn from(src: LoadMetadata) -> Metadata {
        let LoadMetadata { mut loaded } = src;
        loaded.sort_unstable_by(|a, b| a.path.cmp(&b.path));

        let mut scoped_metadata: BTreeMap<SelectorString, Dict> = BTreeMap::new();

        for RawMetadataEntry {
            scope,
            settings,
            path,
        } in loaded
        {
            let scoped_settings = scoped_metadata.entry(scope.clone()).or_insert_with(|| {
                let mut d = Dict::new();
                d.insert(
                    "source_file_path".to_string(),
                    path.to_string_lossy().into(),
                );
                d
            });

            for (key, value) in settings {
                if !KEYS_WE_USE.contains(&key.as_str()) {
                    continue;
                }

                if key.as_str() == "shellVariables" {
                    if let Err(err) = append_vars(scoped_settings, value, &scope) {
                        eprintln!(
                            "failed to merge tmPreferences shellVariables for scope {scope} from {}: {err}",
                            path.display()
                        );
                    }
                } else {
                    scoped_settings.insert(key, value);
                }
            }
        }

        let scoped_metadata = scoped_metadata
            .into_iter()
            .flat_map(|r| MetadataSet::from_raw(r).map_err(|e| eprintln!("{e}")))
            .collect();
        Metadata { scoped_metadata }
    }
}

fn append_vars(obj: &mut Dict, vars: Settings, scope: &str) -> anyhow::Result<()> {
    #[derive(Deserialize)]
    struct KeyPair {
        name: String,
        value: Settings,
    }
    #[derive(Deserialize)]
    struct ShellVars(Vec<KeyPair>);

    let vars = serde_json::from_value::<ShellVars>(vars)
        .map_err(|err| anyhow::anyhow!("malformed shell variables for scope {scope}: {err}"))?;
    let shell_vars = obj
        .entry(String::from("shellVariables"))
        .or_insert_with(|| Dict::new().into());
    let shell_vars = shell_vars.as_object_mut().ok_or_else(|| {
        anyhow::anyhow!(
            "malformed shellVariables container for scope {scope}: expected object while merging tmPreferences metadata"
        )
    })?;
    for KeyPair { name, value } in vars.0 {
        shell_vars.insert(name, value);
    }
    Ok(())
}

impl LoadMetadata {
    /// Adds the provided `RawMetadataEntry`
    ///
    /// When creating the final [`Metadata`] object, all [`RawMetadataEntry`] items are sorted by
    /// path, and items that share a scope selector are merged; last writer wins.
    ///
    /// [`Metadata`]: struct.Metadata.html
    /// [`RawMetadataEntry`]: struct.RawMetadataEntry.html
    pub fn add_raw(&mut self, raw: RawMetadataEntry) {
        self.loaded.push(raw);
    }
}
