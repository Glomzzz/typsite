use crate::pass::pure::PurePass;
use crate::pass::rewrite::*;
use crate::util::html::Attributes;
use crate::util::str::ac_replace;
use anyhow::anyhow;
use std::collections::HashMap;
use typsite_macros::rewrite_pass;

rewrite_pass![
    FootnoteRef,
    id = "footnote-ref-svg",
    atom = false,
    pure = false
];
impl TagRewritePass for FootnoteRef {
    fn init(&self, attrs: Attributes, _: &mut PurePass) -> Result<HashMap<String, String>> {
        let name = attrs.get("name");
        if name.is_none() {
            return Err(anyhow!(
                "FootnoteRefSvgRule: expect name attribute, found: {:?}",
                attrs
            ));
        }
        let transform = attrs.get("transform");
        if transform.is_none() {
            return Err(anyhow!(
                "FootnoteRefSvgRule: expect transform attribute, found: {:?}",
                attrs
            ));
        }
        let name = name.unwrap();
        let transform = transform.unwrap();
        Ok([
            (String::from("name"), name.to_string()),
            (String::from("transform"), transform.to_string()),
        ]
        .into_iter()
        .collect())
    }

    fn impure_start<'c, 'b: 'c, 'a: 'b>(
        &self,
        attrs: &HashMap<String, String>,
        _: &'c GlobalData<'a, 'b, 'c>,
        body: &str,
    ) -> Option<String> {
        let name = &attrs["name"];
        let transform = &attrs["transform"];
        footnote_ref_svg(name.as_str(), transform.as_str(), body)
    }

    fn impure_end<'c, 'b: 'c, 'a: 'b>(
        &self,
        attrs: &HashMap<String, String>,
        _: &'c GlobalData<'a, 'b, 'c>,
        tail: &str,
    ) -> Option<String> {
        let name = &attrs["name"];
        let transform = &attrs["transform"];
        footnote_ref_svg(name.as_str(), transform.as_str(), tail)
    }
}

fn footnote_ref_svg<'c, 'b: 'c, 'a: 'b>(name: &str, transform: &str, text: &str) -> Option<String> {
    let text = ac_replace(text, &[("{name}", name), ("{transform}", transform)]);
    Some(text)
}
