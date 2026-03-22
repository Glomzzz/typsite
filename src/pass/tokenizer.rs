use anyhow::*;
use std::cell::LazyCell;
use std::collections::BTreeMap;
use std::result::Result::Ok;

use std::iter::Peekable;

use crate::ir::embed::EmbedVariables;
use crate::util::html::{from_css_style, html_as_str, to_css_style, Attributes};
use html5gum::{EndTag, HtmlString, StartTag, StringReader, Token};

pub trait Label {
    fn name(&self) -> &'static str;
}

pub enum Event<T: Label> {
    Start(T),
    End(T),
    Other(Token),
    Eof,
}

#[derive(Debug, Clone)]
pub enum HeadTag {
    Schema { schema: String },
    Unique,
}

const SCHEMA_KEY: &str = "schema";
const UNIQUE_KEY: &str = "unique";
impl Label for HeadTag {
    fn name(&self) -> &'static str {
        match &self {
            HeadTag::Schema { .. } => SCHEMA_KEY,
            HeadTag::Unique => UNIQUE_KEY,
        }
    }
}

#[derive(Debug, Clone)]
pub enum BodyTag {
    // Meta
    MetaGraph {
        key: String,
        slug: String,
    },
    MetaOption {
        key: String,
        value: String,
    },
    MetaContentGet {
        attrs: Attributes,
    },
    MetaContentSet {
        key: String,
    },
    // Content
    Section {
        heading_level: usize,
    },
    Rewrite {
        tag: String,
        attrs: Attributes,
    },
    Embed {
        slug: String,
        open: bool,
        variables: EmbedVariables,
        sidebar: String,
        heading_level: usize,
    },
    AnchorGoto {
        id: String,
        svg_transform: Option<String>,
    },
    AnchorDef {
        id: String,
        svg_transform: Option<String>,
    },
}

const META_GRAPH_KEY: &str = "metagraph";
const META_OPTION_KEY: &str = "metaoption";
const META_CONTENT_KEY: &str = "metacontent";
const HEADING_KEYS: [&str; 6] = ["h1", "h2", "h3", "h4", "h5", "h6"];
const EMBED_KEY: &str = "embed";
const REWRITE_KEY: &str = "rewrite";
const ANCHOR_DEF_KEY: &str = "anchordef";
const ANCHOR_GOTO_KEY: &str = "anchorgoto";

impl Label for BodyTag {
    fn name(&self) -> &'static str {
        match &self {
            BodyTag::MetaGraph { .. } => META_GRAPH_KEY,
            BodyTag::MetaOption { .. } => META_OPTION_KEY,
            BodyTag::MetaContentGet { .. } | BodyTag::MetaContentSet { .. } => META_CONTENT_KEY,
            BodyTag::Section { heading_level } => HEADING_KEYS[*heading_level - 1],
            BodyTag::Rewrite { .. } => REWRITE_KEY,
            BodyTag::Embed { .. } => EMBED_KEY,
            BodyTag::AnchorGoto { .. } => ANCHOR_GOTO_KEY,
            BodyTag::AnchorDef { .. } => ANCHOR_DEF_KEY,
        }
    }
}

pub type PeekableTokenizer<'a> = Peekable<html5gum::Tokenizer<StringReader<'a>>>;

pub trait EventTokenizer<T: Label> {
    fn next(&mut self) -> Option<Result<Event<T>>>;
}
struct State {
    auto_svg: Option<f64>,
    svg: bool,
    link_in_svg: bool,
}

pub struct Tokenizer<'a, 'b, T: Label> {
    tokenizer: &'b mut PeekableTokenizer<'a>,
    backtrace: Vec<T>,
    state: State,
}

impl<'a, 'b, T: Label> Tokenizer<'a, 'b, T> {
    pub fn new(tokenizer: &'b mut PeekableTokenizer<'a>) -> Tokenizer<'a, 'b, T> {
        Tokenizer {
            tokenizer,
            backtrace: Vec::new(),
            state: State {
                auto_svg: None,
                svg: false,
                link_in_svg: false,
            },
        }
    }
}

