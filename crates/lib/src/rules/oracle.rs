use crate::core::rules::ErasedRule;

pub mod or01;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::Erased as _;

    vec![or01::RuleOR01.erased()]
}
