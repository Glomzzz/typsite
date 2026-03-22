use crate::config::{TypsiteConfig, RULES_DIR};
use crate::ir::article::data::GlobalData;
use crate::ir::article::dep::Source;
use crate::pass::pure::PurePass;
use crate::pass::rewrite::*;
use crate::util::error::log_err_or_ok;
use crate::util::html::parse_first_tag;
use crate::util::html::Attributes;
use crate::util::html::HtmlWithTail;
use crate::util::path::file_stem;
use crate::walk_glob;
use anyhow::{anyhow, Context};
use glob::glob;
use html5gum::{Token, Tokenizer};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct RulesConfig {
    rules: HashMap<String, TagRewriteRule>,
}

impl RulesConfig {
    pub fn load(config_path: &Path) -> anyhow::Result<Self> {
        let rules_path = config_path.join(RULES_DIR);
        let mut rules = HashMap::new();
        let load_rules = walk_glob!("{}/**/*.html", rules_path.display())
            .par_bridge()
            .map(TagRewriteRule::load_rule)
            .collect::<Vec<anyhow::Result<(String, TagRewriteRule)>>>();
        rules.extend(load_rules.into_iter().filter_map(log_err_or_ok));
        let metacontent = TagRewriteRule::default(METACONTENT_TAG);

        if let Some(rule) = metacontent {
            rules.insert(METACONTENT_TAG.to_string(), rule);
        }

        anyhow::Ok(Self { rules })
    }
    pub fn get(&self, tag: &str) -> Option<&TagRewriteRule> {
        self.rules.get(tag)
    }

    pub fn rule_name(&self, tag: &str) -> Option<&str> {
        self.rules.get_key_value(tag).map(|(k, _)| k.as_str())
    }
}

pub struct TagRewriteRule {
    pub path: Option<Arc<Path>>,
    pub head: String,
    pub pass: Arc<dyn TagRewritePass>,
    pub body: String,
    pub tail: String,
}

impl<'b, 'a: 'b> TagRewriteRule {
    fn default(rewrite_pass: &str) -> Option<Self> {
        let pass = find_rewrite_pass(rewrite_pass)?;
        let path = None;
        let head = String::new();
        let body = String::new();
        let tail = String::new();
        let rule = Self {
            path,
            pass,
            head,
            body,
            tail,
        };
        Some(rule)
    }

    fn load_rule(path: PathBuf) -> anyhow::Result<(String, Self)> {
        let mut tag: Option<String> = None;
        let mut pass: Option<String> = None;

        let content = std::fs::read_to_string(&path)?;
        let mut tokenizer = Tokenizer::new(content.as_str());

        parse_first_tag(&mut tokenizer, b"rewrite", |token| match token {
            Token::StartTag(start) => {
                let mut attr = Attributes::new(start.attributes);
                tag = file_stem(&path).map(|s| s.to_string());
                pass = Some(attr.take("pass")?);
                anyhow::Ok(())
            }
            t => Err(anyhow!("Unexpected token {:?} in rewrite tag", t)),
        })?;

        let tag = tag.with_context(|| format!("Missing rewrite tag name in {}", path.display()))?;
        let pass = pass.with_context(|| format!("Missing rewrite pass in {}", path.display()))?;

        let (head, body, tail) = HtmlWithTail::load_by_tokenizer(tokenizer, "{body}")
            .map(|HtmlWithTail { head, body, tail }| (head, body, tail))?;

        let pass =
            find_rewrite_pass(&pass).with_context(|| format!("No rewrite pass called {pass}"))?;

        let path = Some(Arc::from(path));
        let rule = Self {
            path,
            pass,
            head,
            body,
            tail,
        };
        anyhow::Ok((tag, rule))
    }
    pub fn init(
        &self,
        attrs: Attributes,
        passor: &mut PurePass,
    ) -> anyhow::Result<HashMap<String, String>> {
        self.pass.init(attrs, passor)
    }

    pub fn dependents(
        &self,
        attrs: &HashMap<String, String>,
        passor: &PurePass<'a, '_>,
    ) -> anyhow::Result<HashSet<Source>> {
        self.pass.dependents(attrs, passor)
    }

    pub fn pure_start(
        &self,
        attrs: &HashMap<String, String>,
        config: &TypsiteConfig,
    ) -> Option<String> {
        self.pass.pure_start(attrs, config, &self.body)
    }
    pub fn pure_end(
        &self,
        attrs: &HashMap<String, String>,
        config: &TypsiteConfig,
    ) -> Option<String> {
        self.pass.pure_end(attrs, config, &self.tail)
    }

    pub fn impure_start<'c>(
        &self,
        attrs: &HashMap<String, String>,
        global_data: &'c GlobalData<'a, 'b, 'c>,
    ) -> Option<String> {
        self.pass.impure_start(attrs, global_data, &self.body)
    }
    pub fn impure_end<'c>(
        &self,
        attrs: &HashMap<String, String>,
        global_data: &'c GlobalData<'a, 'b, 'c>,
    ) -> Option<String> {
        self.pass.impure_end(attrs, global_data, &self.tail)
    }
}
