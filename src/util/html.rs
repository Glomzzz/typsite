use crate::pass::tokenizer::PeekableTokenizer;
use crate::util::error::TypsiteError;
use crate::util::str::ElemTokenizerTrait;
use crate::util::str::{Elem, ElemTokenizer, ac_replace};
use anyhow::Context;
use anyhow::Result;
use anyhow::*;
use html5gum::{HtmlString, StringReader, Token, Tokenizer};
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Display;
use std::fmt::Write;
use std::fs::File;
use std::io::{BufReader, read_to_string};
use std::path::Path;
use std::result::Result::Ok;

#[derive(Debug, Clone, PartialEq)]
pub struct Attributes {
    attrs: BTreeMap<HtmlString, HtmlString>,
}

impl Attributes {
    pub fn new(attrs: BTreeMap<HtmlString, HtmlString>) -> Attributes {
        Self { attrs }
    }
    pub fn take(&mut self, key: &str) -> Result<String> {
        if let Some(html) = self
            .attrs
            .remove(&HtmlString::from(key.as_bytes().to_vec()))
        {
            Ok(String::from_utf8(html.0)
                .with_context(|| format!("Attribute {key} parsing failed"))?)
        } else {
            Err(anyhow!("Attribute {} not found", key))
        }
    }
    pub fn expect(&self, key: &str) -> Result<Cow<'_, str>> {
        self.get(key)
            .ok_or_else(|| anyhow!("Attribute {} not found", key))
    }

    pub fn get(&self, key: &str) -> Option<Cow<'_, str>> {
        self.attrs
            .get(&HtmlString::from(key.as_bytes().to_vec()))
            .map(|v| String::from_utf8_lossy(&v.0))
    }
    pub fn into_variables(self) -> Vec<(String, String)> {
        self.attrs
            .into_iter()
            .map(|(key, value)| {
                (
                    format!("{{{}}}", html_as_str(&key)),
                    html_as_str(&value).to_string(),
                )
            })
            .collect()
    }
}
impl FromIterator<(String, String)> for Attributes {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        let attrs = iter
            .into_iter()
            .map(|(k, v)| {
                (
                    HtmlString::from(k.as_bytes().to_vec()),
                    HtmlString::from(v.as_bytes().to_vec()),
                )
            })
            .collect();
        Self { attrs }
    }
}
impl Display for Attributes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut attrs = String::new();
        for (key, val) in &self.attrs {
            attrs.push_str(&format!("{}=\"{}\" ", html_as_str(key), html_as_str(val)));
        }
        write!(f, "{attrs}")
    }
}

impl Serialize for Attributes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map_serializer = serializer.serialize_map(Some(self.attrs.len()))?;

        for (key_bytes, val_bytes) in &self.attrs {
            let key_str = String::from_utf8_lossy(&key_bytes.0);
            let val_str = String::from_utf8_lossy(&val_bytes.0);

            map_serializer.serialize_entry(&key_str, &val_str)?;
        }

        map_serializer.end()
    }
}
impl<'ce> Deserialize<'ce> for Attributes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'ce>,
    {
        struct AttributeVisitor;
        impl<'ce> Visitor<'ce> for AttributeVisitor {
            type Value = Attributes;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a map of HTML attributes")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'ce>,
            {
                let mut attrs = BTreeMap::new();

                while let Some((raw_key, raw_val)) = map.next_entry::<String, String>()? {
                    let key = HtmlString::from(raw_key.as_bytes().to_vec());
                    let val = HtmlString::from(raw_val.as_bytes().to_vec());

                    attrs.insert(key, val);
                }
                Ok(Attributes { attrs })
            }
        }
        deserializer.deserialize_map(AttributeVisitor)
    }
}

#[derive(Debug, Clone)]
pub struct OutputHead<'a> {
    start: Vec<String>,
    head: Vec<&'a str>,
    end: Vec<String>,
}

impl<'a> OutputHead<'a> {
    pub fn empty() -> Self {
        Self {
            start: Vec::new(),
            head: Vec::new(),
            end: Vec::new(),
        }
    }

    pub fn start(&mut self, start: String) {
        if self.start.contains(&start) {
            return;
        }
        self.start.push(start);
    }

    pub fn end(&mut self, end: String) {
        if self.end.contains(&end) {
            return;
        }
        self.end.push(end);
    }

    pub fn push(&mut self, head: &'a str) {
        let head = head.trim();
        if self.head.contains(&head) {
            return;
        }
        self.head.push(head);
    }

    pub fn to_html(&self) -> String {
        let mut vec = vec![];
        vec.extend(self.start.iter().map(|it| it.as_str()));
        vec.extend(self.head.iter());
        vec.extend(self.end.iter().map(|it| it.as_str()));
        let head: String = vec
            .iter()
            .map(|it| format!("  {}", it.trim()))
            .collect::<Vec<_>>()
            .join("\n");
        ac_replace(&head, &[("../assets", "")])
    }
}

#[derive(Debug, Clone)]
pub struct OutputHtml<'a> {
    pub head: OutputHead<'a>,
    pub body: String,
}

