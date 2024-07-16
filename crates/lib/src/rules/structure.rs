use crate::core::rules::base::ErasedRule;

pub mod ST01;
pub mod ST02;
pub mod ST03;
pub mod ST04;
pub mod ST06;
pub mod ST07;
pub mod ST08;
mod ST09;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        ST01::RuleST01.erased(),
        ST02::RuleST02.erased(),
        ST03::RuleST03.erased(),
        ST04::RuleST04.erased(),
        ST06::RuleST06.erased(),
        ST07::RuleST07.erased(),
        ST08::RuleST08.erased(),
        ST09::RuleST09::default().erased(),
    ]
}
