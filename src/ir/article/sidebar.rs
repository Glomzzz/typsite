use crate::ir::metadata::content::TITLE_KEY;
use crate::ir::metadata::Metadata;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub type Pos = Vec<usize>;
pub type SidebarPos = (Pos, usize);
pub type SidebarIndexes = HashSet<usize>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sidebar {
    contents: Vec<String>,
    title_indexes: SidebarIndexes,
    show_children: SidebarIndexes,
    #[serde(with = "pos_to_sidebar_index_serde")]
    numberings: HashMap<Pos, SidebarIndexes>,
    #[serde(with = "pos_to_sidebar_index_serde")]
    anchors: HashMap<Pos, SidebarIndexes>,
}
impl Sidebar {
    pub fn new(
        contents: Vec<String>,
        title_indexes: SidebarIndexes,
        show_children: SidebarIndexes,
        numbering: HashMap<Pos, SidebarIndexes>,
        anchor: HashMap<Pos, SidebarIndexes>,
    ) -> Self {
        Self {
            contents,
            title_indexes,
            show_children,
            numberings: numbering,
            anchors: anchor,
        }
    }

    pub fn with_contents(self, contents: Vec<String>) -> Self {
        Self { contents, ..self }
    }

    pub fn cache(&self, metadata: &Metadata) -> Vec<String> {
        let mut contents = self.contents.clone();
        let Some(title) = metadata.contents.get(TITLE_KEY) else {
            let message = format!(
                "[ERROR] Missing `{TITLE_KEY}` metadata required for sidebar title rendering"
            );
            eprintln!("{message}");
            for &title_index in &self.title_indexes {
                if let Some(slot) = contents.get_mut(title_index) {
                    *slot = message.clone();
                }
            }
            return contents;
        };
        for &title_index in &self.title_indexes {
            if let Some(slot) = contents.get_mut(title_index) {
                *slot = title.to_string();
            }
        }
        contents
    }
    pub fn title_index(&self) -> &SidebarIndexes {
        &self.title_indexes
    }

    pub fn numberings(&self) -> &HashMap<Pos, SidebarIndexes> {
        &self.numberings
    }

    pub fn anchors(&self) -> &HashMap<Pos, SidebarIndexes> {
        &self.anchors
    }
    pub fn indexes(&self) -> &SidebarIndexes {
        &self.show_children
    }
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Copy, Clone)]
pub enum SidebarType {
    All,
    OnlyEmbed,
}
impl From<&str> for SidebarType {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "all" => SidebarType::All,
            "only-embed" | "only_embed" => SidebarType::OnlyEmbed,
            _ => SidebarType::All,
        }
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum HeadingNumberingStyle {
    Bullet,
    Roman,
    Alphabet,
    None,
}
impl From<&str> for HeadingNumberingStyle {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "none" => Self::None,
            "roman" => Self::Roman,
            "alphabet" => Self::Alphabet,
            _ => Self::Bullet,
        }
    }
}
impl Default for HeadingNumberingStyle {
    fn default() -> Self {
        Self::Bullet
    }
}

const ROMANS: [&str; 20] = [
    "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII", "XIII", "XIV", "XV",
    "XVI", "XVII", "XVIII", "XIX", "XX",
];

impl HeadingNumberingStyle {
    pub fn display(&self, pos: &Pos) -> String {
        match self {
            Self::Bullet => pos
                .iter()
                .map(|it| (it + 1).to_string())
                .collect::<Vec<String>>()
                .join("."),
            Self::Roman => pos
                .iter()
                .map(|it| ROMANS.get(*it).copied().unwrap_or("?"))
                .collect::<Vec<&str>>()
                .join("."),
            Self::Alphabet => pos
                .iter()
                .map(|it| ((b'A' + *it as u8) as char).to_string())
                .collect::<Vec<String>>()
                .join("."),
            Self::None => String::new(),
        }
    }
}

mod pos_to_sidebar_index_serde {
    use super::*;
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer>(
        map: &HashMap<Pos, SidebarIndexes>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut entries = Vec::with_capacity(map.len());
        for (pos, set) in map {
            entries.push((
                pos.iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join("-"),
                set,
            ));
        }
        entries.serialize(serializer)
    }
    pub fn deserialize<'ce, D: Deserializer<'ce>>(
        deserializer: D,
    ) -> Result<HashMap<Pos, SidebarIndexes>, D::Error> {
        let entries: Vec<(String, SidebarIndexes)> = Vec::deserialize(deserializer)?;
        let mut map = HashMap::with_capacity(entries.len());
        for (pos_str, set) in entries {
            let pos: Pos = if pos_str.is_empty() {
                vec![]
            } else {
                pos_str
                    .split('-')
                    .map(|s| s.parse().map_err(Error::custom))
                    .collect::<Result<_, _>>()?
            };
            map.insert(pos, set);
        }
        Ok(map)
    }
}

#[cfg(test)]
mod test_serde_pos_to_sidebar_index {
    use std::collections::HashMap;

    use anyhow::{Context, Result};

    use crate::ir::article::sidebar::{Pos, SidebarIndexes};
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct WrapperPosToSidebarIndex {
        #[serde(with = "super::pos_to_sidebar_index_serde")]
        map: HashMap<Pos, SidebarIndexes>,
    }
    #[test]
    fn pos_to_sidebar_index() -> Result<()> {
        let mut test: HashMap<Pos, SidebarIndexes> = HashMap::new();
        test.insert(vec![1], vec![10].into_iter().collect());
        test.insert(vec![1, 1], vec![12, 15].into_iter().collect());
        test.insert(vec![2, 3], vec![17, 20].into_iter().collect());
        test.insert(vec![], vec![3].into_iter().collect());
        let test = WrapperPosToSidebarIndex { map: test };

        let str = serde_json::to_string(&test)
            .context("Failed to deserialize HashMap<Pos,SidebarIndex>!")?;
        let test_read = serde_json::from_str::<WrapperPosToSidebarIndex>(&str)
            .context("Failed to serialize HashMap<Pos,SidebarIndex>!")?;
        assert_eq!(test, test_read);
        Ok(())
    }
}