impl EventTokenizer<HeadTag> for Tokenizer<'_, '_, HeadTag> {
    fn next(&mut self) -> Option<Result<Event<HeadTag>>> {
        let next = self.tokenizer.next()?;
        let event = emit_head_next(self, next.context("Error ocurred while parsing HTML"));
        match event {
            Ok(Some(event)) => Some(Ok(event)),
            Ok(None) => self.next(),
            Err(err) => Some(Err(err)),
        }
    }
}
fn emit_head_next(
    tokenizer: &mut Tokenizer<'_, '_, HeadTag>,
    token: Result<Token>,
) -> Result<Option<Event<HeadTag>>> {
    let next = token?;
    match next {
        Token::StartTag(start_tag) => {
            let name = String::from_utf8_lossy(&start_tag.name).to_string();
            let tag = match name.as_str() {
                SCHEMA_KEY => {
                    let attrs = Attributes::new(start_tag.attributes);
                    let schema = attrs
                        .get("name")
                        .context("Schema: expect name attribute")?
                        .to_string();
                    HeadTag::Schema { schema }
                }
                UNIQUE_KEY => HeadTag::Unique,
                _ => return Ok(Some(Event::Other(Token::StartTag(start_tag)))),
            };
            if !start_tag.self_closing {
                tokenizer.backtrace.push(tag.clone());
            }

            Ok(Some(Event::Start(tag)))
        }
        Token::EndTag(end_tag) => {
            let name = String::from_utf8_lossy(&end_tag.name).to_string();
            let event = match name.as_str() {
                "head" => Event::Eof,
                SCHEMA_KEY => {
                    let backtrace = tokenizer.backtrace.pop();
                    Event::End(backtrace.context("Expect a start tag in the backtrace stack.")?)
                }
                UNIQUE_KEY => Event::End(HeadTag::Unique),
                _ => Event::Other(Token::EndTag(end_tag)),
            };
            Ok(Some(event))
        }

        plain @ Token::String(_) => Ok(Some(Event::Other(plain))),
        _ => Ok(None),
    }
}
impl EventTokenizer<BodyTag> for Tokenizer<'_, '_, BodyTag> {
    fn next(&mut self) -> Option<Result<Event<BodyTag>>> {
        let next = self.tokenizer.next()?;
        let event = emit_body_next(self, next.context("Error ocurred while parsing HTML"));
        match event {
            Ok(Some(event)) => Some(Ok(event)),
            Ok(None) => self.next(),
            Err(err) => Some(Err(err)),
        }
    }
}
fn emit_body_next(
    tokenizer: &mut Tokenizer<'_, '_, BodyTag>,
    token: Result<Token>,
) -> Result<Option<Event<BodyTag>>> {
    let next = token?;
    match next {
        Token::StartTag(start_tag) => {
            let name = String::from_utf8_lossy(&start_tag.name).to_string();
            let tag = match name.as_str() {
                META_OPTION_KEY => {
                    let attrs = Attributes::new(start_tag.attributes);
                    let key = attrs
                        .get("key")
                        .context("MetaOption: expect key attribute")?
                        .to_string();
                    let value = attrs
                        .get("value")
                        .context("MetaOption: expect value attribute")?
                        .to_string();
                    BodyTag::MetaOption { key, value }
                }
                META_CONTENT_KEY => {
                    let attrs = Attributes::new(start_tag.attributes);
                    if let Some(key) = attrs.get("set") {
                        BodyTag::MetaContentSet {
                            key: key.to_string(),
                        }
                    } else if attrs.get("get").is_some() {
                        BodyTag::MetaContentGet { attrs }
                    } else {
                        return Err(anyhow!("Expect `set` or `get` attribute in <metacontent>"));
                    }
                }
                META_GRAPH_KEY => {
                    let attrs = Attributes::new(start_tag.attributes);
                    let key = attrs
                        .get("key")
                        .context("MetaGraph: expect key attribute")?
                        .to_string();
                    let slug = attrs
                        .get("slug")
                        .context("MetaGraph: expect slug attribute")?
                        .to_string();
                    BodyTag::MetaGraph { key, slug }
                }
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let heading_level = name[1..].parse::<usize>()?;
                    let peek = tokenizer.tokenizer.peek();
                    if let Some(peek) = peek {
                        let peek = peek.clone()?;
                        match peek {
                            Token::StartTag(peek) if html_as_str(&peek.name) == EMBED_KEY => {
                                let _ = tokenizer.tokenizer.next();
                                let mut attrs = Attributes::new(peek.attributes);
                                let slug = attrs
                                    .take("slug")
                                    .context("Embed: expect slug attribute")?
                                    .to_string();
                                let open = attrs.take("open").map(|v| v == "true").unwrap_or(false);
                                let sidebar = attrs.take("sidebar").unwrap_or("full".to_string());
                                let variables = attrs.into_variables();
                                BodyTag::Embed {
                                    slug,
                                    open,
                                    variables,
                                    sidebar,
                                    heading_level,
                                }
                            }
                            _ => BodyTag::Section { heading_level },
                        }
                    } else {
                        BodyTag::Section { heading_level }
                    }
                }
                REWRITE_KEY => {
                    let attrs = Attributes::new(start_tag.attributes);
                    let tag = attrs
                        .get("id")
                        .context("Rewrite: expect id attribute")?
                        .to_string();
                    BodyTag::Rewrite { tag, attrs }
                }
                EMBED_KEY => {
                    let mut attrs = Attributes::new(start_tag.attributes);
                    let heading_level = attrs
                        .take("heading_level")
                        .unwrap_or("1".to_string())
                        .parse()?;
                    let slug = attrs
                        .take("slug")
                        .context("Embed: expect slug attribute")?
                        .to_string();
                    let open = attrs.take("open").map(|v| v == "true").unwrap_or(false);
                    let sidebar = attrs
                        .take("sidebar")
                        .unwrap_or("full".to_string())
                        .to_string();
                    let variables = attrs.into_variables();
                    BodyTag::Embed {
                        slug,
                        open,
                        variables,
                        sidebar,
                        heading_level,
                    }
                }
                ANCHOR_GOTO_KEY => {
                    let attrs = Attributes::new(start_tag.attributes);
                    let id = attrs
                        .expect("id")
                        .context("Expected attribute name on anchor")?
                        .to_string();
                    BodyTag::AnchorGoto {
                        id,
                        svg_transform: None,
                    }
                }
                ANCHOR_DEF_KEY => {
                    let attrs = Attributes::new(start_tag.attributes);
                    let id = attrs
                        .expect("id")
                        .context("Expected attribute name on anchor")?
                        .to_string();
                    BodyTag::AnchorDef {
                        id,
                        svg_transform: None,
                    }
                }
                _ => {
                    return emit_other_start(
                        &mut tokenizer.backtrace,
                        &mut tokenizer.state,
                        name,
                        start_tag,
                    );
                }
            };
            if !start_tag.self_closing {
                tokenizer.backtrace.push(tag.clone());
            }
            Ok(Some(Event::Start(tag)))
        }
        Token::EndTag(end_tag) => {
            let name = String::from_utf8_lossy(&end_tag.name).to_string();
            match name.as_str() {
                "body" => Ok(Some(Event::Eof)),
                META_GRAPH_KEY | META_OPTION_KEY | META_CONTENT_KEY | REWRITE_KEY | EMBED_KEY
                | ANCHOR_GOTO_KEY | ANCHOR_DEF_KEY | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let backtrace = tokenizer.backtrace.pop();
                    Ok(Some(Event::End(
                        backtrace.context("Expect a start tag in the backtrace stack.")?,
                    )))
                }
                _ => emit_other_end(
                    &mut tokenizer.backtrace,
                    &mut tokenizer.state,
                    name,
                    end_tag,
                ),
            }
        }
        plain @ Token::String(_) => Ok(Some(Event::Other(plain))),
        _ => Ok(None),
    }
}

