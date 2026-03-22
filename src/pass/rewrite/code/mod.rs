use crate::pass::pure::PurePass;
use crate::pass::rewrite::code::highlighter::highlight;
use crate::pass::rewrite::*;
use crate::util::html::Attributes;
use crate::util::str::ac_replace;
use crate::{compile::proj_options, config::TypsiteConfig};
use std::collections::{HashMap, HashSet};
use typsite_macros::rewrite_pass;

pub mod highlighter;

rewrite_pass![CodeBlockPass, id = "code", atom = true];

fn normalized_lang(attrs: &HashMap<String, String>) -> &str {
    attrs
        .get("lang")
        .map(|value| value.as_str())
        .expect("code rewrite pass should always carry a normalized `lang` attribute after init")
}

fn normalized_theme(attrs: &HashMap<String, String>) -> &str {
    attrs
        .get("theme")
        .map(|value| value.as_str())
        .expect("code rewrite pass should always carry a normalized `theme` attribute after init")
}

fn normalized_content(attrs: &HashMap<String, String>) -> &str {
    attrs
        .get("content")
        .map(|value| value.as_str())
        .expect("code rewrite pass should always carry a normalized `content` attribute after init")
}

impl TagRewritePass for CodeBlockPass {
    fn init(&self, mut attrs: Attributes, _: &mut PurePass) -> Result<HashMap<String, String>> {
        let lang = attrs.take("lang")?;
        let theme = attrs.take("theme").unwrap_or("onedark".into());
        let content = attrs.take("content")?;
        Ok([
            ("lang".into(), lang.to_string()),
            ("theme".into(), theme.to_string()),
            ("content".into(), content.to_string()),
        ]
        .into_iter()
        .collect())
    }

    fn dependents<'a>(
        &self,
        attrs: &HashMap<String, String>,
        pass: &PurePass<'a, '_>,
    ) -> Result<HashSet<Source>> {
        let mut path = HashSet::new();
        let lang = normalized_lang(attrs);
        let config = &pass.config.highlight;
        if !config.is_syntax_by_default(lang) {
            let syntax = config
                .find_syntax_path(lang)
                .with_context(|| format!("Can't find syntax path for lang {lang}"))?;
            path.insert(Source::Path(syntax));
            for metadata_path in config.metadata_paths() {
                path.insert(Source::Path(metadata_path));
            }
        }
        let theme = normalized_theme(attrs);
        let (light, dark) = config.find_theme_path_pair(theme).with_context(|| {
            format!("Can't find theme path pair(light & dark) for theme {theme}")
        })?;
        path.insert(Source::Path(light.clone()));
        path.insert(Source::Path(dark.clone()));
        Ok(path)
    }

    fn pure_start(
        &self,
        attrs: &HashMap<String, String>,
        config: &TypsiteConfig,
        body: &str,
    ) -> Option<String> {
        let lang = normalized_lang(attrs);
        let theme = normalized_theme(attrs);
        let content = normalized_content(attrs);
        let syntax = config.highlight.find_syntax(lang);
        let syntax_set = config.highlight.syntax_set(lang);
        let (light, dark) = config.highlight.find_theme(theme).expect(
            "code rewrite pass should only render with themes validated during dependency discovery",
        );
        let fallback = proj_options()
            .expect("project options should be initialized before code highlighting")
            .code_fallback_style
            .clone();
        let light = highlight(syntax_set, syntax, content, light, &fallback.light);
        let dark = highlight(syntax_set, syntax, content, dark, &fallback.dark);
        Some(ac_replace(
            body,
            &[("{content-light}", &light), ("{content-dark}", &dark)],
        ))
    }
}
