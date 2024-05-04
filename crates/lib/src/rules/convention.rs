use crate::core::rules::base::ErasedRule;

pub mod CV02;
pub mod CV03;
pub mod CV04;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        CV02::RuleCV02::default().erased(),
        CV03::RuleCV03::default().erased(),
        CV04::RuleCV04::default().erased(),
    ]
}