const SVG_ANCHOR_PREFIX: &str = "#anchor:";
const SVG_GOTO_PREFIX: &str = "#goto:";
const SVG_FOOTNOTE_REF_PREFIX: &str = "#footnote-ref:";
const SVG_LINK: &str = "svg-link";
const CLASS_KEY: &[u8] = b"class";
const AUTO_SVG_KEY: &[u8] = b"auto-svg"; // will be removed 10 versions later
const AUTO_SIZED_SVG_KEY: &[u8] = b"auto-sized-svg";
const SCALE_KEY: &[u8] = b"scale";
const TYPST_DOC_KEY: &[u8] = b"typst-frame";
const STYLE_KEY: &[u8] = b"style";
const WIDTH_KEY: &[u8] = b"width";
const HEIGHT_KEY: &[u8] = b"height";

// const VIEW_BOX_KEY: LazyCell<HtmlString> = LazyCell::new(|| b"viewbox".to_vec().into());
const SVG_HREF_KEY: LazyCell<HtmlString> = LazyCell::new(|| b"href".to_vec().into());
const SVG_TRANSFORM_KEY: LazyCell<HtmlString> = LazyCell::new(|| b"transform".to_vec().into());
const PT_OVER_PX: f64 = 3.0 / 4.0;

const FRACTION_TO_PX: f64 = 11.0 / 9.0;

