use crate::core::rules::base::ErasedRule;

pub mod CP01;
pub mod CP02;
pub mod CP03;
pub mod CP04;
pub mod CP05;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::base::Erased as _;

    vec![
        CP01::RuleCP01::default().erased(),
        CP02::RuleCP02::default().erased(),
        CP03::RuleCP03::default().erased(),
        CP04::RuleCP04::default().erased(),
        CP05::RuleCP05::default().erased(),
    ]
}
