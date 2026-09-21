use crate::core::rules::ErasedRule;

pub mod tq02;
pub mod tq03;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::Erased as _;

    vec![tq02::RuleTQ02.erased(), tq03::RuleTQ03.erased()]
}
