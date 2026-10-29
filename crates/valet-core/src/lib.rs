pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

pub mod collision;
pub mod config;
pub mod entitlements;
pub mod executor;
pub mod index;
pub mod journal;
pub mod license;
pub mod matcher;
pub mod model;
pub mod planner;
pub mod platform;
pub mod presets;
pub mod rule_transfer;
pub mod rules;
pub mod storage;
pub mod typegroups;
