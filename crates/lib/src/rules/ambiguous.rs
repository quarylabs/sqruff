use crate::core::rules::base::ErasedRule;

pub mod AM01;
pub mod AM02;
pub mod AM03;
pub mod AM05;
pub mod AM06;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        AM01::RuleAM01.erased(),
        AM02::RuleAM02.erased(),
        AM03::RuleAM03.erased(),
        AM05::RuleAM05::default().erased(),
        AM06::RuleAM06::default().erased(),
    ]
}
