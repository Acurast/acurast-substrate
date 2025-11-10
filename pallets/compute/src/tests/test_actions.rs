use frame_support::{assert_ok, traits::Hooks};
use sp_core::bounded_vec;
use sp_runtime::{traits::AccountIdConversion, AccountId32, FixedU128, Perbill, Perquintill};
use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::{
	mock::{InflationPerEpoch, *},
	stub::*,
	types::*,
	Config, Cycle, Event,
};
use acurast_common::{CommitmentIdProvider, ComputeHooks, ManagerIdProvider, ManagerLookup};

/// Test helper function for compute pallet that simplifies test writing
pub fn compute_test_flow(
	executions: usize,
	pools: &[u8],
	committers: &[(&str, &[&str])],
	actions: &[Action],
) {
	let mut flow = ComputeTestFlow {
		pools: pools.to_vec(),
		committers: committers
			.iter()
			.map(|(committer, processors)| {
				(committer.to_string(), processors.iter().map(|s| s.to_string()).collect())
			})
			.collect(),
		actions: actions.to_vec(),
		commitment_ids: Vec::new(),
	};

	flow.setup();
	for n in 0..executions {
		flow.execute(n);
	}
}

#[derive(Debug, Clone)]
pub enum Action {
	CommitCompute {
		committer: String,
		stake: Balance,
		cooldown: u64,
		metrics: Vec<(u8, u128, u128)>, // (pool_id, numerator, denominator)
		commission: Perbill,
	},
	StakeMore {
		committer: String,
		extra_amount: Balance,
	},
	Delegate {
		delegator: String,
		committer: String,
		amount: Balance,
		cooldown: u64,
	},
	WithdrawDelegation {
		delegator: String,
		committer: String,
		expected_reward: Balance,
	},
	WithdrawCommitment {
		committer: String,
		expected_reward: Balance,
	},
	CooldownComputeCommitment {
		committer: String,
	},
	CooldownDelegation {
		delegator: String,
		committer: String,
	},
	EndComputeCommitment {
		committer: String,
		expected_reward: Balance,
	},
	EndDelegation {
		delegator: String,
		committer: String,
		expected_reward: Balance,
	},
	RollToBlock {
		block_number: u64,
		expected_cycle: Cycle<u64, u64>,
	},
	ProcessorCommit {
		processor: String,
		metrics: Vec<(u8, u128, u128)>,
	}, // (pool_id, numerator, denominator)
}

impl Display for Action {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Action::CommitCompute { committer, stake, cooldown, metrics: _, commission: _ } => {
				write!(
					f,
					"CommitCompute(committer={}, stake={}, cooldown={})",
					committer, stake, cooldown
				)
			},
			Action::StakeMore { committer, extra_amount } => {
				write!(f, "StakeMore(committer={}, extra_amount={})", committer, extra_amount)
			},
			Action::WithdrawDelegation { delegator, committer, expected_reward: _ } => {
				write!(f, "WithdrawDelegation(delegator={}, committer={})", delegator, committer)
			},
			Action::WithdrawCommitment { committer, expected_reward: _ } => {
				write!(f, "WithdrawCommitment(committer={})", committer)
			},
			Action::Delegate { delegator, committer, amount, cooldown } => {
				write!(
					f,
					"Delegate(delegator={}, committer={}, amount={}, cooldown={})",
					delegator, committer, amount, cooldown
				)
			},
			Action::CooldownComputeCommitment { committer } => {
				write!(f, "CooldownComputeCommitment(committer={})", committer)
			},
			Action::CooldownDelegation { delegator, committer } => {
				write!(f, "CooldownDelegation(delegator={}, committer={})", delegator, committer)
			},
			Action::EndComputeCommitment { committer, expected_reward } => {
				write!(
					f,
					"EndComputeCommitment(committer={}, expected_reward={})",
					committer, expected_reward
				)
			},
			Action::EndDelegation { delegator, committer, expected_reward } => {
				write!(
					f,
					"EndDelegation(delegator={}, committer={}, expected_reward={})",
					delegator, committer, expected_reward
				)
			},
			Action::RollToBlock { block_number, expected_cycle: _ } => {
				write!(f, "RollToBlock(block_number={})", block_number)
			},
			Action::ProcessorCommit { processor, metrics } => {
				write!(f, "ProcessorCommit(processor={}, metrics={:?})", processor, metrics)
			},
		}
	}
}

