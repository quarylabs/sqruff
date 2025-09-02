use crate::core::rules::ErasedRule;

pub mod jj01;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::Erased as _;
    vec![jj01::RuleJJ01.erased()]
}
