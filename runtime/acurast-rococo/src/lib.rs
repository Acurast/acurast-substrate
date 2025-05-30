#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

extern crate alloc;

pub mod apis;
#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
mod constants;
mod pallets;
mod types;
mod utils;
pub mod xcm_config;

pub use acurast_runtime_common::types::Balance;
use acurast_runtime_common::types::{AccountId, Nonce};
use alloc::vec::Vec;
pub use constants::*;
pub use types::*;
pub use utils::*;

#[frame_support::runtime]
mod runtime {
	#[runtime::runtime]
	#[runtime::derive(
		RuntimeCall,
		RuntimeEvent,
		RuntimeError,
		RuntimeOrigin,
		RuntimeFreezeReason,
		RuntimeHoldReason,
		RuntimeSlashReason,
		RuntimeLockId,
		RuntimeTask,
		RuntimeViewFunction
	)]
	pub struct Runtime;

	#[runtime::pallet_index(0)]
	pub type System = frame_system;
	#[runtime::pallet_index(1)]
	pub type ParachainSystem = cumulus_pallet_parachain_system;
	#[runtime::pallet_index(2)]
	pub type Timestamp = pallet_timestamp;
	#[runtime::pallet_index(3)]
	pub type ParachainInfo = parachain_info;
	#[runtime::pallet_index(4)]
	pub type Sudo = pallet_sudo;
	#[runtime::pallet_index(5)]
	pub type Scheduler = pallet_scheduler;
	#[runtime::pallet_index(6)]
	pub type Preimage = pallet_preimage;
	#[runtime::pallet_index(7)]
	pub type Multisig = pallet_multisig;
	#[runtime::pallet_index(8)]
	pub type Utility = pallet_utility;
	#[runtime::pallet_index(9)]
	pub type WeightReclaim = cumulus_pallet_weight_reclaim;

	// Monetary stuff.
	#[runtime::pallet_index(10)]
	pub type Balances = pallet_balances;
	#[runtime::pallet_index(11)]
	pub type TransactionPayment = pallet_transaction_payment;
	#[runtime::pallet_index(14)]
	pub type Uniques = pallet_uniques;

	// Governance stuff.
	#[runtime::pallet_index(15)]
	pub type Democracy = pallet_democracy;

	// Consensus. The order of these are important and shall not change.
	#[runtime::pallet_index(20)]
	pub type Authorship = pallet_authorship;
	#[runtime::pallet_index(21)]
	pub type CollatorSelection = pallet_collator_selection;
	#[runtime::pallet_index(22)]
	pub type Session = pallet_session;
	#[runtime::pallet_index(23)]
	pub type Aura = pallet_aura;
	#[runtime::pallet_index(24)]
	pub type AuraExt = cumulus_pallet_aura_ext;

	// XCM helpers.
	#[runtime::pallet_index(30)]
	pub type XcmpQueue = cumulus_pallet_xcmp_queue;
	#[runtime::pallet_index(31)]
	pub type PolkadotXcm = pallet_xcm;
	#[runtime::pallet_index(32)]
	pub type CumulusXcm = cumulus_pallet_xcm;
	#[runtime::pallet_index(34)]
	pub type MessageQueue = pallet_message_queue;

	// Acurast pallets
	#[runtime::pallet_index(40)]
	pub type Acurast = pallet_acurast;
	#[runtime::pallet_index(41)]
	pub type AcurastProcessorManager = pallet_acurast_processor_manager;
	#[runtime::pallet_index(42)]
	pub type AcurastFeeManager = pallet_acurast_fee_manager<Instance1>;
	#[runtime::pallet_index(43)]
	pub type AcurastMarketplace = pallet_acurast_marketplace;
	#[runtime::pallet_index(44)]
	pub type AcurastMatcherFeeManager = pallet_acurast_fee_manager<Instance2>;
	#[runtime::pallet_index(45)]
	pub type AcurastHyperdrive = pallet_acurast_hyperdrive<Instance1>;
	#[runtime::pallet_index(47)]
	pub type AcurastRewardsTreasury = pallet_acurast_rewards_treasury;
	#[runtime::pallet_index(48)]
	pub type AcurastCompute = pallet_acurast_compute;
	#[runtime::pallet_index(52)]
	pub type AcurastHyperdriveIbc = pallet_acurast_hyperdrive_ibc<Instance1>;
	#[runtime::pallet_index(53)]
	pub type AcurastHyperdriveToken = pallet_acurast_hyperdrive_token<Instance1>;
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}