pub struct ComputeTestFlow {
	pub pools: Vec<u8>,
	pub committers: Vec<(String, Vec<String>)>,
	pub actions: Vec<Action>,
	commitment_ids: Vec<u128>,
}

impl ComputeTestFlow {
	fn print_repetition(repetition: usize) {
		println!(
			"\n╔═══════════════════════════════════════════════════════════════════════ rep: {} ═══"
		,repetition);
		println!("║ START REPETITION: {}", repetition);
		println!(
			"╚══════════════════════════════════════════════════════════════════════════════════"
		);
	}

	fn print_action(action: &Action, repetition: usize) {
		println!(
			"\n╔═══════════════════════════════════════════════════════════════════════ rep: {} ═══"
		,repetition);
		println!("║ ACTION: {}", action);
		println!(
			"╚══════════════════════════════════════════════════════════════════════════════════"
		);
	}

	fn print_storage_state() {
		// Print current cycle
		let cycle = Compute::current_cycle();
		println!("🕐 Current Cycle:");
		println!("   └─ epoch={}, epoch_start={}", cycle.epoch, cycle.epoch_start);

		// TODO: Update after refactoring - staking_pools storage no longer exists
		// // Print staking pools with their members consolidated below each one
		// for pool_id in 1..=10u8 {
		// 	let pool = Compute::staking_pools(pool_id);
		// 	// Only show if it has meaningful data
		// 	if !pool.reward_weight.is_zero() || !pool.reward_per_token.is_zero() {
		// 		println!(
		// 			"\n🎯 Staking Pool [{}]: 🔘reward_weight={:>12}, 💰reward_per_token={:>12}",
		// 			pool_id, pool.reward_weight, pool.reward_per_token
		// 		);
		//
		// 		// Print members of this staking pool in single-line format
		// 		for cid in 0..10u128 {
		// 			if let Some(member) = Compute::staking_pool_members(cid, pool_id) {
		// 				// Get stake info for this commitment
		// 				let stakes = Compute::stakes(cid);
		// 				if let Some(stake) = stakes {
		// 					let cooldown_info = match stake.cooldown_started {
		// 						Some(start_block) => {
		// 							format!("{}+{}", start_block, stake.cooldown_period)
		// 						},
		// 						None => format!("X+{}", stake.cooldown_period),
		// 					};
		// 					println!("   └─ [cid={}]       amount={:>12},  accrued_reward={:>12},  cooldown={:>12},  reward_weight={:>12},  reward_debt={:>12}",
		// 						cid, stake.amount, stake.accrued_reward, cooldown_info, member.reward_weight, member.reward_debt);
		// 				} else {
		// 					println!("   └─ [cid={}]       amount={:>12},  accrued_reward={:>12},  cooldown={:>12},  reward_weight={:>12},  reward_debt={:>12}",
		// 						cid, "N/A", "N/A", "N/A", member.reward_weight, member.reward_debt);
		// 				}
		// 			}
		// 		}
		// 	}
		// }

		// TODO: Update after refactoring - delegation_pools, self_delegation, delegation_pool_members storage no longer exists
		// // Print delegation pools with their members consolidated below each one
		// for cid in 0..10u128 {
		// 	let pool = Compute::delegation_pools(cid);
		// 	// Only show if it has meaningful data (non-zero values)
		// 	if !pool.reward_weight.is_zero()
		// 		|| !pool.slash_weight.is_zero()
		// 		|| !pool.reward_per_token.is_zero()
		// 		|| !pool.slash_per_token.is_zero()
		// 	{
		// 		println!("\n🏊 Delegation Pool [cid={}]: 🔘reward_weight={:>12}, 🔘slash_weight={:>12}, 💰reward_per_token={:>12}, 💰slash_per_token={:>12}",
		// 			cid, pool.reward_weight, pool.slash_weight, pool.reward_per_token, pool.slash_per_token);
		//
		// 		// Print self delegation first (visually separated)
		// 		if let Some(self_del) = Compute::self_delegation(cid) {
		// 			println!("   └─ [Self]     reward_weight={:>12},    slash_weight={:>12},    reward_debt={:>12},     slash_debt={:>12}",
		// 				self_del.reward_weight, self_del.slash_weight, self_del.reward_debt, self_del.slash_debt);
		// 		}
		//
		// 		// Print delegation pool members
		// 		let test_accounts = [
		// 			("A", alice_account_id()),
		// 			("B", bob_account_id()),
		// 			("C", charlie_account_id()),
		// 			("D", dave_account_id()),
		// 			("E", eve_account_id()),
		// 			("F", ferdie_account_id()),
		// 			("G", george_account_id()),
		// 			("H", henry_account_id()),
		// 			("I", ivan_account_id()),
		// 		];
		//
		// 		for (name, account) in test_accounts.iter() {
		// 			if let Some(delegation) = Compute::delegations(account.clone(), cid) {
		// 				if let Some(member) = Compute::delegation_pool_members(account.clone(), cid)
		// 				{
		// 					let cooldown_info = match delegation.stake.cooldown_started {
		// 						Some(start_block) => {
		// 							format!("{}+{}", start_block, delegation.stake.cooldown_period)
		// 						},
		// 						None => format!("X+{}", delegation.stake.cooldown_period),
		// 					};
		// 					println!("   └─ [{}]          amount={:>12},  cooldown={:>12},  accrued_reward={:>12},  reward_weight={:>12},    slash_weight={:>12},    reward_debt={:>12},     slash_debt={:>12}",
		// 						name, delegation.stake.amount, cooldown_info, delegation.stake.accrued_reward, member.reward_weight, member.slash_weight, member.reward_debt, member.slash_debt);
		// 				}
		// 			}
		// 		}
		// 	}
		// }
	}

