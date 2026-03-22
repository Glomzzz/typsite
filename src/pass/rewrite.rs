use crate::compile::registry::Key;
use crate::config::TypsiteConfig;
use crate::ir::article::data::GlobalData;
use crate::ir::article::dep::{Indexes, Source};
use crate::ir::rewriter::{BodyRewriter, MetaRewriter, RewriterType};
use crate::pass::pure::{PurePass, PurePassData};
use crate::util::html::Attributes;
use anyhow::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, LazyLock, Mutex};

mod cite;
mod code;
mod footnote;
mod metacontent;

mod footnote_ref_svg;

pub const METACONTENT_TAG: &str = "metacontent";

static REWRITE_PASSES: LazyLock<Arc<Mutex<HashMap<String, Arc<dyn TagRewritePass>>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

pub fn register_rewrite_pass(rewriter: impl TagRewritePass + 'static) {
    let tag = rewriter.id().to_string();
    let arc_instance = Arc::new(rewriter);
    REWRITE_PASSES
        .lock()
        .expect("rewrite pass registry mutex should not be poisoned")
        .insert(tag, arc_instance);
}

pub fn find_rewrite_pass(tag: &str) -> Option<Arc<dyn TagRewritePass>> {
    REWRITE_PASSES
        .lock()
        .expect("rewrite pass registry mutex should not be poisoned")
        .get(tag)
        .map(Arc::clone)
}

pub trait Id {
    fn id(&self) -> &str;
}

pub trait Atom {
    // If atom, its children WILL NOT be passed by the rewriter management
    // If not atom, you should impl the xxx_end method
    fn atom(&self) -> bool {
        true
    }
}

pub trait Purity {
    // If rewrite, it can pass HTML with MetaData, but needs to wait for the metadata to be loaded
    fn pure(&self) -> bool {
        true
    }
}

#[allow(unused_variables)]
pub trait TagRewritePass: Id + Atom + Send + Sync + Purity {
    // Initialize the pass, return the attributes to be passed to the following functions
    fn init(&self, attrs: Attributes, pass: &mut PurePass) -> Result<HashMap<String, String>>;

    // Build the attributes, called when the whole HTML is passed
    fn build_attr(
        &self,
        attrs: HashMap<String, String>,
        data: &PurePassData,
    ) -> Result<HashMap<String, String>> {
        Ok(attrs)
    }

    // Return the dependents of the pass rule, if any
    fn dependents<'a>(
        &self,
        attrs: &HashMap<String, String>,
        pass: &PurePass<'a, '_>,
    ) -> Result<HashSet<Source>> {
        Ok(HashSet::default())
    }

    // If pure, it can pass HTML without MetaData
    fn pure_start(
        &self,
        attrs: &HashMap<String, String>,
        config: &TypsiteConfig,
        body: &str,
    ) -> Option<String> {
        None
    }
    fn pure_end(
        &self,
        attrs: &HashMap<String, String>,
        config: &TypsiteConfig,
        tail: &str,
    ) -> Option<String> {
        None
    }
    // If rewrite, it can pass HTML with global articles' data, but needs to wait for the metadata to be loaded
    fn impure_start<'c, 'b: 'c, 'a: 'b>(
        &self,
        attrs: &HashMap<String, String>,
        global_data: &'c GlobalData<'a, 'b, 'c>,
        body: &str,
    ) -> Option<String> {
        None
    }
    fn impure_end<'c, 'b: 'c, 'a: 'b>(
        &self,
        attrs: &HashMap<String, String>,
        global_data: &'c GlobalData<'a, 'b, 'c>,
        tail: &str,
    ) -> Option<String> {
        None
    }
}
pub struct RewritePass<'a, 'b, 'c> {
    slug: Key,
    global_data: &'c GlobalData<'a, 'b, 'c>,
}

impl<'c, 'b: 'c, 'a: 'b> RewritePass<'a, 'b, 'c> {
    pub fn new(slug: Key, global_data: &'c GlobalData<'a, 'b, 'c>) -> Self {
        Self { slug, global_data }
    }

