use crate::config::{
    ANCHOR_DEF_PATH, ANCHOR_DEF_SVG_PATH, ANCHOR_GOTO_PATH, ANCHOR_GOTO_SVG_PATH, HtmlConfig,
};
use crate::ir::article::sidebar::Pos;
use crate::ir::pending::AnchorKind;
use crate::util::html::HtmlWithTail;
use crate::util::str::ac_replace;
use std::path::Path;
use std::sync::Arc;

pub const ANCHOR_KEY: &str = "{anchor}";
pub const TRANSFORM_KEY: &str = "{transform}";
pub const CONTENT_KEY: &str = "{content}";

#[derive(Debug)]
pub struct AnchorGoto {
    pub path: Arc<Path>,
    pub head: String,
    pub body: String,
    pub tail: String,
}

impl AnchorGoto {
    pub fn load(config: &Path, path: &str) -> anyhow::Result<Self> {
        let path = Arc::from(config.join(path));
        let HtmlWithTail { head, body, tail } = HtmlWithTail::load(&path, CONTENT_KEY)?;
        Ok(Self {
            path,
            head,
            body,
            tail,
        })
    }
}

pub struct AnchorConfig {
    pub define: HtmlConfig,
    pub define_in_svg: HtmlConfig,
    pub goto: AnchorGoto,
    pub goto_in_svg: AnchorGoto,
}

pub fn format_pos_as_anchor(pos: &Pos) -> String {
    if pos.is_empty() {
        String::default()
    } else {
        format!(
            "{}-",
            pos.iter()
                .map(|it| (it + 1).to_string())
                .collect::<Vec<String>>()
                .join("-")
        )
    }
}
fn format_anchor_with_name(pos: Option<&Pos>, anchor: &str) -> String {
    if let Some(pos) = pos {
        let pos = format_pos_as_anchor(pos);
        format!("{pos}{anchor}")
    } else {
        anchor.to_string()
    }
}

impl AnchorConfig {
    pub fn load(config: &Path) -> anyhow::Result<Self> {
        let define = HtmlConfig::load(config, ANCHOR_DEF_PATH)?;
        let define_in_svg = HtmlConfig::load(config, ANCHOR_DEF_SVG_PATH)?;
        let goto = AnchorGoto::load(config, ANCHOR_GOTO_PATH)?;
        let goto_in_svg = AnchorGoto::load(config, ANCHOR_GOTO_SVG_PATH)?;
        Ok(Self {
            define,
            define_in_svg,
            goto,
            goto_in_svg,
        })
    }

    pub fn get(&self, kind: &AnchorKind, pos: Option<&Pos>, anchor: &str) -> String {
        match kind {
            AnchorKind::DefineSvg { transform } => self.get_define_in_svg(pos, anchor, transform),
            AnchorKind::Define => self.get_define(pos, anchor),
            AnchorKind::GotoHead => self.get_goto_head(pos, anchor),
            AnchorKind::GotoHeadSvg { transform } => self.get_goto_head_svg(pos, anchor, transform),
            AnchorKind::GotoTail => self.get_goto_tail(pos, anchor),
        }
    }
    pub fn get_define_in_svg(&self, pos: Option<&Pos>, anchor: &str, transform: &str) -> String {
        ac_replace(
            &self.define_in_svg.body,
            &[
                (ANCHOR_KEY, format_anchor_with_name(pos, anchor).as_str()),
                (TRANSFORM_KEY, transform),
            ],
        )
    }

    pub fn get_define(&self, pos: Option<&Pos>, anchor: &str) -> String {
        ac_replace(
            &self.define.body,
            &[(ANCHOR_KEY, format_anchor_with_name(pos, anchor).as_str())],
        )
    }
    pub fn get_goto_head(&self, pos: Option<&Pos>, anchor: &str) -> String {
        ac_replace(
            &self.goto.body,
            &[(ANCHOR_KEY, format_anchor_with_name(pos, anchor).as_str())],
        )
    }
    pub fn get_goto_head_svg(&self, pos: Option<&Pos>, anchor: &str, transform: &str) -> String {
        ac_replace(
            &self.goto_in_svg.body,
            &[
                (ANCHOR_KEY, format_anchor_with_name(pos, anchor).as_str()),
                (TRANSFORM_KEY, transform),
            ],
        )
    }
    pub fn get_goto_tail(&self, pos: Option<&Pos>, anchor: &str) -> String {
        ac_replace(
            &self.goto.tail,
            &[(ANCHOR_KEY, format_anchor_with_name(pos, anchor).as_str())],
        )
    }
}
