use crate::core::rules::base::ErasedRule;

pub mod RF01;
pub mod RF02;
pub mod RF03;
pub mod RF04;
pub mod RF05;
pub mod RF06;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        RF01::RuleRF01.erased(),
        RF02::RuleRF02::default().erased(),
        RF03::RuleRF03::default().erased(),
        RF04::RuleRF04::default().erased(),
        RF05::RuleRF05::default().erased(),
        RF06::RuleRF06::default().erased(),
    ]
}
