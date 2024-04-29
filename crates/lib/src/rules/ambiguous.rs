use crate::core::rules::base::ErasedRule;

mod AM01;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![AM01::RuleAM01.erased()]
}
