#![cfg(test)]

use frame_support::assert_ok;

use crate::mock::*;

#[test]
fn update_fee_percentage() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        // Validate default storage
        assert_eq!(
            FeeManager::fee_percentage(0),
            sp_arithmetic::Percent::from_percent(30)
        );
        assert_eq!(FeeManager::fee_version(), 0);

        // Update fee
        assert_ok!(FeeManager::update_fee_percentage(
            RuntimeOrigin::root(),
            sp_arithmetic::Percent::from_percent(50)
        ));

        // Validate updated fee
        assert_eq!(FeeManager::fee_version(), 1);
        assert_eq!(
            FeeManager::fee_percentage(FeeManager::fee_version()),
            sp_arithmetic::Percent::from_percent(50)
        );
    });
}
