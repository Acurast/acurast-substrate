#![cfg(test)]

use frame_support::{assert_err, assert_ok};
use sp_arithmetic::Perbill;

use crate::{mock::*, stub::*, types::*, Error, Event};

#[test]
fn test_single_vest_no_rewards() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(9);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(10);
        assert_ok!(AcurastVesting::vest(
            RuntimeOrigin::signed(alice_account_id()).into(),
            Vesting {
                stake: 10u128 * UNIT,
                locking_period: 100u64,
            }
        ));

        System::set_block_number(26);
        assert_ok!(AcurastVesting::cooldown(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));

        System::set_block_number(125);
        assert_err!(
            AcurastVesting::divest(RuntimeOrigin::signed(alice_account_id()).into()),
            Error::<Test>::CannotDivestBeforeCooldownEnds
        );

        System::set_block_number(126);
        assert_ok!(AcurastVesting::divest(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));

        assert_eq!(
            events(),
            [
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::LockStake(
                    alice_account_id(),
                    10 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Vested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 10_000_000,
                        stake: 10 * UNIT,
                        accrued: 0,
                        s: 0,
                        cooldown_started: None,
                    },
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PowerDecreased(
                    alice_account_id(),
                    Perbill::from_percent(50)
                )),
                RuntimeEvent::AcurastVesting(Event::CooldownStarted(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 5_000_000,
                        stake: 10 * UNIT,
                        accrued: 0,
                        s: 0,
                        cooldown_started: Some(26),
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayAccrued(alice_account_id(), 0,)),
                RuntimeEvent::MockPallet(mock_pallet::Event::UnlockStake(
                    alice_account_id(),
                    10 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Divested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 5_000_000,
                        stake: 10 * UNIT,
                        accrued: 0,
                        s: 0,
                        cooldown_started: Some(26),
                    }
                )),
            ]
        );
    });
}

#[test]
fn test_single_vest_rewards() {
    ExtBuilder::default().build().execute_with(|| {
        // will miss this reward
        System::set_block_number(9);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(10);
        assert_ok!(AcurastVesting::vest(
            RuntimeOrigin::signed(alice_account_id()).into(),
            Vesting {
                stake: 10u128 * UNIT,
                locking_period: 100u64,
            }
        ));

        assert_eq!(
            AcurastVesting::pool(),
            PoolState {
                total_power: 10_000_000,
                total_stake: 10u128 * UNIT,
                s: (0, 0),
            },
        );

        // catches this reward
        System::set_block_number(12);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));
        assert_eq!(
            AcurastVesting::pool(),
            PoolState {
                total_power: 10_000_000,
                total_stake: 10u128 * UNIT,
                s: (4400000, 5399999),
            },
        );

        System::set_block_number(26);
        assert_ok!(AcurastVesting::cooldown(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));

        // catches this reward with halfed weight
        System::set_block_number(27);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(126);
        assert_ok!(AcurastVesting::divest(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));

        assert_eq!(
            events(),
            [
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::LockStake(
                    alice_account_id(),
                    10 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Vested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 10_000_000,
                        stake: 10 * UNIT,
                        accrued: 0,
                        s: 0,
                        cooldown_started: None,
                    },
                )),
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PowerDecreased(
                    alice_account_id(),
                    Perbill::from_percent(50)
                )),
                RuntimeEvent::AcurastVesting(Event::CooldownStarted(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 5_000_000,
                        stake: 10 * UNIT,
                        accrued: 44_000_000,
                        s: 5399999,
                        cooldown_started: Some(26),
                    }
                )),
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayAccrued(
                    alice_account_id(),
                    61000005,
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::UnlockStake(
                    alice_account_id(),
                    10 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Divested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 5_000_000,
                        stake: 10 * UNIT,
                        accrued: 61_000_005,
                        s: 10799998,
                        cooldown_started: Some(26),
                    }
                )),
            ]
        );
    });
}