	pub fn setup(&mut self) {
		// Set inflation per epoch to 0 for tests
		InflationPerEpoch::set(142857142857 * NANOUNIT); // 142.857142857 makes 70% be 100 * UNITS

		// Setup
		setup_balances();

		// Create pools
		for (i, &percent) in self.pools.iter().enumerate() {
			let pool_name = match i {
				0 => *b"cpu-ops-per-second______",
				1 => *b"mem-read-count-per-sec--",
				2 => *b"mem-write-count-per-sec-",
				3 => *b"network-io-count-per-sec",
				4 => *b"disk-io-count-per-second",
				_ => panic!("Too many pools defined"),
			};

			assert_ok!(Compute::create_pool(
				RuntimeOrigin::root(),
				pool_name,
				Perquintill::from_percent(percent as u64),
				bounded_vec![],
			));
		}

		// Setup committers and their processor mappings
		MockManagerProvider::clear_mappings();
		for (committer_name, processors) in &self.committers {
			let committer_account = Self::name_to_account(committer_name);

			// Setup processor -> manager mappings
			for processor_name in processors {
				let processor_account = Self::name_to_account(processor_name);
				MockManagerProvider::set_mapping(processor_account, committer_account.clone());
			}

			// Do offer_accept_backing flow
			Self::setup_backing(&committer_account);

			self.commitment_ids.push(
				<Test as Config>::CommitmentIdProvider::commitment_id_for(&committer_account)
					.unwrap(),
			);
		}
	}

