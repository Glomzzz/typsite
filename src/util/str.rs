use aho_corasick::{AhoCorasick, MatchKind};
use std::iter::Peekable;
use std::mem;
use std::str::Chars;

fn ac_replacement(patterns: &[&str]) -> Option<AhoCorasick> {
    if patterns.is_empty() {
        return None;
    }

    AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostLongest)
        .build(patterns)
        .ok()
}

pub fn ac_replace_map(text: &str, (patterns, values): (Vec<&str>, Vec<&str>)) -> String {
    ac_replacement(&patterns)
        .map(|matcher| matcher.replace_all(text, &values))
        .unwrap_or_else(|| text.to_string())
}

pub fn ac_replace(text: &str, replacements: &[(&str, &str)]) -> String {
    ac_replace_map(text, replacements.iter().cloned().unzip())
}

pub trait Elem {
    fn parse_not_plain(token: &str) -> Self;
    fn plain(plain: String) -> Self;
}

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub enum SectionElem {
    Plain(String),
    Content,
    Level,
    Title,
    HeadingNumbering,
}

impl Elem for SectionElem {
    fn parse_not_plain(token: &str) -> SectionElem {
        match token.trim() {
            "title" | "embed-title" => SectionElem::Title,
            "content" => SectionElem::Content,
            "level" => SectionElem::Level,
            "numbering" => SectionElem::HeadingNumbering,
            _ => SectionElem::Plain(format!("{{{token}}}")),
        }
    }
    fn plain(plain: String) -> Self {
        SectionElem::Plain(plain)
    }
}

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub enum SidebarElem {
    Plain(String),
    Children,
    Anchor,
    Title,
    HeadingNumbering,
    ShowChildren,
}

impl Elem for SidebarElem {
    fn parse_not_plain(token: &str) -> SidebarElem {
        match token.trim() {
            "title" => SidebarElem::Title,
            "children" => SidebarElem::Children,
            "anchor" => SidebarElem::Anchor,
            "numbering" => SidebarElem::HeadingNumbering,
            "show-children" => SidebarElem::ShowChildren,
            _ => SidebarElem::Plain(format!("{{{token}}}")),
        }
    }
    fn plain(plain: String) -> Self {
        SidebarElem::Plain(plain)
    }
}
pub trait ElemTokenizerTrait<E: Elem> {
    fn flush_plain(&mut self) -> Option<E>;
    fn parse_braced_content(&mut self) -> E;
    fn next(&mut self) -> Option<E>;
    fn collect(self) -> Vec<E>;
}

pub struct ElemTokenizer<'a> {
    chars: Peekable<Chars<'a>>,
    buffer: String,
    brace_depth: u8,
}
impl<'a> ElemTokenizer<'a> {
    pub fn from<E: Elem>(s: &'a str) -> impl ElemTokenizerTrait<E> + use<'a, E> {
        Self {
            chars: s.chars().peekable(),
            buffer: String::new(),
            brace_depth: 0,
        }
    }
}

impl<E: Elem> ElemTokenizerTrait<E> for ElemTokenizer<'_> {
    fn flush_plain(&mut self) -> Option<E> {
        if self.buffer.is_empty() {
            None
        } else {
            let content = mem::take(&mut self.buffer);
            Some(E::plain(content))
        }
    }

    fn parse_braced_content(&mut self) -> E {
        let mut content = String::new();
        self.brace_depth = 1;

        for char in self.chars.by_ref() {
            match char {
                '{' => self.brace_depth += 1,
                '}' => {
                    self.brace_depth -= 1;
                    if self.brace_depth == 0 {
                        return E::parse_not_plain(content.trim());
                    }
                }
                _ => {}
            }
            content.push(char);
        }

        // 处理未闭合的括号
        E::plain(format!("{{{content}"))
    }

    fn next(&mut self) -> Option<E> {
        loop {
            match self.chars.peek() {
                Some('{') => {
                    let plain = self.flush_plain();
                    if plain.is_some() {
                        return plain;
                    }
                    self.chars.next();
                    let elem = self.parse_braced_content();
                    return Some(elem);
                }
                Some(_) => {
                    if let Some(next) = self.chars.next() {
                        self.buffer.push(next);
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }

        self.flush_plain()
    }

    fn collect(mut self) -> Vec<E> {
        let mut result = Vec::new();
        while let Some(elem) = self.next() {
            result.push(elem);
        }
        result
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn test_correct_parsing() {
        let cases = vec![
            ("{title}", vec![SidebarElem::Title]),
            (
                "prefix{children}suffix",
                vec![
                    SidebarElem::Plain("prefix".into()),
                    SidebarElem::Children,
                    SidebarElem::Plain("suffix".into()),
                ],
            ),
            ("{anchor}", vec![SidebarElem::Anchor]),
            (
                "{{invalid}}",
                vec![SidebarElem::Plain("{{invalid}}".into())],
            ),
            (
                "mixed{title}content{children}",
                vec![
                    SidebarElem::Plain("mixed".into()),
                    SidebarElem::Title,
                    SidebarElem::Plain("content".into()),
                    SidebarElem::Children,
                ],
            ),
            (
                "unclosed{title",
                vec![
                    SidebarElem::Plain("unclosed".into()),
                    SidebarElem::Plain("{title".into()),
                ],
            ),
            (
                "nested{{brace}}",
                vec![
                    SidebarElem::Plain("nested".into()),
                    SidebarElem::Plain("{{brace}}".into()),
                ],
            ),
            (
                "empty{}",
                vec![
                    SidebarElem::Plain("empty".into()),
                    SidebarElem::Plain("{}".into()),
                ],
            ),
        ];

        for (input, expected) in cases {
            let tokenizer = ElemTokenizer::from::<SidebarElem>(input);
            let result: Vec<SidebarElem> = tokenizer.collect();
            assert_eq!(result, expected, "Failed case: {}", input);
        }
    }

    #[test]
    fn test_edge_cases() {
        let empty = ElemTokenizer::from::<SidebarElem>("").collect();
        assert!(empty.is_empty());

        let only_braces = ElemTokenizer::from::<SidebarElem>("{{}}").collect();
        assert_eq!(only_braces, vec![SidebarElem::Plain("{{}}".into())]);
    }

    pub(crate) fn run_ac_replacement_handles_empty_patterns_without_unwrap() {
        assert_eq!(ac_replace("unchanged", &[]), "unchanged");
    }

    #[test]
    fn ac_replacement_handles_empty_patterns_without_unwrap() {
        run_ac_replacement_handles_empty_patterns_without_unwrap();
    }
}