#[test]
fn test_single_revest_in_cooldown() {
    ExtBuilder::default().build().execute_with(|| {
        // will miss this reward
        System::set_block_number(9);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(10);
        assert_ok!(AcurastVesting::vest(
            RuntimeOrigin::signed(alice_account_id()).into(),
            Vesting {
                stake: 10u128 * UNIT,
                locking_period: 100u64,
            }
        ));

        assert_eq!(
            AcurastVesting::pool(),
            PoolState {
                total_power: 10_000_000,
                total_stake: 10u128 * UNIT,
                s: (0, 0),
            },
        );

        // catches this reward
        System::set_block_number(12);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));
        assert_eq!(
            AcurastVesting::pool(),
            PoolState {
                total_power: 10_000_000,
                total_stake: 10u128 * UNIT,
                s: (4400000, 5399999),
            },
        );

        System::set_block_number(26);
        assert_ok!(AcurastVesting::cooldown(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));

        // catches this reward with halfed weight
        System::set_block_number(27);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(28);
        assert_ok!(AcurastVesting::revest(
            RuntimeOrigin::signed(alice_account_id()).into(),
            // revest the same stake and for same locking_period
            Vesting {
                stake: 10u128 * UNIT,
                locking_period: 100u64,
            }
        ));

        // catches this reward with full weight again
        System::set_block_number(29);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(30);
        assert_ok!(AcurastVesting::cooldown(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));

        System::set_block_number(130);
        assert_ok!(AcurastVesting::divest(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));

        assert_eq!(
            events(),
            [
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::LockStake(
                    alice_account_id(),
                    10 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Vested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 10_000_000,
                        stake: 10 * UNIT,
                        accrued: 0,
                        s: 0,
                        cooldown_started: None,
                    },
                )),
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PowerDecreased(
                    alice_account_id(),
                    Perbill::from_percent(50)
                )),
                RuntimeEvent::AcurastVesting(Event::CooldownStarted(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 5_000_000,
                        stake: 10 * UNIT,
                        accrued: 44_000_000,
                        s: 5399999,
                        cooldown_started: Some(26),
                    }
                )),
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PowerIncreased(
                    alice_account_id(),
                    Perbill::from_percent(50)
                )),
                RuntimeEvent::AcurastVesting(Event::Revested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 10_000_000,
                        stake: 10 * UNIT,
                        accrued: 61000005,
                        s: 10799998,
                        cooldown_started: None,
                    },
                    true
                )),
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PowerDecreased(
                    alice_account_id(),
                    Perbill::from_percent(50)
                )),
                RuntimeEvent::AcurastVesting(Event::CooldownStarted(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 5_000_000,
                        stake: 10 * UNIT,
                        accrued: 85000025,
                        s: 16199997,
                        cooldown_started: Some(30),
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayAccrued(
                    alice_account_id(),
                    85000025,
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::UnlockStake(
                    alice_account_id(),
                    10 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Divested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 5_000_000,
                        stake: 10 * UNIT,
                        accrued: 85000025,
                        s: 16199997,
                        cooldown_started: Some(30),
                    }
                )),
            ]
        );
    });
}

#[test]
fn test_single_revest_before_cooldown() {
    ExtBuilder::default().build().execute_with(|| {
        // will miss this reward
        System::set_block_number(9);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(10);
        assert_ok!(AcurastVesting::vest(
            RuntimeOrigin::signed(alice_account_id()).into(),
            Vesting {
                stake: 10u128 * UNIT,
                locking_period: 100u64,
            }
        ));

        assert_eq!(
            AcurastVesting::pool(),
            PoolState {
                total_power: 10_000_000,
                total_stake: 10u128 * UNIT,
                s: (0, 0),
            },
        );

        // catches this reward
        System::set_block_number(12);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));
        assert_eq!(
            AcurastVesting::pool(),
            PoolState {
                total_power: 10_000_000,
                total_stake: 10u128 * UNIT,
                s: (4400000, 5399999),
            },
        );

        System::set_block_number(15);
        assert_ok!(AcurastVesting::revest(
            RuntimeOrigin::signed(alice_account_id()).into(),
            // revest the same stake and for same locking_period
            Vesting {
                stake: 20u128 * UNIT,
                locking_period: 100u64,
            }
        ));

        System::set_block_number(26);
        assert_ok!(AcurastVesting::cooldown(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));

        // catches this reward with halfed weight
        System::set_block_number(27);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(126);
        assert_ok!(AcurastVesting::divest(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));

        assert_eq!(
            events(),
            [
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::LockStake(
                    alice_account_id(),
                    10 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Vested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 10_000_000,
                        stake: 10 * UNIT,
                        accrued: 0,
                        s: 0,
                        cooldown_started: None,
                    },
                )),
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PowerIncreased(
                    alice_account_id(),
                    Perbill::from_percent(50)
                )),
                RuntimeEvent::AcurastVesting(Event::Revested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 20000000,
                        stake: 20 * UNIT,
                        accrued: 44000000,
                        s: 5399999,
                        cooldown_started: None,
                    },
                    false
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PowerDecreased(
                    alice_account_id(),
                    Perbill::from_percent(50)
                )),
                RuntimeEvent::AcurastVesting(Event::CooldownStarted(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 10000000,
                        stake: 20 * UNIT,
                        accrued: 44_000_000,
                        s: 5399999,
                        cooldown_started: Some(26),
                    }
                )),
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayAccrued(
                    alice_account_id(),
                    78000010,
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::UnlockStake(
                    alice_account_id(),
                    20 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Divested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 10000000,
                        stake: 20 * UNIT,
                        accrued: 78000010,
                        s: 10799998,
                        cooldown_started: Some(26),
                    }
                )),
            ]
        );
    });
}