	pub fn execute(&self, repetition: usize) {
		Self::print_repetition(repetition);

		// Print initial state
		Self::print_storage_state();

		let mut pool_ids = vec![];
		// Execute actions
		for action in &self.actions {
			Self::print_action(action, repetition);

			match action {
				Action::CommitCompute { committer, stake, cooldown, metrics, commission } => {
					let account = Self::name_to_account(committer);
					pool_ids.extend(Self::execute_commit_compute(
						&account,
						*stake,
						*cooldown,
						metrics,
						*commission,
					));
				},
				Action::StakeMore { committer, extra_amount } => {
					let account = Self::name_to_account(committer);
					Self::execute_stake_more(&account, *extra_amount);
				},
				Action::WithdrawDelegation { delegator, committer, expected_reward } => {
					let delegator_account = Self::name_to_account(delegator);
					let committer_account = Self::name_to_account(committer);
					Self::execute_withdraw_delegation(
						&delegator_account,
						&committer_account,
						*expected_reward,
					);
				},
				Action::WithdrawCommitment { committer, expected_reward } => {
					let committer_account = Self::name_to_account(committer);
					Self::execute_withdraw_commitment(&committer_account, *expected_reward);
				},
				Action::Delegate { delegator, committer, amount, cooldown } => {
					let delegator_account = Self::name_to_account(delegator);
					let committer_account = Self::name_to_account(committer);
					Self::execute_delegate(
						&delegator_account,
						&committer_account,
						*amount,
						*cooldown,
					);
				},
				Action::CooldownComputeCommitment { committer } => {
					let account = Self::name_to_account(committer);
					Self::execute_cooldown_compute_commitment(&account);
				},
				Action::CooldownDelegation { delegator, committer } => {
					let delegator_account = Self::name_to_account(delegator);
					let committer_account = Self::name_to_account(committer);
					Self::execute_cooldown_delegation(&delegator_account, &committer_account);
				},
				Action::EndComputeCommitment { committer, expected_reward } => {
					let account = Self::name_to_account(committer);
					Self::execute_end_compute_commitment(&account, *expected_reward);
				},
				Action::EndDelegation { delegator, committer, expected_reward } => {
					let delegator_account = Self::name_to_account(delegator);
					let committer_account = Self::name_to_account(committer);
					Self::execute_end_delegation(
						&delegator_account,
						&committer_account,
						*expected_reward,
					);
				},
				Action::RollToBlock { block_number, expected_cycle } => {
					roll_to_block(*block_number + (1000usize * repetition) as u64);

					let actual_cycle = Compute::current_cycle();
					if repetition == 0 {
						assert_eq!(
							actual_cycle, *expected_cycle,
							"Expected cycle {:?} at block {}, got {:?}",
							expected_cycle, block_number, actual_cycle
						);
					}
				},
				Action::ProcessorCommit { processor, metrics } => {
					let account = Self::name_to_account(processor);
					Self::execute_processor_commit(&account, metrics);
				},
			}

			Self::print_storage_state();
		}

		// TODO: Update after refactoring - staking_pool_members storage no longer exists
		// for pool_id in pool_ids {
		// 	for commitment_id in &self.commitment_ids {
		// 		let staking_pool = Compute::staking_pool_members(*commitment_id, pool_id);
		// 		assert!(staking_pool.is_none(), "staking_pool_members(commitment_id: {}, pool_id: {}) should be None but was {:?}", commitment_id, pool_id, staking_pool.unwrap());
		//
		// 		let compute_commitment = Compute::compute_commitments(*commitment_id, pool_id);
		// 		assert!(compute_commitment.is_none(), "compute_commitment(commitment_id: {}, pool_id: {}) should be None but was {:?}", commitment_id, pool_id, compute_commitment.unwrap());
		// 	}
		// }
		let _ = pool_ids;
	}

	fn name_to_account(name: &str) -> AccountId32 {
		match name {
			"A" => alice_account_id(),
			"B" => bob_account_id(),
			"C" => charlie_account_id(),
			"D" => dave_account_id(),
			"E" => eve_account_id(),
			"F" => ferdie_account_id(),
			"G" => george_account_id(),
			"H" => henry_account_id(),
			"I" => ivan_account_id(),
			"J" => judy_account_id(),
			"K" => kate_account_id(),
			"L" => luke_account_id(),
			"M" => maria_account_id(),
			"N" => nick_account_id(),
			_ => panic!("Unknown account name: {}", name),
		}
	}