impl Default for OutputHtml<'_> {
    fn default() -> Self {
        Self::empty()
    }
}
impl<'a> OutputHtml<'a> {
    pub fn empty() -> Self {
        Self::new(OutputHead::empty(), String::new())
    }
    pub fn new(head: OutputHead<'a>, body: String) -> Self {
        Self { head, body }
    }

    pub fn extend_body(&mut self, html: &OutputHtml<'a>) {
        self.body.push('\n');
        self.body.push_str(&html.body);
    }
    pub fn to_html(&self) -> String {
        let head = self.head.to_html();
        html(&head, &self.body)
    }
}

/**
* A struct that represents a HTML.
*/
#[derive(Debug)]
pub struct Html {
    pub head: String,
    pub body: String,
}

impl Html {
    pub fn load_with_body_callback<F>(path: &Path, body_callback: F) -> Result<Self>
    where
        F: FnMut(Token) -> Result<()>,
    {
        let file = File::open(path).with_context(|| format!("Cannot find the path: {path:?}"))?;
        let reader = read_to_string(BufReader::new(file))
            .with_context(|| format!("Cannot read the file {path:?}"))?;
        Self::load_by_tokenizer_with_body_callback(Tokenizer::new(&reader), body_callback)
    }
    pub fn load_by_tokenizer_with_body_callback<F>(
        mut tokenizer: Tokenizer<StringReader>,
        mut body_callback: F,
    ) -> Result<Self>
    where
        F: FnMut(Token) -> Result<()>,
    {
        let mut head = String::new();
        let mut body = String::new();
        parse_within_first(&mut tokenizer, b"head", |token| {
            write_token(&mut head, &token)
        })?;
        parse_within_first(&mut tokenizer, b"body", |token| {
            write_token(&mut body, &token).and_then(|_| body_callback(token))
        })?;
        Ok(Self::new(head, body))
    }
    pub fn load(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("Cannot find the path: {path:?}"))?;
        let reader = read_to_string(BufReader::new(file))
            .with_context(|| format!("Cannot read the file {path:?}"))?;
        Self::load_by_tokenizer(Tokenizer::new(&reader))
    }

    pub fn load_by_tokenizer(mut tokenizer: Tokenizer<StringReader>) -> Result<Self> {
        let mut head = String::new();
        let mut body = String::new();
        parse_within_first(&mut tokenizer, b"head", |token| {
            write_token(&mut head, &token)
        })?;
        parse_within_first(&mut tokenizer, b"body", |token| {
            write_token(&mut body, &token)
        })?;
        Ok(Self::new(head, body))
    }

    fn new(head: String, body: String) -> Self {
        let head = ac_replace(&head, &[("../assets", "")]);
        Self { head, body }
    }
}

/**
* A struct that represents a HTML.
*/
#[derive(Debug)]
pub struct HtmlWithTail {
    pub head: String,
    pub body: String,
    pub tail: String,
}

impl HtmlWithTail {
    pub fn load(path: &Path, delimiter: &str) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("Cannot find the path: {path:?}"))?;
        let reader = read_to_string(BufReader::new(file))
            .with_context(|| format!("Cannot read the file {path:?}"))?;
        Self::load_by_tokenizer(Tokenizer::new(&reader), delimiter)
    }

    pub fn load_by_tokenizer(
        mut tokenizer: Tokenizer<StringReader>,
        delimiter: &str,
    ) -> Result<Self> {
        let mut head = String::new();
        let mut body = String::new();
        parse_within_first(&mut tokenizer, b"head", |token| {
            write_token(&mut head, &token)
        })?;
        parse_within_first(&mut tokenizer, b"body", |token| {
            write_token(&mut body, &token)
        })?;
        Ok(Self::new(head, body, delimiter))
    }

    fn new(head: String, body: String, delimiter: &str) -> Self {
        let head = ac_replace(&head, &[("../assets", "")]);
        let tail;
        let mut body = body;
        if let Some((pre, post)) = body.split_once(delimiter) {
            tail = post.to_string();
            body = pre.to_string();
        } else {
            tail = String::new();
        }
        Self { head, body, tail }
    }
}

pub struct HtmlWithElem<E: Elem> {
    pub head: String,
    pub body: Vec<E>,
}

impl<E: Elem> HtmlWithElem<E> {
    pub fn load(html_elem_path: &Path) -> Result<Self> {
        let html = Html::load(html_elem_path)?;
        let head = html.head;
        let body: Vec<E> = ElemTokenizer::from::<E>(&html.body).collect();
        Ok(Self { head, body })
    }
}

#[macro_export]
macro_rules! write_into {
    ($dst:expr, $($arg:tt)*) => {
        $dst.write_fmt(format_args!($($arg)*)).with_context(|| format!("Cannot write to {:?}",$dst))
    };
}

pub fn html_as_str(html_str: &HtmlString) -> Cow<'_, str> {
    String::from_utf8_lossy(&html_str.0)
}

