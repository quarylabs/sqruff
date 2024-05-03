use crate::core::rules::base::ErasedRule;

pub mod ST01;
pub mod ST02;
pub mod ST03;
pub mod ST08;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        ST01::RuleST01::default().erased(),
        ST02::RuleST02::default().erased(),
        ST03::RuleST03::default().erased(),
        ST08::RuleST08.erased(),
    ]
}
