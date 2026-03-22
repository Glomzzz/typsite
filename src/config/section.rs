use crate::config::SECTION_PATH;
use crate::util::html::HtmlWithElem;
use crate::util::str::SectionElem;
use std::path::Path;
use std::sync::Arc;

pub struct SectionConfig {
    pub path: Arc<Path>,
    pub head: String,
    pub body: Vec<SectionElem>,
    title_index: usize,
    content_index: usize,
}

impl SectionConfig {
    pub fn load(config: &Path) -> anyhow::Result<Self> {
        let path = Arc::from(config.join(SECTION_PATH));
        let HtmlWithElem { head, body } = HtmlWithElem::load(&path)?;
        let title_count = body.iter().filter(|&it| it == &SectionElem::Title).count();
        let content_count = body
            .iter()
            .filter(|&it| it == &SectionElem::Content)
            .count();
        if title_count != 1 || content_count != 1 {
            return Err(anyhow::anyhow!(
                "Invalid heading config in {}: expected exactly one {{title}} placeholder and one {{content}} placeholder",
                path.display()
            ));
        }
        let title_index = body
            .iter()
            .position(|it| it == &SectionElem::Title)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid heading config in {}: missing {{title}} placeholder",
                    path.display()
                )
            })?;
        let content_index = body
            .iter()
            .position(|it| it == &SectionElem::Content)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid heading config in {}: missing {{content}} placeholder",
                    path.display()
                )
            })?;
        Ok(SectionConfig {
            path,
            head,
            body,
            title_index,
            content_index,
        })
    }

    pub fn before_title(&self) -> &[SectionElem] {
        &self.body[..self.title_index]
    }
    pub fn before_content(&self) -> &[SectionElem] {
        &self.body[self.title_index + 1..self.content_index]
    }
    pub fn after_content(&self) -> &[SectionElem] {
        &self.body[self.content_index + 1..]
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    pub(crate) fn run_config_loader_reports_missing_section_placeholders() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test timestamp should be available")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("typsite-section-config-{unique}"));
        let components = root.join("components");
        fs::create_dir_all(&components).expect("test components directory should be created");
        fs::write(
            components.join("section.html"),
            "<html><body><div>{title}</div></body></html>",
        )
        .expect("test section config should be written");

        let err = match SectionConfig::load(&root) {
            Ok(_) => panic!("missing content placeholder should be reported"),
            Err(err) => err,
        };
        let err = err.to_string();

        assert!(err.contains("section.html"));
        assert!(err.contains("{content}"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn config_loader_reports_missing_section_placeholders() {
        run_config_loader_reports_missing_section_placeholders();
    }
}
