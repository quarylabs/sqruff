use crate::core::rules::base::ErasedRule;

pub mod LT01;
pub mod LT02;
pub mod LT03;
pub mod LT04;
pub mod LT05;
pub mod LT06;
pub mod LT07;
pub mod LT08;
pub mod LT09;
pub mod LT10;
pub mod LT11;
pub mod LT12;
pub mod LT13;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        LT01::RuleLT01::default().erased(),
        LT02::RuleLT02::default().erased(),
        LT03::RuleLT03::default().erased(),
        LT04::RuleLT04::default().erased(),
        LT05::RuleLT05::default().erased(),
        LT06::RuleLT06::default().erased(),
        LT07::RuleLT07::default().erased(),
        LT08::RuleLT08::default().erased(),
        LT09::RuleLT09::default().erased(),
        LT10::RuleLT10::default().erased(),
        LT11::RuleLT11.erased(),
        LT12::RuleLT12::default().erased(),
    ]
}