fn scale(attrs: &mut BTreeMap<HtmlString, HtmlString>, key: &[u8], ratio: f64) -> Result<()> {
    let origin = attrs
        .get(key)
        .with_context(|| format!("Expect an attribute of {key:#?} in auto-svg tag"))?;
    let origin = html_as_str(origin).to_string();
    if !origin.ends_with("em") {
        return Err(anyhow!("Cannot pass {key:#?} using units other than em"));
    }
    let origin = origin[0..origin.len() - 2].parse::<f64>()?;
    let result = origin * ratio * PT_OVER_PX * FRACTION_TO_PX;
    let result = format!("{result}em");
    attrs.insert(key.to_vec().into(), result.as_bytes().to_vec().into());
    Ok(())
}

fn emit_other_start(
    backtrace: &mut Vec<BodyTag>,
    state: &mut State,
    name: String,
    mut start_tag: StartTag,
) -> Result<Option<Event<BodyTag>>> {
    if name.as_str() == "svg" {
        state.svg = true;
    }
    match name.as_str() {
        "span" if matches!(start_tag.attributes.get(CLASS_KEY),Some(class) if class == &AUTO_SVG_KEY || class == &AUTO_SIZED_SVG_KEY) =>
        {
            let scale = start_tag.attributes.get(SCALE_KEY).with_context(|| {
                format!("Expect an attribute of {SCALE_KEY:#?} in auto-sized-svg tag")
            })?;
            let scale = html_as_str(scale).to_string();
            let scale = scale[0..scale.len() - 1].parse::<f64>()? / 100.0;
            state.auto_svg = Some(scale)
        }
        "svg"
            if state.auto_svg.is_some()
                && matches!(start_tag.attributes.get(CLASS_KEY),Some(class) if class == &TYPST_DOC_KEY) =>
        {
            let svg = &mut start_tag.attributes;
            if let Some(ratio) = state.auto_svg {
                let mut svg_style = svg
                    .get(STYLE_KEY)
                    .ok_or(anyhow!("Unsupport typst version, 0.14.1 required."))
                    .and_then(|it| from_css_style(html_as_str(it).as_ref()))?;
                // svg.remove(VIEW_BOX_KEY.as_ref());
                scale(&mut svg_style, WIDTH_KEY, ratio)?;
                scale(&mut svg_style, HEIGHT_KEY, ratio)?;
                svg.insert(
                    STYLE_KEY.to_vec().into(),
                    to_css_style(&svg_style).into_bytes().into(),
                );
                state.auto_svg = None;
            }
        }
        "a" if state.svg => {
            let url = start_tag
                .attributes
                .get((*SVG_HREF_KEY).as_ref())
                .ok_or(anyhow!(
                    "Expect an attribute of {:?} in svg <a> tag",
                    *SVG_HREF_KEY
                ))
                .map(|v| html_as_str(v))?;
            let transform = start_tag
                .attributes
                .get((*SVG_TRANSFORM_KEY).as_ref())
                .ok_or(anyhow!(
                    "Expect an attribute of {:?} in svg <a> tag",
                    *SVG_TRANSFORM_KEY
                ))
                .map(|v| html_as_str(v).to_string())?;
            let url = url.as_ref();
            let tag = if url.starts_with(SVG_ANCHOR_PREFIX) {
                let Some(url) = url.strip_prefix(SVG_ANCHOR_PREFIX) else {
                    return Err(anyhow!("Missing svg anchor prefix after validation"));
                };
                BodyTag::AnchorDef {
                    id: url.to_string(),
                    svg_transform: Some(transform),
                }
            } else if url.starts_with(SVG_GOTO_PREFIX) {
                state.link_in_svg = true;
                let Some(url) = url.strip_prefix(SVG_GOTO_PREFIX) else {
                    return Err(anyhow!("Missing svg goto prefix after validation"));
                };
                BodyTag::AnchorGoto {
                    id: url.to_string(),
                    svg_transform: Some(transform),
                }
            } else if url.starts_with(SVG_FOOTNOTE_REF_PREFIX) {
                state.link_in_svg = true;
                let Some(footnote) = url.strip_prefix(SVG_FOOTNOTE_REF_PREFIX) else {
                    return Err(anyhow!("Missing svg footnote prefix after validation"));
                };
                BodyTag::Rewrite {
                    tag: "footnote-ref-svg".to_string(),
                    attrs: {
                        let mut attrs = BTreeMap::new();
                        attrs.insert(
                            HtmlString("name".as_bytes().to_vec()),
                            footnote.as_bytes().to_vec().into(),
                        );
                        attrs.insert(
                            HtmlString("transform".as_bytes().to_vec()),
                            transform.as_bytes().to_vec().into(),
                        );
                        Attributes::new(attrs)
                    },
                }
            } else {
                state.link_in_svg = true;
                if !start_tag.self_closing {
                    backtrace.push(BodyTag::AnchorGoto {
                        id: url.to_string(),
                        svg_transform: Some(transform),
                    });
                }
                return Ok(Some(Event::Other(Token::StartTag(start_tag))));
            };
            if !start_tag.self_closing {
                backtrace.push(tag.clone());
            }
            return Ok(Some(Event::Start(tag)));
        }

        "rect" if state.link_in_svg => {
            let rect = &mut start_tag.attributes;
            let class_val = if let Some(class) = rect.get(CLASS_KEY) {
                let class = html_as_str(class);
                let class = class.as_ref();
                if class.is_empty() {
                    SVG_LINK.to_string()
                } else {
                    format!("{class} {}", SVG_LINK)
                }
            } else {
                SVG_LINK.to_string()
            };
            rect.insert(
                CLASS_KEY.to_vec().into(),
                class_val.as_bytes().to_vec().into(),
            );
        }
        _ => {}
    }
    Ok(Some(Event::Other(Token::StartTag(start_tag))))
}

