use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter, Theme};
use syntect::html::{append_highlighted_html_for_styled_line, IncludeBackground};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

pub fn highlight(
    syntax_set: &SyntaxSet,
    syntax: &SyntaxReference,
    content: &str,
    theme: &Theme,
    fallback_color: &str,
) -> String {
    let highlighter = Highlighter::new(theme);
    let scope_stack = ScopeStack::new();
    let mut highlight_state = HighlightState::new(&highlighter, scope_stack);

    let mut output = String::new();

    // Init the parser
    let mut parse_state = ParseState::new(syntax);
    // Iterate over the lines of the content
    for line in LinesWithEndings::from(content) {
        // Try to parse the line
        let ops = match parse_state.parse_line(line, syntax_set) {
            Ok(ops) => ops,
            Err(_) => {
                //  If the line can't be parsed, reset the parser and apply a fallback style
                // 1. Reset the parser
                parse_state = ParseState::new(syntax);
                // 2. Apply a fallback style
                output.push_str(&apply_fallback_style(fallback_color, line));
                continue;
            }
        };

        let styles = HighlightIterator::new(&mut highlight_state, &ops, line, &highlighter);

        // Append the highlighted line to the output HTML
        for (style, text) in styles {
            append_highlighted_html_for_styled_line(
                &[(style, text)],
                IncludeBackground::No,
                &mut output,
            )
            .expect("writing highlighted HTML into a String should be infallible");
        }
    }
    output
}

fn apply_fallback_style(color: &str, text: &str) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;");
    format!("<span style='color:{color};'>{escaped}</span>")
}
