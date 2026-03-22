use crate::pass::pure::PurePass;
use crate::pass::rewrite::*;
use crate::util::html::Attributes;
use crate::util::str::ac_replace;
use anyhow::anyhow;
use std::collections::HashMap;
use typsite_macros::rewrite_pass;

rewrite_pass![FootnoteRef, id = "footnote-ref", atom = true, pure = false];
impl TagRewritePass for FootnoteRef {
    fn init(&self, attrs: Attributes, _: &mut PurePass) -> Result<HashMap<String, String>> {
        let name = attrs.get("name");
        if name.is_none() {
            return Err(anyhow!("FootnoteRefRule: expect name attribute"));
        }
        let Some(name) = name else {
            return Err(anyhow!("FootnoteRefRule: expect name attribute"));
        };
        Ok([(String::from("name"), name.to_string())]
            .into_iter()
            .collect())
    }

    fn build_attr(
        &self,
        mut attrs: HashMap<String, String>,
        data: &PurePassData,
    ) -> Result<HashMap<String, String>> {
        let name = &attrs["name"];
        let numbering = data
            .footnotes
            .get_numbering(name)
            .with_context(|| format!("FootnoteRefRule: no such footnote called {name}"))?;
        let numbering = numbering.to_string();
        attrs.insert(String::from("numbering"), numbering);
        Ok(attrs)
    }

    fn impure_start<'c, 'b: 'c, 'a: 'b>(
        &self,
        attrs: &HashMap<String, String>,
        _: &'c GlobalData<'a, 'b, 'c>,
        body: &str,
    ) -> Option<String> {
        let name = &attrs["name"];
        let numbering = &attrs["numbering"];
        footnote_ref(name.as_str(), numbering.as_str(), body)
    }

    fn impure_end<'c, 'b: 'c, 'a: 'b>(
        &self,
        attrs: &HashMap<String, String>,
        _: &'c GlobalData<'a, 'b, 'c>,
        tail: &str,
    ) -> Option<String> {
        let name = &attrs["name"];
        let numbering = &attrs["numbering"];
        footnote_ref(name.as_str(), numbering.as_str(), tail)
    }
}

fn footnote_ref<'c, 'b: 'c, 'a: 'b>(name: &str, numbering: &str, text: &str) -> Option<String> {
    let text = ac_replace(text, &[("{name}", name), ("{numbering}", numbering)]);
    Some(text)
}

rewrite_pass![FootnoteDef, id = "footnote-def", atom = false, pure = true];
impl TagRewritePass for FootnoteDef {
    fn init(&self, attrs: Attributes, pure: &mut PurePass) -> Result<HashMap<String, String>> {
        let name = attrs.get("name");
        if name.is_none() {
            return Err(anyhow!("FootnoteDefRule: expect name attribute"));
        }
        let Some(name) = name else {
            return Err(anyhow!("FootnoteDefRule: expect name attribute"));
        };
        let (name, numbering) = pure.add_footnote(name.to_string());
        Ok([
            (String::from("name"), name.to_string()),
            (String::from("numbering"), numbering.to_string()),
        ]
        .into_iter()
        .collect())
    }

    fn pure_start(
        &self,
        attrs: &HashMap<String, String>,
        _: &TypsiteConfig,
        body: &str,
    ) -> Option<String> {
        let name = &attrs["name"];
        let numbering = &attrs["numbering"];
        footnote_def(name.as_str(), numbering.as_str(), body)
    }

    fn pure_end(
        &self,
        attrs: &HashMap<String, String>,
        _: &TypsiteConfig,
        tail: &str,
    ) -> Option<String> {
        let name = &attrs["name"];
        let numbering = &attrs["numbering"];
        footnote_def(name.as_str(), numbering.as_str(), tail)
    }
}

fn footnote_def<'c, 'b: 'c, 'a: 'b>(name: &str, numbering: &str, text: &str) -> Option<String> {
    let text = ac_replace(text, &[("{name}", name), ("{numbering}", numbering)]);
    Some(text)
}
