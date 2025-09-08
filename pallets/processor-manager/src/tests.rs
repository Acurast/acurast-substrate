use crate::{
	mock::*, stub::*, BalanceFor, BinaryLocation, Error, Event, ProcessorPairingFor,
	ProcessorPairingUpdateFor, RewardDistributionSettings, UpdateInfo,
};
use acurast_common::{AccountLookup, ListUpdateOperation, Version};
use frame_support::{assert_err, assert_ok, error::BadOrigin, traits::fungible::Inspect};
use hex_literal::hex;
use pallet_balances::Event as BalancesEvent;

fn paired_manager_processor() -> (AccountId, AccountId) {
	let (signer, manager_account) = generate_pair_account();
	let (_, processor_account) = generate_pair_account();
	let initial_timestamp = 1657363915010u64;
	if Timestamp::get() != initial_timestamp {
		Timestamp::set_timestamp(initial_timestamp);
	}
	let timestamp = 1657363915002u128;
	let signature = generate_signature(&signer, &manager_account, timestamp, 1);
	let update =
		ProcessorPairingFor::<Test>::new_with_proof(manager_account.clone(), timestamp, signature);
	assert_ok!(AcurastProcessorManager::pair_with_manager(
		RuntimeOrigin::signed(processor_account.clone()),
		update,
	));

	(manager_account, processor_account)
}

pub fn processor_account_id() -> AccountId {
	hex!("b8bc25a2b4c0386b8892b43e435b71fe11fa50533935f027949caf04bcce4694").into()
}

#[test]
fn test_update_processor_pairings_succeed_1() {
	ExtBuilder.build().execute_with(|| {
		let (signer, processor_account) = generate_pair_account();
		let _ = Timestamp::set(RuntimeOrigin::none(), 1657363915010);
		let timestamp = 1657363915002u128;
		let signature = generate_signature(&signer, &alice_account_id(), timestamp, 1);
		let updates = vec![ProcessorPairingUpdateFor::<Test> {
			operation: ListUpdateOperation::Add,
			item: ProcessorPairingFor::<Test>::new_with_proof(
				processor_account.clone(),
				timestamp,
				signature,
			),
		}];
		let call = AcurastProcessorManager::update_processor_pairings(
			RuntimeOrigin::signed(alice_account_id()),
			updates.clone().try_into().unwrap(),
		);
		assert_ok!(call);
		assert_eq!(Some(1), AcurastProcessorManager::last_manager_id());
		assert_eq!(Some(1), AcurastProcessorManager::manager_id_for_processor(&processor_account));
		assert_eq!(Some(alice_account_id()), AcurastProcessorManager::lookup(&processor_account));
		assert!(AcurastProcessorManager::managed_processors(1, &processor_account).is_some());
		let last_events = events();
		assert_eq!(
			last_events[(last_events.len() - 2)..],
			vec![
				RuntimeEvent::AcurastProcessorManager(Event::ManagerCreated(alice_account_id(), 1)),
				RuntimeEvent::AcurastProcessorManager(Event::ProcessorPairingsUpdated(
					alice_account_id(),
					updates.try_into().unwrap()
				)),
			]
		);

		let updates = vec![ProcessorPairingUpdateFor::<Test> {
			operation: ListUpdateOperation::Remove,
			item: ProcessorPairingFor::<Test>::new(processor_account.clone()),
		}];
		let call = AcurastProcessorManager::update_processor_pairings(
			RuntimeOrigin::signed(alice_account_id()),
			updates.clone().try_into().unwrap(),
		);
		assert_ok!(call);
		assert_eq!(None, AcurastProcessorManager::manager_id_for_processor(&processor_account));
		assert_eq!(None, AcurastProcessorManager::lookup(&processor_account));
		assert_eq!(
			events(),
			vec![RuntimeEvent::AcurastProcessorManager(Event::ProcessorPairingsUpdated(
				alice_account_id(),
				updates.try_into().unwrap()
			)),]
		);
	});
}

