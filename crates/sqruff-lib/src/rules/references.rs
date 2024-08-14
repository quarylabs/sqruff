use crate::core::rules::base::ErasedRule;

pub mod rf01;
pub mod rf02;
pub mod rf03;
pub mod rf04;
pub mod rf05;
pub mod rf06;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        rf01::RuleRF01::default().erased(),
        rf02::RuleRF02::default().erased(),
        rf03::RuleRF03::default().erased(),
        rf04::RuleRF04::default().erased(),
        rf05::RuleRF05::default().erased(),
        rf06::RuleRF06::default().erased(),
    ]
}
