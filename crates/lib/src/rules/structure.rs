use crate::core::rules::base::ErasedRule;

pub mod ST01;
pub mod ST02;
pub mod ST03;
pub mod ST04;
pub mod ST08;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        ST01::RuleST01.erased(),
        ST02::RuleST02.erased(),
        ST03::RuleST03.erased(),
        ST04::RuleST04.erased(),
        ST08::RuleST08.erased(),
    ]
}