#[test]
fn test_update_processor_pairings_succeed_2() {
	ExtBuilder.build().execute_with(|| {
		let (signer, processor_account) = generate_pair_account();
		let _ = Timestamp::set(RuntimeOrigin::none(), 1657363915010);
		let timestamp = 1657363915002u128;
		let signature = generate_signature(&signer, &alice_account_id(), timestamp, 1);
		let updates = vec![ProcessorPairingUpdateFor::<Test> {
			operation: ListUpdateOperation::Add,
			item: ProcessorPairingFor::<Test>::new_with_proof(
				processor_account.clone(),
				timestamp,
				signature,
			),
		}];
		let call = AcurastProcessorManager::update_processor_pairings(
			RuntimeOrigin::signed(alice_account_id()),
			updates.clone().try_into().unwrap(),
		);
		assert_ok!(call);
		_ = events();

		let (signer, processor_account) = generate_pair_account();
		let signature = generate_signature(&signer, &bob_account_id(), timestamp, 1);
		let updates = vec![ProcessorPairingUpdateFor::<Test> {
			operation: ListUpdateOperation::Add,
			item: ProcessorPairingFor::<Test>::new_with_proof(
				processor_account.clone(),
				timestamp,
				signature,
			),
		}];
		let call = AcurastProcessorManager::update_processor_pairings(
			RuntimeOrigin::signed(bob_account_id()),
			updates.clone().try_into().unwrap(),
		);
		assert_ok!(call);

		assert_eq!(Some(2), AcurastProcessorManager::last_manager_id());
		assert_eq!(Some(2), AcurastProcessorManager::manager_id_for_processor(&processor_account));
		assert_eq!(Some(bob_account_id()), AcurastProcessorManager::lookup(&processor_account));
		assert!(AcurastProcessorManager::managed_processors(2, &processor_account).is_some());
		let last_events = events();
		assert_eq!(
			last_events[(last_events.len() - 2)..],
			vec![
				RuntimeEvent::AcurastProcessorManager(Event::ManagerCreated(bob_account_id(), 2)),
				RuntimeEvent::AcurastProcessorManager(Event::ProcessorPairingsUpdated(
					bob_account_id(),
					updates.try_into().unwrap()
				)),
			]
		);
	});
}

#[test]
fn test_update_processor_pairings_failure_1() {
	ExtBuilder.build().execute_with(|| {
		let (signer, processor_account) = generate_pair_account();
		let _ = Timestamp::set(RuntimeOrigin::none(), 1657363915010);
		let timestamp = 1657363915002u128;
		let signature = generate_signature(&signer, &alice_account_id(), timestamp, 1);
		let updates = vec![ProcessorPairingUpdateFor::<Test> {
			operation: ListUpdateOperation::Add,
			item: ProcessorPairingFor::<Test>::new_with_proof(
				processor_account.clone(),
				1657363915003u128,
				signature,
			),
		}];
		let call = AcurastProcessorManager::update_processor_pairings(
			RuntimeOrigin::signed(alice_account_id()),
			updates.clone().try_into().unwrap(),
		);
		assert_err!(call, Error::<Test>::InvalidPairingProof);
	});
}

#[test]
fn test_update_processor_pairings_failure_2() {
	ExtBuilder.build().execute_with(|| {
		let (signer, processor_account) = generate_pair_account();
		let _ = Timestamp::set(RuntimeOrigin::none(), 1657363915010);
		let timestamp = 1657363915002u128;
		let signature_1 = generate_signature(&signer, &alice_account_id(), timestamp, 1);
		let signature_2 = generate_signature(&signer, &alice_account_id(), timestamp, 2);
		let updates = vec![
			ProcessorPairingUpdateFor::<Test> {
				operation: ListUpdateOperation::Add,
				item: ProcessorPairingFor::<Test>::new_with_proof(
					processor_account.clone(),
					timestamp,
					signature_1,
				),
			},
			ProcessorPairingUpdateFor::<Test> {
				operation: ListUpdateOperation::Add,
				item: ProcessorPairingFor::<Test>::new_with_proof(
					processor_account.clone(),
					timestamp,
					signature_2,
				),
			},
		];
		let call = AcurastProcessorManager::update_processor_pairings(
			RuntimeOrigin::signed(alice_account_id()),
			updates.clone().try_into().unwrap(),
		);
		assert_err!(call, Error::<Test>::ProcessorAlreadyPaired);
	});
}

