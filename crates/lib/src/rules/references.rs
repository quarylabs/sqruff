use crate::core::rules::base::ErasedRule;

pub mod RF01;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![RF01::RuleRF01.erased()]
}