	fn setup_backing(who: &AccountId32) {
		// Create manager ID and setup backing
		let _manager_id =
			<Test as Config>::ManagerIdProvider::manager_id_for(who).unwrap_or_else(|_| {
				// Generate a unique manager ID using thread-local counter
				use std::cell::RefCell;
				thread_local! {
					static MANAGER_COUNTER: RefCell<u128> = RefCell::new(1);
				}

				let id = MANAGER_COUNTER.with(|counter| {
					let mut counter = counter.borrow_mut();
					let id = *counter;
					*counter += 1;
					id
				});

				assert_ok!(<Test as Config>::ManagerIdProvider::create_manager_id(id, who));
				id
			});

		assert_ok!(Compute::offer_backing(RuntimeOrigin::signed(who.clone()), who.clone()));
		assert_ok!(Compute::accept_backing_offer(RuntimeOrigin::signed(who.clone()), who.clone()));
	}

	fn execute_commit_compute(
		who: &AccountId32,
		stake: u128,
		cooldown: u64,
		metrics: &Vec<(u8, u128, u128)>,
		commission: Perbill,
	) -> Vec<u8> {
		let mut pool_ids = vec![];
		let commitments: Vec<ComputeCommitment> = metrics
			.iter()
			.map(|(pool_id, numerator, denominator)| {
				pool_ids.push(*pool_id);
				ComputeCommitment {
					pool_id: *pool_id,
					metric: FixedU128::from_rational(*numerator, *denominator),
				}
			})
			.collect();

		let commitment = commitments.try_into().expect("Too many commitments");

		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(who.clone()),
			stake,
			cooldown,
			commitment,
			commission,
			true,
		));

		pool_ids
	}

	fn execute_stake_more(committer: &AccountId32, extra_amount: u128) {
		assert_ok!(Compute::stake_more(
			RuntimeOrigin::signed(committer.clone()),
			extra_amount,
			None,
			None,
			None,
			None
		));
	}

	fn execute_withdraw_delegation(
		delegator: &AccountId32,
		committer: &AccountId32,
		expected_reward: u128,
	) {
		events();
		assert_ok!(Compute::withdraw_delegation(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone()
		));
		assert_transferred_amount(delegator, expected_reward);
	}

	fn execute_withdraw_commitment(committer: &AccountId32, expected_reward: u128) {
		events();
		assert_ok!(Compute::withdraw_commitment(RuntimeOrigin::signed(committer.clone())));
		assert_transferred_amount(committer, expected_reward);
	}

	fn execute_delegate(
		delegator: &AccountId32,
		committer: &AccountId32,
		amount: u128,
		cooldown: u64,
	) {
		assert_ok!(Compute::delegate(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
			amount,
			cooldown,
			true,
		));
	}

	fn execute_cooldown_compute_commitment(who: &AccountId32) {
		let commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&who).unwrap();

		// TODO: Update after refactoring - stakes() and self_delegation() storage no longer exists
		// let prev_stake = Compute::stakes(commitment_id).unwrap();
		// let prev_self_delegation = Compute::self_delegation(commitment_id).unwrap();
		let prev_commitment = Compute::commitments(commitment_id).unwrap();
		let prev_stake = prev_commitment.stake.as_ref().unwrap();

		assert_ok!(Compute::cooldown_compute_commitment(RuntimeOrigin::signed(who.clone())));

		// let after_stake = Compute::stakes(commitment_id).unwrap();
		// let after_self_delegation = Compute::self_delegation(commitment_id).unwrap();
		let after_commitment = Compute::commitments(commitment_id).unwrap();
		let after_stake = after_commitment.stake.as_ref().unwrap();

		assert!(after_stake.rewardable_amount < prev_stake.rewardable_amount);
		// TODO: Verify weight changes in commitment structure
		// assert!(after_self_delegation.reward_weight < prev_self_delegation.reward_weight);
	}

	fn execute_cooldown_delegation(delegator: &AccountId32, committer: &AccountId32) {
		assert_ok!(Compute::cooldown_delegation(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
		));
	}

	fn execute_end_compute_commitment(who: &AccountId32, expected_reward: u128) {
		// Clear events before the operation
		events();
		assert_ok!(Compute::end_compute_commitment(RuntimeOrigin::signed(who.clone())));
		assert_transferred_amount(who, expected_reward);
	}

	fn execute_end_delegation(
		delegator: &AccountId32,
		committer: &AccountId32,
		expected_reward: u128,
	) {
		// Clear events before the operation
		events();

		assert_ok!(Compute::end_delegation(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
		));

		// Check for Transfer event (reward) and DelegationEnded event
		let all_events = events();

		// Look for Transfer events to this delegator (rewards)
		let transfer_events: Vec<_> = all_events
			.iter()
			.filter_map(|e| match e {
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: _,
					to,
					amount,
				}) if *to == *delegator => Some(*amount),
				_ => None,
			})
			.collect();

		// Look for DelegationEnded event
		let delegation_ended: bool = all_events.iter().any(|e| match e {
			RuntimeEvent::Compute(Event::DelegationEnded(del, _, _)) if *del == *delegator => true,
			_ => false,
		});

		assert!(delegation_ended, "No DelegationEnded event found for delegator");

		if expected_reward > 0 {
			assert!(!transfer_events.is_empty(), "No transfer event found for ending delegation");
			assert!(
				expected_reward >= transfer_events[0],
				"Got expected reward {} smaller than actual {}",
				expected_reward,
				transfer_events[0]
			);

			assert!(
				expected_reward - transfer_events[0] < 1 * MICROUNIT, // allow for small rounding error
				"Expected reward {} but got {}",
				expected_reward,
				transfer_events[0]
			);
		} else if expected_reward == 0 && !transfer_events.is_empty() {
			assert!(transfer_events.is_empty(), "Actual reward received: {}", transfer_events[0]);
		}
	}

	fn execute_processor_commit(processor: &AccountId32, metrics: &Vec<(u8, u128, u128)>) {
		let commit_data: Vec<(u8, u128, u128)> = metrics.iter().cloned().collect();
		let manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(processor).unwrap();
		let _ = Compute::commit(processor, &manager, &commit_data);
	}
}

