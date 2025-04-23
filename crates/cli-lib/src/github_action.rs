use std::env;

pub(crate) fn is_in_github_action() -> bool {
    env::var("GITHUB_ACTIONS")
        .map(|s| s == "true")
        .unwrap_or(false)
}
