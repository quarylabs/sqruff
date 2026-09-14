use anstyle::Style;
use std::borrow::Cow;
use std::io::IsTerminal;

pub(crate) fn should_produce_plain_output(nocolor: Option<bool>) -> bool {
    plain_output_policy(
        nocolor,
        std::io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()),
    )
}

fn plain_output_policy(nocolor: Option<bool>, is_terminal: bool, env_nocolor: bool) -> bool {
    nocolor == Some(true) || !is_terminal || (env_nocolor && nocolor != Some(false))
}

pub(crate) fn colorize_helper(nocolor: bool, s: &str, style: Style) -> Cow<'_, str> {
    if nocolor {
        s.into()
    } else {
        format!("{style}{s}{style:#}").into()
    }
}

pub(crate) fn split_string_on_spaces(s: &str, line_length: usize) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut line_start = 0;
    let mut last_space = 0;

    for (idx, char) in s.char_indices() {
        if char.is_whitespace() {
            last_space = idx;
        }

        if idx - line_start >= line_length {
            if last_space == line_start {
                lines.push(&s[line_start..idx]);
                line_start = idx + 1;
            } else {
                lines.push(&s[line_start..last_space]);
                line_start = last_space + 1;
            }
            last_space = line_start;
        }
    }

    if line_start < s.len() {
        lines.push(&s[line_start..]);
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::plain_output_policy;

    #[test]
    fn no_color_environment_and_explicit_options() {
        // The upstream cases plus explicit nocolor and redirected output.
        for (nocolor, env, has_color) in [
            (None, None, true),
            (Some(true), None, false),
            (Some(false), None, true),
            (None, Some("1"), false),
            (None, Some("true"), false),
            (None, Some("True"), false),
            (None, Some("False"), false),
            (None, Some("anything"), false),
            (None, Some(""), true),
            (Some(false), Some("1"), true),
            (Some(true), Some(""), false),
        ] {
            let env_nocolor = env.is_some_and(|value| !value.is_empty());
            assert_eq!(
                !plain_output_policy(nocolor, true, env_nocolor),
                has_color,
                "nocolor={nocolor:?}, NO_COLOR={env:?}"
            );
            assert!(plain_output_policy(nocolor, false, env_nocolor));
        }
    }
}
