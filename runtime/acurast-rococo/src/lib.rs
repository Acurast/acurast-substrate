#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
mod check_nonce;
mod constants;
mod implementations;
mod pallets;
mod types;
mod utils;
pub mod xcm_config;

use frame_support::{genesis_builder_helper, weights::Weight};
use sp_api::impl_runtime_apis;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
	traits::Block as BlockT,
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult,
};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

/// Acurast Imports
pub use acurast_runtime_common::types::Balance;
use acurast_runtime_common::types::{
	AccountId, AuraId, EnvKeyMaxSize, EnvValueMaxSize, ExtraFor, MaxAllowedSources, MaxEnvVars,
	Nonce,
};
use pallet_acurast::{Attestation, EnvironmentFor, JobId, MultiOrigin};
use pallet_acurast_marketplace::{JobAssignmentFor, PartialJobRegistration, RuntimeApiError};

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
		RuntimeTask
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
	#[runtime::pallet_index(17)]
	pub type AcurastVesting = pallet_acurast_vesting;

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
	#[runtime::pallet_index(52)]
	pub type AcurastHyperdriveIbc = pallet_acurast_hyperdrive_ibc<Instance1>;
}

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate frame_benchmarking;
extern crate core;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	define_benchmarks!(
		// TODO uncomment with fixed version of cumulus-pallet-parachain-system that includes PR https://github.com/paritytech/cumulus/pull/2766/files
		// [frame_system, SystemBench::<Runtime>]
		// [pallet_timestamp, Timestamp]
		// [pallet_multisig, Multisig]
		// [pallet_balances, Balances]
		// [pallet_democracy, Democracy]
		// [pallet_collator_selection, CollatorSelection]
		// [pallet_session, SessionBench::<Runtime>]
		// [cumulus_pallet_xcmp_queue, XcmpQueue]
		[pallet_acurast, Acurast]
		[pallet_acurast_processor_manager, AcurastProcessorManager]
		[pallet_acurast_fee_manager, AcurastFeeManager]
		[pallet_acurast_marketplace, AcurastMarketplace]
		// [pallet_acurast_hyperdrive, AcurastHyperdrive]
		[pallet_acurast_vesting, AcurastVesting]
	);
}

impl_runtime_apis! {
	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(SLOT_DURATION)
		}

		fn authorities() -> Vec<AuraId> {
			pallet_aura::Authorities::<Runtime>::get().into_inner()
		}
	}

	impl cumulus_primitives_aura::AuraUnincludedSegmentApi<Block> for Runtime {
		fn can_build_upon(
			included_hash: <Block as BlockT>::Hash,
			slot: cumulus_primitives_aura::Slot,
		) -> bool {
			ConsensusHook::can_build_upon(included_hash, slot)
		}
	}

	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}

		fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
			Runtime::metadata_at_version(version)
		}

		fn metadata_versions() -> sp_std::vec::Vec<u32> {
			Runtime::metadata_versions()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(account: AccountId) -> Nonce {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
		for Runtime
	{
		fn query_call_info(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_call_info(call, len)
		}
		fn query_call_fee_details(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_call_fee_details(call, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_acurast_marketplace::MarketplaceRuntimeApi<Block, Balance, AccountId, ExtraFor<Runtime>, MaxAllowedSources, MaxEnvVars, EnvKeyMaxSize, EnvValueMaxSize> for Runtime {
		fn filter_matching_sources(
			registration: PartialJobRegistration<Balance, AccountId, MaxAllowedSources>,
			sources: Vec<AccountId>,
			consumer: Option<MultiOrigin<AccountId>>,
			latest_seen_after: Option<u128>,
		) -> Result<Vec<AccountId>, RuntimeApiError> {
			AcurastMarketplace::filter_matching_sources(registration, sources, consumer, latest_seen_after)
		}

		fn job_environment(
			job_id: JobId<AccountId>,
			source: AccountId,
		) -> Result<Option<EnvironmentFor<Runtime>>, RuntimeApiError> {
			Ok(Acurast::execution_environment(job_id, source))
		}

		fn matched_jobs(
			source: AccountId,
		) -> Result<Vec<JobAssignmentFor<Runtime>>, RuntimeApiError> {
			AcurastMarketplace::stored_matches_for_source(source)
		}

		fn attestation(
			source: AccountId,
		) -> Result<Option<Attestation>, RuntimeApiError>{
			Ok(Acurast::stored_attestation(source))
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: bool) -> (Weight, Weight) {
			log::info!("try-runtime::on_runtime_upgrade parachain-acurast.");
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(block: Block, state_root_check: bool, signature_check: bool, try_state: frame_try_runtime::TryStateSelect) -> Weight {
			log::info!(
				target: "runtime::parachain-acurast", "try-runtime: executing block #{} ({:?}) / root checks: {:?} / sanity-checks: {:?}",
				block.header.number,
				block.header.hash(),
				state_root_check,
				try_state,
			);
			Executive::try_execute_block(block, state_root_check, signature_check, try_state).expect("try_execute_block failed")
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			return (list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch};
			use frame_support::traits::TrackedStorageKey;

			impl frame_system_benchmarking::Config for Runtime {
				// TODO uncomment with fixed version of cumulus-pallet-parachain-system that includes PR https://github.com/paritytech/cumulus/pull/2766/files
				// fn setup_set_code_requirements(code: &sp_std::vec::Vec<u8>) -> Result<(), BenchmarkError> {
				// 	ParachainSystem::initialize_for_set_code_benchmark(code.len() as u32);
				// 	 Ok(())
				// }
				//
				// fn verify_set_code() {
				// 	System::assert_last_event(cumulus_pallet_parachain_system::Event::<Runtime>::ValidationFunctionStored.into());
				// }
			}

			impl cumulus_pallet_session_benchmarking::Config for Runtime {}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}

	impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
		fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
			genesis_builder_helper::build_state::<RuntimeGenesisConfig>(config)
		}

		fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
			genesis_builder_helper::get_preset::<RuntimeGenesisConfig>(id, |_| None)
		}

		fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
			vec![]
		}
	}
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}