#[test]
fn test_update_processor_pairings_failure_3() {
	ExtBuilder.build().execute_with(|| {
		let (signer, processor_account) = generate_pair_account();
		let _ = Timestamp::set(RuntimeOrigin::none(), 1657363915010);
		let timestamp = 1657363915002u128;
		let signature_1 = generate_signature(&signer, &alice_account_id(), timestamp, 1);
		let signature_2 = generate_signature(&signer, &bob_account_id(), timestamp, 1);
		let updates = vec![ProcessorPairingUpdateFor::<Test> {
			operation: ListUpdateOperation::Add,
			item: ProcessorPairingFor::<Test>::new_with_proof(
				processor_account.clone(),
				timestamp,
				signature_1,
			),
		}];
		let call = AcurastProcessorManager::update_processor_pairings(
			RuntimeOrigin::signed(alice_account_id()),
			updates.clone().try_into().unwrap(),
		);
		assert_ok!(call);

		let updates = vec![ProcessorPairingUpdateFor::<Test> {
			operation: ListUpdateOperation::Add,
			item: ProcessorPairingFor::<Test>::new_with_proof(
				processor_account.clone(),
				timestamp,
				signature_2,
			),
		}];
		let call = AcurastProcessorManager::update_processor_pairings(
			RuntimeOrigin::signed(bob_account_id()),
			updates.clone().try_into().unwrap(),
		);
		assert_err!(call, Error::<Test>::ProcessorPairedWithAnotherManager);
	});
}

#[test]
fn test_recover_funds_succeed_1() {
	ExtBuilder.build().execute_with(|| {
		let (manager_account, processor_account) = paired_manager_processor();
		assert_ok!(Balances::transfer_allow_death(
			RuntimeOrigin::signed(alice_account_id()),
			processor_account.clone(),
			10_000_000
		));
		assert_eq!(Balances::balance(&alice_account_id()), 90_000_000);

		assert_ok!(AcurastProcessorManager::recover_funds(
			RuntimeOrigin::signed(manager_account),
			processor_account.clone(),
			alice_account_id(),
		));
		assert_eq!(Balances::balance(&alice_account_id()), 99_999_000); // 1_000 of existensial balance remains on the processor

		assert_eq!(
			events().last().unwrap(),
			&RuntimeEvent::AcurastProcessorManager(Event::ProcessorFundsRecovered(
				processor_account,
				alice_account_id()
			)),
		);
	});
}

#[test]
fn test_recover_funds_succeed_2() {
	ExtBuilder.build().execute_with(|| {
		let (manager_account, processor_account) = paired_manager_processor();

		assert_ok!(AcurastProcessorManager::recover_funds(
			RuntimeOrigin::signed(manager_account),
			processor_account.clone(),
			alice_account_id(),
		));

		assert_eq!(
			events().last().unwrap(),
			&RuntimeEvent::AcurastProcessorManager(Event::ProcessorFundsRecovered(
				processor_account,
				alice_account_id()
			)),
		);
	});
}

#[test]
fn test_recover_funds_failure_1() {
	ExtBuilder.build().execute_with(|| {
		let (manager_account, _) = paired_manager_processor();

		let (_, processor_account) = generate_pair_account();

		let call = AcurastProcessorManager::recover_funds(
			RuntimeOrigin::signed(manager_account),
			processor_account.clone(),
			alice_account_id(),
		);

		assert_err!(call, Error::<Test>::ProcessorHasNoManager);
	});
}

#[test]
fn test_recover_funds_failure_2() {
	ExtBuilder.build().execute_with(|| {
		let (manager_account, _) = paired_manager_processor();
		let (_, processor_account) = paired_manager_processor();

		let call = AcurastProcessorManager::recover_funds(
			RuntimeOrigin::signed(manager_account),
			processor_account.clone(),
			alice_account_id(),
		);

		assert_err!(call, Error::<Test>::ProcessorPairedWithAnotherManager);
	});
}

