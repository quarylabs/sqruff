use crate::core::rules::ErasedRule;

pub mod tq02;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::Erased as _;

    vec![tq02::RuleTQ02.erased()]
}
