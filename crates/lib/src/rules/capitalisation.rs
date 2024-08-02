use crate::core::rules::base::ErasedRule;

pub mod cp01;
pub mod cp02;
pub mod cp03;
pub mod cp04;
pub mod cp05;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        cp01::RuleCP01::default().erased(),
        cp02::RuleCP02::default().erased(),
        cp03::RuleCP03::default().erased(),
        cp04::RuleCP04::default().erased(),
        cp05::RuleCP05::default().erased(),
    ]
}