#[test]
fn test_pair_with_manager() {
	ExtBuilder.build().execute_with(|| {
		let (signer, manager_account) = generate_pair_account();
		let (_, processor_account) = generate_pair_account();
		let _ = Timestamp::set(RuntimeOrigin::none(), 1657363915010);
		let timestamp = 1657363915002u128;
		let signature = generate_signature(&signer, &manager_account, timestamp, 1);
		let update = ProcessorPairingFor::<Test>::new_with_proof(
			manager_account.clone(),
			timestamp,
			signature,
		);
		assert_ok!(AcurastProcessorManager::pair_with_manager(
			RuntimeOrigin::signed(processor_account.clone()),
			update.clone(),
		));

		assert_eq!(Some(1), AcurastProcessorManager::last_manager_id());
		assert_eq!(Some(1), AcurastProcessorManager::manager_id_for_processor(&processor_account));
		assert_eq!(
			Some(manager_account.clone()),
			AcurastProcessorManager::lookup(&processor_account)
		);
		assert!(AcurastProcessorManager::managed_processors(1, &processor_account).is_some());
		let last_events = events();
		assert_eq!(
			last_events[(last_events.len() - 2)..],
			vec![
				RuntimeEvent::AcurastProcessorManager(Event::ManagerCreated(manager_account, 1)),
				RuntimeEvent::AcurastProcessorManager(Event::ProcessorPaired(
					processor_account,
					update
				)),
			]
		);
	});
}

#[test]
fn test_multi_pair_with_manager() {
	ExtBuilder.build().execute_with(|| {
		let (signer, manager_account) = generate_pair_account();
		let (_, processor_account_1) = generate_pair_account();
		let (_, processor_account_2) = generate_pair_account();
		let _ = Timestamp::set(RuntimeOrigin::none(), 1657363915010);
		let timestamp = 1657363915002u128;
		let signature = generate_multi_signature(&signer, &manager_account, timestamp);
		let update = ProcessorPairingFor::<Test>::new_with_proof(
			manager_account.clone(),
			timestamp,
			signature,
		);
		assert_ok!(AcurastProcessorManager::multi_pair_with_manager(
			RuntimeOrigin::signed(processor_account_1.clone()),
			update.clone(),
		));

		assert_ok!(AcurastProcessorManager::multi_pair_with_manager(
			RuntimeOrigin::signed(processor_account_2.clone()),
			update.clone(),
		));

		assert_eq!(Some(1), AcurastProcessorManager::last_manager_id());
		assert_eq!(
			Some(1),
			AcurastProcessorManager::manager_id_for_processor(&processor_account_1)
		);
		assert_eq!(
			Some(1),
			AcurastProcessorManager::manager_id_for_processor(&processor_account_2)
		);
		assert_eq!(
			Some(manager_account.clone()),
			AcurastProcessorManager::lookup(&processor_account_1)
		);
		assert!(AcurastProcessorManager::managed_processors(1, &processor_account_1).is_some());
		let last_events = events();
		assert_eq!(
			last_events[(last_events.len() - 3)..],
			vec![
				RuntimeEvent::AcurastProcessorManager(Event::ManagerCreated(manager_account, 1)),
				RuntimeEvent::AcurastProcessorManager(Event::ProcessorPaired(
					processor_account_1,
					update.clone()
				)),
				RuntimeEvent::AcurastProcessorManager(Event::ProcessorPaired(
					processor_account_2,
					update
				)),
			]
		);
	});
}

#[test]
fn test_onboard() {
	ExtBuilder.build().execute_with(|| {
		let (signer, manager_account) = generate_pair_account();
		let processor_account = processor_account_id();
		let _ = Timestamp::set(RuntimeOrigin::none(), 1657363915010);
		let timestamp = 1657363915002u128;
		let signature = generate_signature(&signer, &manager_account, timestamp, 1);
		let pairing = ProcessorPairingFor::<Test>::new_with_proof(
			manager_account.clone(),
			timestamp,
			signature,
		);

		let attestation_chain = attestation_chain();

		assert_ok!(AcurastProcessorManager::onboard(
			RuntimeOrigin::signed(processor_account.clone()),
			pairing.clone(),
			false,
			attestation_chain
		));

		assert_eq!(Some(1), AcurastProcessorManager::last_manager_id());
		assert_eq!(Some(1), AcurastProcessorManager::manager_id_for_processor(&processor_account));
		assert_eq!(
			Some(manager_account.clone()),
			AcurastProcessorManager::lookup(&processor_account)
		);
		assert!(AcurastProcessorManager::managed_processors(1, &processor_account).is_some());
		let last_events = events();
		assert_eq!(
			last_events[(last_events.len() - 2)..],
			vec![
				RuntimeEvent::AcurastProcessorManager(Event::ManagerCreated(manager_account, 1)),
				RuntimeEvent::AcurastProcessorManager(Event::ProcessorPaired(
					processor_account,
					pairing
				)),
			]
		);
	});
}

