use crate::core::rules::base::ErasedRule;

pub mod st01;
pub mod st02;
pub mod st03;
pub mod st04;
mod st05;
pub mod st06;
pub mod st07;
pub mod st08;
pub mod st09;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        st01::RuleST01.erased(),
        st02::RuleST02.erased(),
        st03::RuleST03.erased(),
        st04::RuleST04.erased(),
        st05::RuleST05::default().erased(),
        st06::RuleST06.erased(),
        st07::RuleST07.erased(),
        st08::RuleST08.erased(),
        st09::RuleST09::default().erased(),
    ]
}
