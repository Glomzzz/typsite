use crate::pass::pure::PurePass;
use crate::pass::rewrite::*;
use crate::util::html::Attributes;
use anyhow::anyhow;
use std::collections::{HashMap, HashSet};
use typsite_macros::rewrite_pass;

rewrite_pass![CitePass, id = "cite", atom = false, pure = false];
impl TagRewritePass for CitePass {
    fn init(
        &self,
        attrs: Attributes,
        pass: &mut PurePass,
    ) -> anyhow::Result<HashMap<String, String>> {
        let slug = attrs.get("slug");
        if slug.is_none() {
            return Err(anyhow!("CiteRule: expect slug attribute"));
        }
        let slug = slug.unwrap();
        let slug = pass.resolve_slug(slug.as_ref(), "Cite")?;
        pass.metadata.add_cite(slug.clone());
        let anchor = attrs
            .get("anchor")
            .map(|s| s.to_string())
            .unwrap_or_default();
        Ok([
            (String::from("slug"), slug.to_string()),
            (String::from("anchor"), anchor),
        ]
        .into_iter()
        .collect())
    }

    fn dependents<'a>(
        &self,
        attrs: &HashMap<String, String>,
        pass: &PurePass<'a, '_>,
    ) -> Result<HashSet<Source>> {
        let slug = &attrs["slug"];
        let slug = pass.registry.know(slug.to_string(), "Cite", &pass.slug)?;
        Ok([Source::Article(slug)].into_iter().collect())
    }

    fn impure_start<'c, 'b: 'c, 'a: 'b>(
        &self,
        attrs: &HashMap<String, String>,
        global_data: &'c GlobalData<'a, 'b, 'c>,
        body: &str,
    ) -> Option<String> {
        let slug = &attrs["slug"];
        let anchor = &attrs["anchor"];
        cite(slug.as_str(), anchor.as_str(), global_data, body)
    }

    fn impure_end<'c, 'b: 'c, 'a: 'b>(
        &self,
        attrs: &HashMap<String, String>,
        global_data: &'c GlobalData<'a, 'b, 'c>,
        tail: &str,
    ) -> Option<String> {
        let slug = &attrs["slug"];
        let anchor = &attrs["anchor"];
        cite(slug.as_str(), anchor.as_str(), global_data, tail)
    }
}

fn cite<'c, 'b: 'c, 'a: 'b>(
    slug: &str,
    anchor: &str,
    global_data: &'c GlobalData<'a, 'b, 'c>,
    text: &str,
) -> Option<String> {
    let url = if anchor.is_empty() {
        slug.to_string()
    } else {
        format!("{slug}#{anchor}")
    };
    global_data
        .metadata(slug)
        .map(|metadata| metadata.inline_with(text, &[("{url}", url.as_str())]))
}
