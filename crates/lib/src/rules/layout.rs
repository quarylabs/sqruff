use crate::core::rules::base::ErasedRule;

pub mod lt01;
pub mod lt02;
pub mod lt03;
pub mod lt04;
pub mod lt05;
pub mod lt06;
pub mod lt07;
pub mod lt08;
pub mod lt09;
pub mod lt10;
pub mod lt11;
pub mod lt12;
pub mod lt13;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        lt01::RuleLT01.erased(),
        lt02::RuleLT02.erased(),
        lt03::RuleLT03.erased(),
        lt04::RuleLT04::default().erased(),
        lt05::RuleLT05::default().erased(),
        lt06::RuleLT06.erased(),
        lt07::RuleLT07.erased(),
        lt08::RuleLT08.erased(),
        lt09::RuleLT09::default().erased(),
        lt10::RuleLT10.erased(),
        lt11::RuleLT11.erased(),
        lt12::RuleLT12.erased(),
        lt13::RuleLT13.erased(),
    ]
}
