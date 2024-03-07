pub mod LT01;
pub mod LT02;
pub mod LT03;
pub mod LT04;
pub mod LT05;
pub mod LT06;
pub mod LT07;
pub mod LT08;
pub mod LT09;
pub mod LT10;
pub mod LT11;
pub mod LT12;
pub mod LT13;

pub fn get_rules(
    config: &crate::core::config::FluffConfig,
) -> Vec<crate::core::rules::base::ErasedRule> {
    use crate::core::rules::base::{Erased as _, Rule as _};

    vec![
        LT01::RuleLT01::from_config(config).erased(),
        LT02::RuleLT02::from_config(config).erased(),
        LT03::RuleLT03::from_config(config).erased(),
        LT04::RuleLT04::from_config(config).erased(),
        LT05::RuleLT05::from_config(config).erased(),
        LT06::RuleLT06::from_config(config).erased(),
        LT07::RuleLT07::from_config(config).erased(),
        LT08::RuleLT08::from_config(config).erased(),
        LT09::RuleLT09::from_config(config).erased(),
        LT10::RuleLT10::from_config(config).erased(),
        LT11::RuleLT11::from_config(config).erased(),
        LT12::RuleLT12::from_config(config).erased(),
    ]
}
