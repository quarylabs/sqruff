use crate::core::rules::base::ErasedRule;

pub mod CV01;
pub mod CV02;
pub mod CV03;
pub mod CV04;
pub mod CV05;
pub mod CV07;
pub mod CV08;
pub mod CV09;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        CV01::RuleCV01::default().erased(),
        CV02::RuleCV02.erased(),
        CV03::RuleCV03::default().erased(),
        CV04::RuleCV04::default().erased(),
        CV07::RuleCV07.erased(),
        CV08::RuleCV08.erased(),
        CV09::RuleCV09::default().erased(),
    ]
}
