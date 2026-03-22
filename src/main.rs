use crate::cli::cli;

pub(crate) mod cli;
pub(crate) mod compile;
pub(crate) mod config;
pub(crate) mod ir;
pub(crate) mod pass;
pub(crate) mod resource;
pub(crate) mod util;

#[allow(dead_code)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli().await
}

#[cfg(test)]
fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};

    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().expect("test lock should not be poisoned")
}

#[cfg(test)]
#[test]
fn article_from_collects_registry_and_schema_errors() {
    let _guard = test_lock();
    crate::ir::article::tests::run_article_from_collects_registry_and_schema_errors();
}

#[cfg(test)]
#[test]
fn config_loader_reports_missing_section_placeholders() {
    let _guard = test_lock();
    crate::config::section::tests::run_config_loader_reports_missing_section_placeholders();
}

#[cfg(test)]
#[test]
fn schema_pass_reports_missing_metadata_tag_attr() {
    let _guard = test_lock();
    crate::pass::schema::tests::run_schema_pass_reports_missing_metadata_tag_attr();
}

#[cfg(test)]
#[test]
fn meta_contents_init_replacement_handles_root_slug_and_parent_defaults() {
    let _guard = test_lock();
    crate::ir::metadata::content::tests::run_meta_contents_init_replacement_handles_root_slug_and_parent_defaults();
}

#[cfg(test)]
#[test]
fn ac_replacement_handles_empty_patterns_without_unwrap() {
    let _guard = test_lock();
    crate::util::str::tests::run_ac_replacement_handles_empty_patterns_without_unwrap();
}

#[cfg(test)]
#[test]
fn sidebar_builder_reports_invalid_parent_section() {
    let _guard = test_lock();
    crate::pass::pure::tests::run_sidebar_builder_reports_invalid_parent_section();
}

#[cfg(test)]
#[test]
fn pos_slug_rejects_missing_leading_slash() {
    let _guard = test_lock();
    crate::util::tests::run_pos_slug_rejects_missing_leading_slash();
}

#[cfg(test)]
#[test]
fn pure_pass_reports_missing_sidebar_title_slot() {
    let _guard = test_lock();
    crate::pass::pure::tests::run_pure_pass_reports_missing_sidebar_title_slot();
}

#[cfg(test)]
#[test]
fn metadata_builder_reports_unbalanced_metacontent_state() {
    let _guard = test_lock();
    crate::pass::pure::tests::run_metadata_builder_reports_unbalanced_metacontent_state();
}

#[cfg(test)]
#[test]
fn rewrite_pass_reports_missing_required_attr() {
    let _guard = test_lock();
    crate::pass::rewrite::tests::run_rewrite_pass_reports_missing_required_attr();
}

#[cfg(test)]
#[test]
fn compose_pages_collects_missing_article_as_error() {
    let _guard = test_lock();
    crate::compile::compiler::tests::run_compose_pages_collects_missing_article_as_error();
}

#[cfg(test)]
#[test]
fn pass_html_collects_registry_errors_without_unreachable_panics() {
    let _guard = test_lock();
    crate::compile::compiler::tests::run_pass_html_collects_registry_errors_without_unreachable_panics();
}

#[cfg(test)]
#[test]
fn compile_typsts_reports_missing_cache_parent_without_panic() {
    let _guard = test_lock();
    crate::compile::compiler::tests::run_compile_typsts_reports_missing_cache_parent_without_panic();
}

#[cfg(test)]
#[test]
fn watch_response_errors_are_logged_not_panicked() {
    let _guard = test_lock();
    crate::compile::watch::tests::run_watch_response_errors_are_logged_not_panicked();
}

#[cfg(test)]
#[test]
fn initialize_reports_packages_path_and_strip_prefix_failures() {
    let _guard = test_lock();
    crate::compile::compiler::tests::run_initialize_reports_packages_path_and_strip_prefix_failures();
}

#[cfg(test)]
#[test]
fn generate_site_outputs_handles_empty_base_url_without_unwrap() {
    let _guard = test_lock();
    crate::compile::compiler::tests::run_generate_site_outputs_handles_empty_base_url_without_unwrap();
}

#[cfg(test)]
#[test]
fn tokenizer_parses_svg_anchor_without_strip_prefix_panic() {
    let _guard = test_lock();
    crate::pass::tokenizer::tests::run_tokenizer_parses_svg_anchor_without_strip_prefix_panic();
}

#[cfg(test)]
#[test]
fn pending_pass_skips_missing_embed_article_without_panicking() {
    let _guard = test_lock();
    crate::pass::tests::run_pending_pass_skips_missing_embed_article_without_panicking();
}

#[cfg(test)]
#[test]
fn global_data_reports_missing_rewrite_indexes() {
    let _guard = test_lock();
    crate::ir::article::data::tests::run_global_data_reports_missing_rewrite_indexes();
}

#[cfg(test)]
#[test]
fn heading_numbering_roman_overflow_is_reported() {
    let _guard = test_lock();
    crate::config::heading_numbering::tests::run_heading_numbering_roman_overflow_is_reported();
}