fn emit_other_end(
    backtrace: &mut Vec<BodyTag>,
    state: &mut State,
    name: String,
    end_tag: EndTag,
) -> Result<Option<Event<BodyTag>>> {
    match name.as_str() {
        "svg" if state.svg => {
            state.svg = false;
        }
        "a" if state.svg => {
            if let Some(start_tag) = backtrace.pop() {
                state.link_in_svg = false;
                return Ok(Some(Event::End(start_tag)));
            }
        }

        _ => {}
    }
    Ok(Some(Event::Other(Token::EndTag(end_tag))))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{BodyTag, Event, EventTokenizer, Tokenizer};

    pub(crate) fn run_tokenizer_parses_svg_anchor_without_strip_prefix_panic() {
        let html = r##"<body><svg><a href="#anchor:svg-target" transform="translate(1 2)"></a></svg></body>"##;
        let mut raw = html5gum::Tokenizer::new(html).peekable();
        let mut tokenizer = Tokenizer::<BodyTag>::new(&mut raw);

        loop {
            match tokenizer.next() {
                Some(Ok(Event::Start(BodyTag::AnchorDef { id, svg_transform }))) => {
                    assert_eq!(id, "svg-target");
                    assert_eq!(svg_transform.as_deref(), Some("translate(1 2)"));
                    break;
                }
                Some(Ok(Event::Eof)) => panic!("svg anchor should produce an AnchorDef event"),
                Some(Ok(_)) => {}
                Some(Err(err)) => panic!("svg anchor should parse without error: {err}"),
                None => panic!("svg anchor should produce an AnchorDef event"),
            }
        }
    }

    #[test]
    fn tokenizer_parses_svg_anchor_without_strip_prefix_panic() {
        run_tokenizer_parses_svg_anchor_without_strip_prefix_panic();
    }
}
