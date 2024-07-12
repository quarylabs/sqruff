use crate::core::rules::base::ErasedRule;

pub mod ST01;
pub mod ST02;
pub mod ST03;
pub mod ST04;
mod ST05;
pub mod ST06;
pub mod ST07;
pub mod ST08;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        ST01::RuleST01.erased(),
        ST02::RuleST02.erased(),
        ST03::RuleST03.erased(),
        ST04::RuleST04.erased(),
        ST05::RuleST05::default().erased(),
        ST06::RuleST06.erased(),
        ST07::RuleST07.erased(),
        ST08::RuleST08.erased(),
    ]
}