pub fn write_token(html: &mut String, token: &Token) -> Result<()> {
    match token {
        Token::StartTag(tag) => {
            let name = html_as_str(&tag.name);
            write_into!(html, "<{}", name)?;
            let attrs = &tag.attributes;
            if !attrs.is_empty() {
                for (key, value) in attrs {
                    write_into!(html, " {}=\"{}\"", html_as_str(key), html_as_str(value))?;
                }
            }
            if !tag.self_closing {
                write_into!(html, ">")?;
            } else {
                write_into!(html, "/>")?;
            }
        }
        Token::String(hello_world) => {
            write_into!(html, "{}", html_as_str(hello_world))?;
        }
        Token::EndTag(tag) => {
            write_into!(html, "</{}>", html_as_str(&tag.name))?;
        }
        _ => {}
    }
    Ok(())
}

pub fn expect_start(tokenizer: &mut PeekableTokenizer, name: &str) -> Result<()> {
    for next in tokenizer.by_ref() {
        let next = next.context("Error occurred while tokenizing")?;
        match next {
            Token::StartTag(tag) if tag.name == name.as_bytes() => return Ok(()),
            _ => {}
        }
    }
    Err(anyhow!("Expect a start tag of `{name}`"))
}

pub fn parse_within_first<F>(
    tokenizer: &mut Tokenizer<StringReader>,
    target: &[u8],
    f: F,
) -> Result<()>
where
    F: FnMut(Token) -> Result<()>,
{
    // skip until the first target tag
    for token in tokenizer.by_ref() {
        match token {
            Ok(Token::StartTag(tag)) if tag.name == target => break,
            Ok(_) => {}
            Err(e) => return Err(TypsiteError::HtmlParse(e).into()),
        }
    }
    parse_until_end(tokenizer, target, f)
}

pub fn parse_first_tag<F>(
    tokenizer: &mut Tokenizer<StringReader>,
    target: &[u8],
    mut f: F,
) -> Result<()>
where
    F: FnMut(Token) -> Result<()>,
{
    // skip until the first target tag
    for result in tokenizer.by_ref() {
        match result {
            Ok(token) => match &token {
                Token::StartTag(tag) if tag.name == target => {
                    if tag.self_closing {
                        return f(token);
                    } else {
                        f(token)?;
                        break;
                    }
                }
                _ => {}
            },
            Err(e) => return Err(TypsiteError::HtmlParse(e).into()),
        }
    }
    let mut count = 0;
    for token in tokenizer {
        match token {
            // count the number of target start tag
            Ok(Token::StartTag(tag)) if !tag.self_closing && tag.name == target => count += 1,
            // stop when reach the end of the first target tag
            Ok(Token::EndTag(tag)) if tag.name == target => {
                if count == 0 {
                    break;
                } else {
                    count -= 1
                }
            }
            Ok(_) => {}
            Err(e) => return Err(TypsiteError::HtmlParse(e).into()),
        }
    }
    Ok(())
}

pub fn parse_until_end<F>(
    tokenizer: &mut Tokenizer<StringReader>,
    target: &[u8],
    mut f: F,
) -> Result<()>
where
    F: FnMut(Token) -> Result<()>,
{
    let mut count = 0;
    for result in tokenizer {
        let result = result.map_err(TypsiteError::HtmlParse)?;
        match &result {
            // count the number of target start tag
            Token::StartTag(tag) if !tag.self_closing && tag.name == target => count += 1,
            // stop when reach the end of the first target tag
            Token::EndTag(tag) if tag.name == target => {
                if count == 0 {
                    break;
                } else {
                    count -= 1
                }
            }
            _ => {}
        }
        f(result)?;
    }
    Ok(())
}

pub fn html(head: &str, body: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en-US\">\n<head>\n{head}\n</head>\n<body>\n{body}\n</body>\n</html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_serialize() {
        let attrs = Attributes::new(
            vec![
                (
                    HtmlString::from("class".as_bytes().to_vec()),
                    HtmlString::from("test".as_bytes().to_vec()),
                ),
                (
                    HtmlString::from("id".as_bytes().to_vec()),
                    HtmlString::from("test".as_bytes().to_vec()),
                ),
                (
                    HtmlString::from("cn".as_bytes().to_vec()),
                    HtmlString::from("中文".as_bytes().to_vec()),
                ),
            ]
            .into_iter()
            .collect(),
        );
        assert_eq!(
            "{\"class\":\"test\",\"cn\":\"中文\",\"id\":\"test\"}",
            serde_json::to_string(&attrs).unwrap()
        );
    }
    #[test]
    fn attributes_deserialize() {
        let attrs = Attributes::new(
            vec![
                (
                    HtmlString::from("class".as_bytes().to_vec()),
                    HtmlString::from("test".as_bytes().to_vec()),
                ),
                (
                    HtmlString::from("id".as_bytes().to_vec()),
                    HtmlString::from("test".as_bytes().to_vec()),
                ),
                (
                    HtmlString::from("cn".as_bytes().to_vec()),
                    HtmlString::from("中文".as_bytes().to_vec()),
                ),
            ]
            .into_iter()
            .collect(),
        );
        let json = serde_json::to_string(&attrs).unwrap();
        let de_attrs: Attributes = serde_json::from_str(&json).unwrap();
        assert_eq!(de_attrs.to_string(), attrs.to_string());
    }
}