#[test]
fn test_advertise_for_success() {
	ExtBuilder.build().execute_with(|| {
		let (manager_account, processor_account) = paired_manager_processor();

		assert_ok!(AcurastProcessorManager::advertise_for(
			RuntimeOrigin::signed(manager_account.clone()),
			processor_account.clone(),
			(),
		));

		let last_events = events();
		assert_eq!(
			last_events.last(),
			Some(RuntimeEvent::AcurastProcessorManager(Event::ProcessorAdvertisement(
				manager_account,
				processor_account,
				()
			)))
			.as_ref()
		);
	});
}

#[test]
fn test_advertise_for_failure() {
	ExtBuilder.build().execute_with(|| {
		let (_, processor_account) = paired_manager_processor();
		let (manager_account, _) = paired_manager_processor();

		assert_err!(
			AcurastProcessorManager::advertise_for(
				RuntimeOrigin::signed(manager_account),
				processor_account.clone(),
				(),
			),
			Error::<Test>::ProcessorPairedWithAnotherManager,
		);
	});
}

#[test]
fn test_heartbeat_success() {
	ExtBuilder.build().execute_with(|| {
		let (_, processor_account) = paired_manager_processor();

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_none());

		assert_ok!(AcurastProcessorManager::heartbeat(RuntimeOrigin::signed(
			processor_account.clone()
		)));

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_some());

		let last_events = events();
		assert_eq!(
			last_events.last(),
			Some(RuntimeEvent::AcurastProcessorManager(Event::ProcessorHeartbeat(
				processor_account
			)))
			.as_ref()
		);
	});
}

#[test]
fn test_heartbeat_failure() {
	ExtBuilder.build().execute_with(|| {
		let (_, processor_account) = generate_pair_account();

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_none());

		assert_err!(
			AcurastProcessorManager::heartbeat(RuntimeOrigin::signed(processor_account.clone())),
			Error::<Test>::ProcessorHasNoManager,
		);

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_none());
	});
}

#[test]
fn test_heartbeat_with_version_success() {
	ExtBuilder.build().execute_with(|| {
		let (_, processor_account) = paired_manager_processor();

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_none());
		assert!(AcurastProcessorManager::processor_version(&processor_account).is_none());

		let version = Version { platform: 0, build_number: 1 };
		assert_ok!(AcurastProcessorManager::heartbeat_with_version(
			RuntimeOrigin::signed(processor_account.clone()),
			version
		));

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_some());
		assert!(AcurastProcessorManager::processor_version(&processor_account).is_some());

		let last_events = events();
		assert_eq!(
			last_events.last(),
			Some(RuntimeEvent::AcurastProcessorManager(Event::ProcessorHeartbeatWithVersion(
				processor_account,
				version
			)))
			.as_ref()
		);
	});
}

#[test]
fn test_heartbeat_with_version_failure() {
	ExtBuilder.build().execute_with(|| {
		let (_, processor_account) = generate_pair_account();

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_none());
		assert!(AcurastProcessorManager::processor_version(&processor_account).is_none());

		let version = Version { platform: 0, build_number: 1 };
		assert_err!(
			AcurastProcessorManager::heartbeat_with_version(
				RuntimeOrigin::signed(processor_account.clone()),
				version
			),
			Error::<Test>::ProcessorHasNoManager,
		);

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_none());
		assert!(AcurastProcessorManager::processor_version(&processor_account).is_none());
	});
}

