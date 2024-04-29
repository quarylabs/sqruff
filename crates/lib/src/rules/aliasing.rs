use crate::core::rules::base::ErasedRule;

pub mod AL01;
pub mod AL02;
pub mod AL03;
pub mod AL04;
pub mod AL05;
pub mod AL06;
pub mod AL07;
pub mod AL08;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        AL01::RuleAL01::default().erased(),
        AL02::RuleAL02::default().erased(),
        AL03::RuleAL03::default().erased(),
        AL04::RuleAL04::default().erased(),
        AL05::RuleAL05::default().erased(),
        AL06::RuleAL06::default().erased(),
        AL07::RuleAL07::default().erased(),
        AL08::RuleAL08::default().erased(),
    ]
}
