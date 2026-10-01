use crate::core::rules::ErasedRule;

pub mod pg01;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::Erased as _;

    vec![pg01::RulePG01.erased()]
}