#[test]
fn test_reward_distribution_success() {
	ExtBuilder.build().execute_with(|| {
		let (_, processor_account) = paired_manager_processor();

		let mut timestamp = 1657363915010u64;
		let mut block_number = 1;
		if Timestamp::get() != timestamp {
			Timestamp::set_timestamp(timestamp);
		}
		System::set_block_number(block_number);

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_none());
		assert!(AcurastProcessorManager::processor_version(&processor_account).is_none());

		let reward_distribution_settings = RewardDistributionSettings::<
			BalanceFor<Test>,
			<Test as frame_system::Config>::AccountId,
		> {
			window_length: 300,
			tollerance: 25,
			min_heartbeats: 3,
			reward_per_distribution: 300_000_000_000,
			distributor_account: alice_account_id(),
		};

		assert_ok!(AcurastProcessorManager::update_reward_distribution_settings(
			RuntimeOrigin::root(),
			Some(reward_distribution_settings.clone())
		));

		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			alice_account_id(),
			u128::MAX
		));

		assert_ok!(AcurastProcessorManager::update_min_processor_version_for_reward(
			RuntimeOrigin::root(),
			Version { platform: 0, build_number: 1 }
		));

		let version = Version { platform: 0, build_number: 1 };
		assert_ok!(AcurastProcessorManager::heartbeat_with_version(
			RuntimeOrigin::signed(processor_account.clone()),
			version
		));

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_some());
		assert!(AcurastProcessorManager::processor_version(&processor_account).is_some());

		timestamp += 900_000;
		block_number += 75;
		Timestamp::set_timestamp(timestamp);
		System::set_block_number(block_number);

		assert_ok!(AcurastProcessorManager::heartbeat_with_version(
			RuntimeOrigin::signed(processor_account.clone()),
			version
		));

		timestamp += 900_000;
		block_number += 75;
		Timestamp::set_timestamp(timestamp);
		System::set_block_number(block_number);

		assert_ok!(AcurastProcessorManager::heartbeat_with_version(
			RuntimeOrigin::signed(processor_account.clone()),
			version
		));

		timestamp += 900_000;
		block_number += 75;
		Timestamp::set_timestamp(timestamp);
		System::set_block_number(block_number);

		assert_ok!(AcurastProcessorManager::heartbeat_with_version(
			RuntimeOrigin::signed(processor_account.clone()),
			version
		));

		timestamp += 900_000;
		block_number += 75;
		Timestamp::set_timestamp(timestamp);
		System::set_block_number(block_number);

		assert_ok!(AcurastProcessorManager::heartbeat_with_version(
			RuntimeOrigin::signed(processor_account.clone()),
			version
		));

		let last_events = events();
		assert_eq!(
			last_events.last(),
			Some(RuntimeEvent::Balances(BalancesEvent::Transfer {
				from: alice_account_id(),
				to: processor_account.clone(),
				amount: reward_distribution_settings.reward_per_distribution
			}))
			.as_ref()
		);
	});
}

#[test]
fn test_reward_distribution_failure() {
	ExtBuilder.build().execute_with(|| {
		let (_, processor_account) = paired_manager_processor();

		let mut timestamp = 1657363915010u64;
		let mut block_number = 1;
		if Timestamp::get() != timestamp {
			Timestamp::set_timestamp(timestamp);
		}
		System::set_block_number(block_number);

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_none());
		assert!(AcurastProcessorManager::processor_version(&processor_account).is_none());

		let reward_distribution_settings = RewardDistributionSettings::<
			BalanceFor<Test>,
			<Test as frame_system::Config>::AccountId,
		> {
			window_length: 300,
			tollerance: 25,
			min_heartbeats: 3,
			reward_per_distribution: 300_000_000_000,
			distributor_account: alice_account_id(),
		};

		assert_ok!(AcurastProcessorManager::update_reward_distribution_settings(
			RuntimeOrigin::root(),
			Some(reward_distribution_settings.clone())
		));

		assert_ok!(AcurastProcessorManager::update_min_processor_version_for_reward(
			RuntimeOrigin::root(),
			Version { platform: 0, build_number: 2 }
		));

		let version = Version { platform: 0, build_number: 1 };
		assert_ok!(AcurastProcessorManager::heartbeat_with_version(
			RuntimeOrigin::signed(processor_account.clone()),
			version
		));

		assert!(AcurastProcessorManager::processor_last_seen(&processor_account).is_some());
		assert!(AcurastProcessorManager::processor_version(&processor_account).is_some());

		timestamp += 900_000;
		block_number += 75;
		Timestamp::set_timestamp(timestamp);
		System::set_block_number(block_number);

		assert_ok!(AcurastProcessorManager::heartbeat_with_version(
			RuntimeOrigin::signed(processor_account.clone()),
			version
		));

		timestamp += 900_000;
		block_number += 75;
		Timestamp::set_timestamp(timestamp);
		System::set_block_number(block_number);

		assert_ok!(AcurastProcessorManager::heartbeat_with_version(
			RuntimeOrigin::signed(processor_account.clone()),
			version
		));

		timestamp += 900_000;
		block_number += 75;
		Timestamp::set_timestamp(timestamp);
		System::set_block_number(block_number);

		assert_ok!(AcurastProcessorManager::heartbeat_with_version(
			RuntimeOrigin::signed(processor_account.clone()),
			version
		));

		timestamp += 900_000;
		block_number += 75;
		Timestamp::set_timestamp(timestamp);
		System::set_block_number(block_number);

		assert_ok!(AcurastProcessorManager::heartbeat_with_version(
			RuntimeOrigin::signed(processor_account.clone()),
			version
		));

		let last_events = events();
		assert_eq!(
			last_events.last(),
			Some(RuntimeEvent::AcurastProcessorManager(Event::ProcessorHeartbeatWithVersion(
				processor_account.clone(),
				version
			)))
			.as_ref()
		);
	});
}

