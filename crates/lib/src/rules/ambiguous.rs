use crate::core::rules::base::ErasedRule;

pub mod am01;
pub mod am02;
pub mod am03;
pub mod am04;
pub mod am05;
pub mod am06;
pub mod am07;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        am01::RuleAM01.erased(),
        am02::RuleAM02.erased(),
        am03::RuleAM03.erased(),
        am04::RuleAM04.erased(),
        am05::RuleAM05::default().erased(),
        am06::RuleAM06::default().erased(),
        am07::RuleAM07.erased(),
    ]
}
