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

use core::marker::PhantomData;
use cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases;
use cumulus_primitives_core::{AggregateMessageOrigin, ParaId};
use derive_more::{From, Into};
use frame_support::{
	construct_runtime, derive_impl,
	dispatch::{DispatchInfo, DispatchResultWithPostInfo},
	genesis_builder_helper::{build_config, create_default_config},
	instances::Instance1,
	pallet_prelude::{InvalidTransaction, TransactionLongevity, ValidTransaction},
	parameter_types,
	traits::{
		fungible::{HoldConsideration, Inspect, Mutate},
		nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable},
		tokens::{Fortitude, Precision, Preservation},
		AsEnsureOriginWithArg, ConstBool, ConstU128, ConstU32, ConstU64, Currency, EitherOfDiverse,
		EnqueueWithOrigin, ExistenceRequirement, Imbalance, LinearStoragePrice, OnUnbalanced,
		TransformOrigin, WithdrawReasons,
	},
	unsigned::TransactionValidityError,
	weights::{
		ConstantMultiplier, Weight, WeightToFeeCoefficient, WeightToFeeCoefficients,
		WeightToFeePolynomial,
	},
	PalletId,
};
use frame_system::{
	EnsureRoot, EnsureRootWithSuccess, EnsureSigned, EnsureSignedBy, EnsureWithSuccess,
};
use parachains_common::message_queue::{NarrowOriginToSibling, ParaIdToSibling};
use parity_scale_codec::{Decode, Encode};
use polkadot_runtime_common::{
	xcm_sender::NoPriceForMessageDelivery, BlockHashCount, SlowAdjustingFeeUpdate,
};
use scale_info::TypeInfo;
use smallvec::smallvec;
use sp_api::impl_runtime_apis;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
	generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, BlakeTwo256, Block as BlockT, DispatchInfoOf, Dispatchable, One,
		PostDispatchInfoOf, SignedExtension, Zero,
	},
	transaction_validity::{TransactionSource, TransactionValidity},
	AccountId32, ApplyExtrinsicResult, DispatchError, Perbill,
};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use weights::{ExtrinsicBaseWeight, RocksDbWeight};
use xcm_config::XcmOriginToTransactDispatchOrigin;

/// Acurast Imports
use acurast_p256_crypto::MultiSignature;
pub use acurast_runtime_common::Balance;
use acurast_runtime_common::{
	barrier::Barrier, opaque, weight, weights, AccountId, Address, AuraId, BlockNumber,
	EnvKeyMaxSize, EnvValueMaxSize, ExtraFor, Hash, MaxAllowedSources, MaxEnvVars, MaxSlots,
	MaxVersions, Nonce, RewardDistributor, Signature, MILLIUNIT, UNIT,
};
use pallet_acurast::{Attestation, EnvironmentFor, JobId, MultiOrigin, CU32};
use pallet_acurast_hyperdrive::{IncomingAction, ParsedAction, ProxyChain};
use pallet_acurast_hyperdrive_ibc::{LayerFor, MessageBody, SubjectFor};
use pallet_acurast_marketplace::{
	JobAssignmentFor, MarketplaceHooks, PartialJobRegistration, PubKey, PubKeys, RuntimeApiError,
};

pub use constants::*;
pub use types::*;
pub use utils::*;

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime
	{
		// System support stuff.
		System: frame_system = 0,
		ParachainSystem: cumulus_pallet_parachain_system = 1,
		Timestamp: pallet_timestamp = 2,
		ParachainInfo: parachain_info = 3,
		Sudo: pallet_sudo = 4,
		Scheduler: pallet_scheduler = 5,
		Preimage: pallet_preimage = 6,
		Multisig: pallet_multisig = 7,
		Utility: pallet_utility = 8,

		// Monetary stuff.
		Balances: pallet_balances = 10,
		TransactionPayment: pallet_transaction_payment = 11,
		// (keep comment, just so we know that pallet assets used to be on this pallet index)
		// Assets: pallet_assets::{Pallet, Storage, Event<T>, Config<T>} = 12,
		// (keep comment, just so we know that pallet assets used to be on this pallet index)
		// AcurastAssets: pallet_acurast_assets_manager::{Pallet, Storage, Event<T>, Config<T>, Call} = 13,
		Uniques: pallet_uniques = 14,

		// Governance stuff.
		Democracy: pallet_democracy = 15,

		// Consensus. The order of these are important and shall not change.
		AcurastVesting: pallet_acurast_vesting = 17,
		// (keep comment, just so we know that pallet_parachain_staking used to be on this pallet index)
		// ParachainStaking: pallet_parachain_staking::{Pallet, Call, Storage, Event<T>, Config<T>} = 18,
		// (keep comment, just so we know that pallet_author_inherent used to be on this pallet index)
		// AuthorInherent: pallet_author_inherent::{Pallet, Call, Storage, Inherent} = 19,
		Authorship: pallet_authorship = 20,
		CollatorSelection: pallet_collator_selection = 21,
		Session: pallet_session = 22,
		Aura: pallet_aura = 23,
		AuraExt: cumulus_pallet_aura_ext = 24,

		// XCM helpers.
		XcmpQueue: cumulus_pallet_xcmp_queue = 30,
		PolkadotXcm: pallet_xcm = 31,
		CumulusXcm: cumulus_pallet_xcm = 32,
		DmpQueue: cumulus_pallet_dmp_queue = 33,
		MessageQueue: pallet_message_queue = 34,

		// Acurast pallets
		Acurast: pallet_acurast = 40,
		AcurastProcessorManager: pallet_acurast_processor_manager = 41,
		AcurastFeeManager: pallet_acurast_fee_manager::<Instance1> = 42,
		AcurastMarketplace: pallet_acurast_marketplace = 43,
		AcurastMatcherFeeManager: pallet_acurast_fee_manager::<Instance2> = 44,
		AcurastHyperdrive: pallet_acurast_hyperdrive::<Instance1> = 45,
		// AcurastHyperdriveOutgoingTezos: pallet_acurast_hyperdrive_outgoing::<Instance1> = 46,
		AcurastRewardsTreasury: pallet_acurast_rewards_treasury = 47,
		// HyperdriveEthereum: pallet_acurast_hyperdrive::<Instance2> = 48,
		// HyperdriveOutgoingEthereum: pallet_acurast_hyperdrive_outgoing::<Instance2> = 49,
		// HyperdriveAlephZero: pallet_acurast_hyperdrive::<Instance3> = 50,
		// HyperdriveOutgoingAlephZero: pallet_acurast_hyperdrive_outgoing::<Instance3> = 51,
		AcurastHyperdriveIbc: pallet_acurast_hyperdrive_ibc::<Instance1> = 52,
	}
);

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
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			Aura::authorities().into_inner()
		}
	}

	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
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
			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

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

			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
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
		fn create_default_config() -> Vec<u8> {
			create_default_config::<RuntimeGenesisConfig>()
		}

		fn build_config(config: Vec<u8>) -> sp_genesis_builder::Result {
			build_config::<RuntimeGenesisConfig>(config)
		}
	}
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
	CheckInherents = CheckInherents,
}