#[test]
fn insert_remove_binary_hash_success() {
	ExtBuilder.build().execute_with(|| {
		let hash = [1u8; 32];
		let version = Version { platform: 0, build_number: 1 };

		assert!(AcurastProcessorManager::known_binary_hash(version).is_none());

		assert_ok!(AcurastProcessorManager::update_binary_hash(
			RuntimeOrigin::root(),
			version,
			Some(hash.into())
		));

		assert!(AcurastProcessorManager::known_binary_hash(version).is_some());

		let last_events = events();
		assert_eq!(
			last_events.last(),
			Some(RuntimeEvent::AcurastProcessorManager(Event::BinaryHashUpdated(
				version,
				Some(hash.into())
			)))
			.as_ref()
		);

		assert_ok!(AcurastProcessorManager::update_binary_hash(
			RuntimeOrigin::root(),
			version,
			None,
		));

		assert!(AcurastProcessorManager::known_binary_hash(version).is_none());

		let last_events = events();
		assert_eq!(
			last_events.last(),
			Some(RuntimeEvent::AcurastProcessorManager(Event::BinaryHashUpdated(version, None)))
				.as_ref()
		);
	});
}

#[test]
fn insert_remove_binary_hash_failure() {
	ExtBuilder.build().execute_with(|| {
		let hash = [1u8; 32];
		let version = Version { platform: 0, build_number: 1 };

		assert!(AcurastProcessorManager::known_binary_hash(version).is_none());

		assert_err!(
			AcurastProcessorManager::update_binary_hash(
				RuntimeOrigin::signed(alice_account_id()),
				version,
				Some(hash.into())
			),
			BadOrigin
		);

		assert!(AcurastProcessorManager::known_binary_hash(version).is_none());

		assert_err!(
			AcurastProcessorManager::update_binary_hash(
				RuntimeOrigin::signed(alice_account_id()),
				version,
				None,
			),
			BadOrigin
		);
	});
}

#[test]
fn set_processor_update_info_success() {
	ExtBuilder.build().execute_with(|| {
        let (manager_account, processor_account) = paired_manager_processor();

        let hash = [1u8; 32];
        let version = Version {
            platform: 0,
            build_number: 1,
        };

        assert_ok!(AcurastProcessorManager::update_binary_hash(RuntimeOrigin::root(), version, Some(hash.into())));

        let binary_location: BinaryLocation = b"https://github.com/Acurast/acurast-processor-update/releases/download/processor-1.3.31/processor-1.3.31-devnet.apk".to_vec().try_into().unwrap();
        let update_info = UpdateInfo {
            version,
            binary_location,
        };

        assert_ok!(AcurastProcessorManager::set_processor_update_info(RuntimeOrigin::signed(manager_account.clone()), update_info.clone(), vec![processor_account.clone()].try_into().unwrap()));

        assert!(AcurastProcessorManager::processor_update_info(&processor_account).is_some());

        let last_events = events();
        assert_eq!(
            last_events.last(),
            Some(RuntimeEvent::AcurastProcessorManager(
                Event::ProcessorUpdateInfoSet(manager_account, update_info)
            )).as_ref()
        );
    });
}