pub fn setup_balances() {
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		<Test as Config>::PalletId::get().into_account_truncating(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		alice_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		bob_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		charlie_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		dave_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		eve_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		ferdie_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		george_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		henry_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		ivan_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		judy_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		kate_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		luke_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		maria_account_id(),
		1_000_000_000 * UNIT
	));
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		nick_account_id(),
		1_000_000_000 * UNIT
	));
}

pub fn roll_to_block(block_number: u64) {
	let current_block = System::block_number();
	for block in current_block + 1..=block_number {
		System::set_block_number(block);
		Compute::on_initialize(block);
	}
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}

fn first_transfer_event(who: &AccountId) -> Option<Balance> {
	// Check for transfer event with expected reward amount
	let transfer_events: Vec<_> = events()
		.into_iter()
		.filter_map(|e| match e {
			RuntimeEvent::Balances(pallet_balances::Event::Transfer { from: _, to, amount })
				if to == *who =>
			{
				Some(amount)
			},
			_ => None,
		})
		.collect();

	transfer_events.first().copied()
}

fn assert_transferred_amount(who: &AccountId32, expected_reward: u128) {
	let transferred_amount = first_transfer_event(who);
	if expected_reward > 0 {
		assert!(
			transferred_amount.is_some(),
			"No transfer event found for ending compute commitment"
		);
		let transferred_amount = transferred_amount.unwrap();
		assert!(
			expected_reward >= transferred_amount,
			"Got expected reward {} smaller than actual {}",
			expected_reward,
			transferred_amount
		);
		assert!(
			expected_reward - transferred_amount < 1 * MICROUNIT, // allow for small rounding error
			"Expected reward {} but got {}",
			expected_reward,
			transferred_amount
		);
	} else if expected_reward == 0 {
		assert!(
			transferred_amount.is_none(),
			"Actual reward received: {}",
			transferred_amount.unwrap()
		);
	}
}
