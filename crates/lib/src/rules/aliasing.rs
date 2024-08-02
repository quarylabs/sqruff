use crate::core::rules::base::ErasedRule;

pub mod al01;
pub mod al02;
pub mod al03;
pub mod al04;
pub mod al05;
pub mod al06;
pub mod al07;
pub mod al08;
pub mod al09;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        al01::RuleAL01::default().erased(),
        al02::RuleAL02::default().erased(),
        al03::RuleAL03.erased(),
        al04::RuleAL04::default().erased(),
        al05::RuleAL05.erased(),
        al06::RuleAL06::default().erased(),
        al07::RuleAL07::default().erased(),
        al08::RuleAL08.erased(),
        al09::RuleAL09.erased(),
    ]
}
