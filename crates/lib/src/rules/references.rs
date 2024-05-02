use crate::core::rules::base::ErasedRule;

pub mod RF01;
pub mod RF03;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![RF01::RuleRF01.erased(), RF03::RuleRF03::default().erased()]
}
