use crate::core::rules::ErasedRule;

pub mod rf01;
pub mod rf02;
pub mod rf03;
pub mod rf04;
pub mod rf05;
pub mod rf06;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::Erased as _;

    vec![
        rf01::RuleRF01.erased(),
        rf02::RuleRF02.erased(),
        rf03::RuleRF03.erased(),
        rf04::RuleRF04.erased(),
        rf05::RuleRF05.erased(),
        rf06::RuleRF06.erased(),
    ]
}
