use crate::ir::article::sidebar::Pos;

pub mod error;
pub mod fs;
pub mod html;
pub mod path;
pub mod str;

pub fn pos_slug(pos: &[usize], slug: &str) -> String {
    let slug = slug
        .strip_prefix('/')
        .expect("internal invariant: article slugs always start with '/'");
    if pos.is_empty() {
        return slug.to_string();
    }
    let pos = pos
        .iter()
        .map(|u| (u + 1).to_string())
        .collect::<Vec<_>>()
        .join(".");
    // no "/"
    format!("{}-{}", slug, pos)
}

pub fn pos_base_on(base: Option<&Pos>, pos: Option<&Pos>) -> Pos {
    match base {
        Some(base) => {
            let mut result = base.clone();
            if let Some(pos) = pos {
                result.extend(pos);
            }
            result
        }
        None => pos.cloned().unwrap_or_default(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn run_pos_slug_rejects_missing_leading_slash() {
        let panic = std::panic::catch_unwind(|| pos_slug(&[], "missing-slash"))
            .expect_err("missing leading slash should violate the invariant");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&'static str>().copied())
            .expect("panic payload should be a string message");

        assert_eq!(
            message,
            "internal invariant: article slugs always start with '/'"
        );
    }

    #[test]
    fn pos_slug_rejects_missing_leading_slash() {
        run_pos_slug_rejects_missing_leading_slash();
    }
}