#[test]
fn set_processor_update_info_failure_1() {
	ExtBuilder.build().execute_with(|| {
        let (manager_account, processor_account) = paired_manager_processor();

        let hash = [1u8; 32];
        let version = Version {
            platform: 0,
            build_number: 1,
        };

        assert_ok!(AcurastProcessorManager::update_binary_hash(RuntimeOrigin::root(), version, Some(hash.into())));

        let binary_location: BinaryLocation = b"https://github.com/Acurast/acurast-processor-update/releases/download/processor-1.3.31/processor-1.3.31-devnet.apk".to_vec().try_into().unwrap();
        let version = Version {
            platform: 0,
            build_number: 2,
        };
        let update_info = UpdateInfo {
            version,
            binary_location,
        };

        assert_err!(
            AcurastProcessorManager::set_processor_update_info(RuntimeOrigin::signed(manager_account.clone()), update_info.clone(), vec![processor_account].try_into().unwrap()),
            Error::<Test>::UnknownProcessorVersion,
        );
    });
}

#[test]
fn set_processor_update_info_failure_2() {
	ExtBuilder.build().execute_with(|| {
        let (manager_account, _) = paired_manager_processor();
        let (_, processor_account) = paired_manager_processor();

        let hash = [1u8; 32];
        let version = Version {
            platform: 0,
            build_number: 1,
        };

        assert_ok!(AcurastProcessorManager::update_binary_hash(RuntimeOrigin::root(), version, Some(hash.into())));

        let binary_location: BinaryLocation = b"https://github.com/Acurast/acurast-processor-update/releases/download/processor-1.3.31/processor-1.3.31-devnet.apk".to_vec().try_into().unwrap();
        let update_info = UpdateInfo {
            version,
            binary_location,
        };

        assert_err!(
            AcurastProcessorManager::set_processor_update_info(RuntimeOrigin::signed(manager_account.clone()), update_info.clone(), vec![processor_account].try_into().unwrap()),
            Error::<Test>::ProcessorPairedWithAnotherManager,
        );
    });
}

#[test]
fn set_processor_update_info_failure_3() {
	ExtBuilder.build().execute_with(|| {
        let (_, processor_account) = paired_manager_processor();

        let hash = [1u8; 32];
        let version = Version {
            platform: 0,
            build_number: 1,
        };

        assert_ok!(AcurastProcessorManager::update_binary_hash(RuntimeOrigin::root(), version, Some(hash.into())));

        let binary_location: BinaryLocation = b"https://github.com/Acurast/acurast-processor-update/releases/download/processor-1.3.31/processor-1.3.31-devnet.apk".to_vec().try_into().unwrap();
        let update_info = UpdateInfo {
            version,
            binary_location,
        };

        assert_err!(
            AcurastProcessorManager::set_processor_update_info(RuntimeOrigin::signed(alice_account_id()), update_info.clone(), vec![processor_account].try_into().unwrap()),
			Error::<Test>::ProcessorPairedWithAnotherManager,
        );
    });
}

#[test]
fn set_processor_update_info_failure_4() {
	ExtBuilder.build().execute_with(|| {
        let (manager_account, _) = paired_manager_processor();

        let hash = [1u8; 32];
        let version = Version {
            platform: 0,
            build_number: 1,
        };

        assert_ok!(AcurastProcessorManager::update_binary_hash(RuntimeOrigin::root(), version, Some(hash.into())));

        let binary_location: BinaryLocation = b"https://github.com/Acurast/acurast-processor-update/releases/download/processor-1.3.31/processor-1.3.31-devnet.apk".to_vec().try_into().unwrap();
        let update_info = UpdateInfo {
            version,
            binary_location,
        };

        assert_err!(
            AcurastProcessorManager::set_processor_update_info(RuntimeOrigin::signed(manager_account.clone()), update_info.clone(), vec![alice_account_id()].try_into().unwrap()),
            Error::<Test>::ProcessorHasNoManager,
        );
    });
}
