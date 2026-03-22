use crate::compile::error::TypResult;
use crate::compile::registry::{Key, KeyRegistry, SlugPath};
use crate::config::schema::Schema;
use crate::config::TypsiteConfig;
use crate::ir::article::data::GlobalData;
use crate::ir::article::dep::Indexes;
use crate::ir::article::Article;
use crate::ir::embed::Embed;
use crate::ir::pending::Pending;
use crate::ir::rewriter::{BodyRewriter, MetaRewriter};
use crate::pass::pending::PendingPass;
use crate::pass::pure::PurePass;
use crate::pass::rewrite::RewritePass;
use crate::pass::schema::SchemaPass;
use crate::util::html::OutputHtml;
use html5gum::Tokenizer as HtmlTokenizer;

mod pending;
pub mod pure;
pub mod rewrite;
pub(crate) mod schema;
pub mod tokenizer;

pub fn pass_pure<'b, 'a: 'b, 'k, 'c>(
    config: &'a TypsiteConfig,
    registry: &'k KeyRegistry,
    path: SlugPath,
    slug: Key,
    cache: Option<&'k Article<'a>>,
    src: &'c str,
) -> TypResult<Article<'a>> {
    let tokenizer = HtmlTokenizer::new(src);
    PurePass::new(config, registry, path, slug, cache).run(tokenizer)
}

pub fn pass_rewriter_body<'c, 'b: 'c, 'a: 'b>(
    slug: Key,
    body: &mut [String],
    sidebar: &mut [String],
    rewriters: &Vec<BodyRewriter>,
    indexes: &Indexes,
    global_data: &'c GlobalData<'a, 'b, 'c>,
) {
    RewritePass::new(slug, global_data).run_body(body, sidebar, rewriters, indexes);
}

pub fn pass_embed<'c, 'b: 'c, 'a: 'b>(
    slug: Key,
    content: &'c (Vec<String>, Vec<String>, Vec<String>),
    embeds: &[Embed],
    indexes: &Indexes,
    global_data: &'c GlobalData<'a, 'b, 'c>,
) -> Pending<'c> {
    PendingPass::new(slug, global_data).run(content, embeds, indexes)
}

pub fn pass_rewriter_meta<'c, 'b: 'c, 'a: 'b>(
    slug: Key,
    contents: &mut [String],
    rewriters: &Vec<MetaRewriter>,
    indexes: &Indexes,
    global_data: &'c GlobalData<'a, 'b, 'c>,
) {
    RewritePass::new(slug, global_data).run_meta(contents, rewriters, indexes);
}

pub fn pass_schema<'c, 'b: 'c, 'a: 'b>(
    config: &'a TypsiteConfig,
    schema: &'a Schema,
    article: &'c Article<'a>,
    content: &str,
    sidebar: &str,
    global_data: &'c GlobalData<'a, 'b, 'c>,
) -> TypResult<OutputHtml<'a>> {
    SchemaPass::new(config, schema, article, content, sidebar, global_data).run()
}

#[cfg(test)]
pub(crate) mod tests {
    pub(crate) fn run_pending_pass_skips_missing_embed_article_without_panicking() {
        super::pending::tests::run_pending_pass_skips_missing_embed_article_without_panicking();
    }
}