#[test]
fn test_multiple_vest_rewards() {
    ExtBuilder::default().build().execute_with(|| {
        // will miss this reward
        System::set_block_number(9);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(10);
        assert_ok!(AcurastVesting::vest(
            RuntimeOrigin::signed(alice_account_id()).into(),
            Vesting {
                stake: 10u128 * UNIT,
                locking_period: 100u64,
            }
        ));

        System::set_block_number(11);
        assert_ok!(AcurastVesting::vest(
            RuntimeOrigin::signed(charlie_account_id()).into(),
            Vesting {
                stake: 20u128 * UNIT,
                locking_period: 50u64,
            }
        ));

        assert_eq!(
            AcurastVesting::pool(),
            PoolState {
                total_power: 20_000_000,
                total_stake: 30u128 * UNIT,
                s: (0, 0),
            },
        );

        // both vesters catch this reward
        System::set_block_number(12);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));
        assert_eq!(
            AcurastVesting::pool(),
            PoolState {
                total_power: 20_000_000,
                total_stake: 30u128 * UNIT,
                s: (2200000, 3199999),
            },
        );

        System::set_block_number(26);
        assert_ok!(AcurastVesting::cooldown(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));
        assert_ok!(AcurastVesting::cooldown(
            RuntimeOrigin::signed(charlie_account_id()).into(),
        ));

        System::set_block_number(76);
        assert_ok!(AcurastVesting::divest(
            RuntimeOrigin::signed(charlie_account_id()).into(),
        ));
        System::set_block_number(126);
        assert_ok!(AcurastVesting::divest(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));

        assert_eq!(
            events(),
            [
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::LockStake(
                    alice_account_id(),
                    10 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Vested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 10_000_000,
                        stake: 10 * UNIT,
                        accrued: 0,
                        s: 0,
                        cooldown_started: None,
                    },
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::LockStake(
                    charlie_account_id(),
                    20 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Vested(
                    charlie_account_id(),
                    VesterState {
                        locking_period: 50,
                        power: 10_000_000,
                        stake: 20 * UNIT,
                        accrued: 0,
                        s: 0,
                        cooldown_started: None,
                    },
                )),
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PowerDecreased(
                    alice_account_id(),
                    Perbill::from_percent(50)
                )),
                RuntimeEvent::AcurastVesting(Event::CooldownStarted(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 5_000_000,
                        stake: 10 * UNIT,
                        accrued: 22_000_000,
                        s: 3199999,
                        cooldown_started: Some(26),
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PowerDecreased(
                    charlie_account_id(),
                    Perbill::from_percent(50)
                )),
                RuntimeEvent::AcurastVesting(Event::CooldownStarted(
                    charlie_account_id(),
                    VesterState {
                        locking_period: 50,
                        power: 5_000_000,
                        stake: 20 * UNIT,
                        accrued: 22_000_000,
                        s: 3199999,
                        cooldown_started: Some(26),
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayAccrued(
                    charlie_account_id(),
                    22000000,
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::UnlockStake(
                    charlie_account_id(),
                    20 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Divested(
                    charlie_account_id(),
                    VesterState {
                        locking_period: 50,
                        power: 5_000_000,
                        stake: 20 * UNIT,
                        accrued: 22_000_000,
                        s: 3199999,
                        cooldown_started: Some(26),
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayAccrued(
                    alice_account_id(),
                    22000000,
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::UnlockStake(
                    alice_account_id(),
                    10 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Divested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 5_000_000,
                        stake: 10 * UNIT,
                        accrued: 22_000_000,
                        s: 3199999,
                        cooldown_started: Some(26),
                    }
                )),
            ]
        );
    });
}

#[test]
fn test_cannot_revest_less() {
    ExtBuilder::default().build().execute_with(|| {
        // will miss this reward
        System::set_block_number(9);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(10);
        assert_ok!(AcurastVesting::vest(
            RuntimeOrigin::signed(alice_account_id()).into(),
            Vesting {
                stake: 10u128 * UNIT,
                locking_period: 100u64,
            }
        ));

        System::set_block_number(15);
        assert_err!(
            AcurastVesting::revest(
                RuntimeOrigin::signed(alice_account_id()).into(),
                // revest the same stake and for same locking_period
                Vesting {
                    stake: 9 * UNIT,
                    locking_period: 100u64,
                }
            ),
            Error::<Test>::CannotRevestLess
        );
    });
}

#[test]
fn test_cannot_revest_with_shorter_locking_period() {
    ExtBuilder::default().build().execute_with(|| {
        // will miss this reward
        System::set_block_number(9);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(10);
        assert_ok!(AcurastVesting::vest(
            RuntimeOrigin::signed(alice_account_id()).into(),
            Vesting {
                stake: 10u128 * UNIT,
                locking_period: 100u64,
            }
        ));

        System::set_block_number(15);
        assert_err!(
            AcurastVesting::revest(
                RuntimeOrigin::signed(alice_account_id()).into(),
                // revest the same stake and for same locking_period
                Vesting {
                    stake: 15 * UNIT,
                    locking_period: 99u64,
                }
            ),
            Error::<Test>::CannotRevestWithShorterLockingPeriod
        );
    });
}

#[test]
fn test_cannot_revest_with_shorter_locking_period_() {
    ExtBuilder::default().build().execute_with(|| {
        // will miss this reward
        System::set_block_number(9);
        assert_ok!(AcurastVesting::distribute_reward(44 * UNIT));

        System::set_block_number(10);
        assert_ok!(AcurastVesting::vest(
            RuntimeOrigin::signed(alice_account_id()).into(),
            Vesting {
                stake: 10u128 * UNIT,
                locking_period: 100u64,
            }
        ));

        System::set_block_number(26);
        assert_ok!(AcurastVesting::cooldown(
            RuntimeOrigin::signed(alice_account_id()).into(),
        ));

        System::set_block_number(125);
        assert_err!(
            AcurastVesting::divest(RuntimeOrigin::signed(alice_account_id()).into()),
            Error::<Test>::CannotDivestBeforeCooldownEnds
        );

        assert_err!(
            AcurastVesting::kick_out(
                RuntimeOrigin::signed(bob_account_id()).into(),
                alice_account_id().into()
            ),
            Error::<Test>::CannotKickoutBeforeCooldownToleranceEnded
        );

        // late divest trial by vester Alice
        System::set_block_number(129);
        assert_err!(
            AcurastVesting::divest(RuntimeOrigin::signed(alice_account_id()).into()),
            Error::<Test>::CannotDivestWhenToleranceEnded
        );

        // Bob kicks Alice out successfully
        System::set_block_number(129);
        assert_ok!(AcurastVesting::kick_out(
            RuntimeOrigin::signed(bob_account_id()).into(),
            alice_account_id().into()
        ));

        assert_eq!(
            events(),
            [
                RuntimeEvent::AcurastVesting(Event::RewardDistributed(44 * UNIT)),
                RuntimeEvent::MockPallet(mock_pallet::Event::LockStake(
                    alice_account_id(),
                    10 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::Vested(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 10_000_000,
                        stake: 10 * UNIT,
                        accrued: 0,
                        s: 0,
                        cooldown_started: None,
                    },
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PowerDecreased(
                    alice_account_id(),
                    Perbill::from_percent(50)
                )),
                RuntimeEvent::AcurastVesting(Event::CooldownStarted(
                    alice_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 5_000_000,
                        stake: 10 * UNIT,
                        accrued: 0,
                        s: 0,
                        cooldown_started: Some(26),
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayKicker(bob_account_id(), 0,)),
                RuntimeEvent::MockPallet(mock_pallet::Event::UnlockStake(
                    alice_account_id(),
                    10 * UNIT,
                )),
                RuntimeEvent::AcurastVesting(Event::KickedOut(
                    alice_account_id(),
                    bob_account_id(),
                    VesterState {
                        locking_period: 100,
                        power: 5_000_000,
                        stake: 10 * UNIT,
                        accrued: 0,
                        s: 0,
                        cooldown_started: Some(26),
                    }
                )),
            ]
        );
    });
}

#[test]
fn test_maximum_locking_period_exceeded() {
    ExtBuilder::default().build().execute_with(|| {
        // pretend current time

        assert_err!(
            AcurastVesting::vest(
                RuntimeOrigin::signed(alice_account_id()).into(),
                Vesting {
                    stake: 10 * UNIT,
                    locking_period: 101,
                }
            ),
            Error::<Test>::MaximumLockingPeriodExceeded
        );

        assert_eq!(events(), []);
    });
}