    fn visit_rewriter_start(
        &self,
        rewriter_id: &str,
        attributes: &HashMap<String, String>,
    ) -> Option<String> {
        let rewriter = self.global_data.config.rules.get(rewriter_id);
        match rewriter {
            None => {
                eprintln!(
                    "[WARN] Rewriter `{}` not found in {}",
                    rewriter_id, self.slug
                );
                Some(format!("<< Rewriter `{rewriter_id}` not found >>"))
            }
            Some(rewriter) => {
                if rewriter.pass.pure() {
                    rewriter.pure_start(attributes, self.global_data.config)
                } else {
                    rewriter.impure_start(attributes, self.global_data)
                }
            }
        }
    }

    fn visit_rewriter_end(
        &self,
        rewriter_id: &str,
        attributes: &HashMap<String, String>,
    ) -> Option<String> {
        let rewriter = self.global_data.config.rules.get(rewriter_id);
        match rewriter {
            None => {
                eprintln!(
                    "[WARN] Rewriter `{}` not found in {}",
                    rewriter_id, self.slug
                );
                Some(format!("<< Rewriter `{rewriter_id}` not found >>"))
            }
            Some(rewriter) => {
                if rewriter.pass.pure() {
                    rewriter.pure_end(attributes, self.global_data.config)
                } else {
                    rewriter.impure_end(attributes, self.global_data)
                }
            }
        }
    }

    pub fn run_body(
        &self,
        body: &mut [String],
        sidebar: &mut [String],
        rewriters: &Vec<BodyRewriter>,
        indexes: &Indexes,
    ) {
        match indexes {
            Indexes::All => {
                for rewriter in rewriters {
                    self.body_rewriter_set_by_index(rewriter, body, sidebar);
                }
            }
            Indexes::Some(indexes) => {
                for index in indexes {
                    let rewriter = &rewriters[*index];
                    self.body_rewriter_set_by_index(rewriter, body, sidebar);
                }
            }
        }
    }

    pub fn run_meta(self, body: &mut [String], rewriters: &Vec<MetaRewriter>, indexes: &Indexes) {
        match indexes {
            Indexes::All => {
                for rewriter in rewriters {
                    self.meta_set_by_index(rewriter, body);
                }
            }
            Indexes::Some(indexes) => {
                for index in indexes {
                    let rewriter = &rewriters[*index];
                    self.meta_set_by_index(rewriter, body);
                }
            }
        }
    }

    fn body_rewriter_set_by_index(
        &self,
        rewriter: &BodyRewriter,
        body: &mut [String],
        sidebar: &mut [String],
    ) {
        let BodyRewriter {
            id: rule_id,
            rewriter_type,
            attributes,
            sidebar_indexes,
            body_index,
        } = rewriter;
        let (str, sidebar_indexes, index) = {
            let result = match rewriter_type {
                RewriterType::Start => self.visit_rewriter_start(rule_id, attributes),
                RewriterType::End => self.visit_rewriter_end(rule_id, attributes),
            };
            (result, sidebar_indexes, body_index)
        };
        if str.is_none() {
            return;
        }
        let Some(str) = str else {
            return;
        };
        for index in sidebar_indexes {
            sidebar[*index] = str.clone();
        }
        body[*index] = str;
    }
    fn meta_set_by_index(&self, atom: &MetaRewriter, contents: &mut [String]) {
        let MetaRewriter {
            id: rule_id,
            rewriter_type,
            attributes,
            body_index,
        } = atom;
        let (str, index) = {
            let result = match rewriter_type {
                RewriterType::Start => self.visit_rewriter_start(rule_id, attributes),
                RewriterType::End => self.visit_rewriter_end(rule_id, attributes),
            };
            (result, body_index)
        };
        if str.is_none() {
            return;
        }
        let Some(str) = str else {
            return;
        };
        contents[*index] = str;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::pass::tokenizer::{BodyTag, Event, EventTokenizer, Tokenizer};

    pub(crate) fn run_rewrite_pass_reports_missing_required_attr() {
        let html = "<body><rewrite></rewrite></body>";
        let mut raw = html5gum::Tokenizer::new(html).peekable();
        let mut tokenizer = Tokenizer::<BodyTag>::new(&mut raw);
        let err = loop {
            match tokenizer.next() {
                Some(Err(err)) => break err,
                Some(Ok(Event::Eof)) => panic!("missing rewrite id should have produced an error"),
                Some(Ok(_)) => {}
                None => panic!("missing rewrite id should have produced an error"),
            }
        };

        assert!(err.to_string().contains("Rewrite: expect id attribute"));
    }

    #[test]
    fn rewrite_pass_reports_missing_required_attr() {
        run_rewrite_pass_reports_missing_required_attr();
    }
}
