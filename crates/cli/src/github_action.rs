use std::env;

pub(crate) fn is_in_github_action() -> bool {
    env::var("GITHUB_ACTIONS").is_ok()
}
