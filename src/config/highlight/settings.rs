/// Code based on <https://github.com/defuz/sublimate/blob/master/src/core/settings.rs>
/// released under the MIT license by @defuz
use plist::Error as PlistError;
use std::io::{Read, Seek};

pub use serde_json::Value as Settings;

/// An error parsing a settings file
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SettingsError {
    /// Incorrect Plist syntax
    #[error("Incorrect Plist syntax: {0}")]
    Plist(PlistError),
}

impl From<PlistError> for SettingsError {
    fn from(error: PlistError) -> SettingsError {
        SettingsError::Plist(error)
    }
}

pub fn read_plist<R: Read + Seek>(reader: R) -> Result<Settings, SettingsError> {
    let settings = plist::from_reader(reader)?;
    Ok(settings)
}
