use crate::core::rules::base::ErasedRule;

pub mod cv01;
pub mod cv02;
pub mod cv03;
pub mod cv04;
pub mod cv05;
pub mod cv06;
pub mod cv07;
pub mod cv08;
pub mod cv09;
pub mod cv10;
pub mod cv11;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        cv01::RuleCV01::default().erased(),
        cv02::RuleCV02.erased(),
        cv03::RuleCV03::default().erased(),
        cv04::RuleCV04::default().erased(),
        cv05::RuleCV05.erased(),
        cv06::RuleCV06::default().erased(),
        cv07::RuleCV07.erased(),
        cv08::RuleCV08.erased(),
        cv09::RuleCV09::default().erased(),
        cv10::RuleCV10::default().erased(),
        cv11::RuleCV11::default().erased(),
    ]
}
