use crate::{stub::*, *};
use frame_support::{
	derive_impl,
	sp_runtime::{
		traits::{ConstU128, ConstU32, IdentityLookup},
		BuildStorage, MultiSignature,
	},
	traits::{
		fungible::{Inspect, Mutate},
		nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable},
		tokens::{Fortitude, Precision, Preservation},
		AsEnsureOriginWithArg, ConstU16, ConstU64,
	},
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
#[cfg(feature = "runtime-benchmarks")]
use sp_core::crypto::UncheckedFrom;
use sp_core::H256;
use sp_std::prelude::*;

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: vec![
				(alice_account_id(), INITIAL_BALANCE),
				(bob_account_id(), INITIAL_BALANCE),
				(processor_account_id(), INITIAL_BALANCE),
			],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>} = 0,
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Uniques: pallet_uniques::{Pallet, Storage, Event<T>, Call},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		AcurastProcessorManager: crate::{Pallet, Call, Storage, Event<T>},
	}
);

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Nonce = u64;
	type Hash = H256;
	type Block = Block<Test>;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for Test {
	/// The type for recording an account's balance.
	type Balance = Balance;
	type DustRemoval = ();
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = ();
	type FreezeIdentifier = ();
	type RuntimeFreezeReason = ();
	type MaxFreezes = ConstU32<0>;
}

impl pallet_uniques::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = u128;
	type ItemId = u128;
	type Currency = Balances;
	type ForceOrigin = EnsureRoot<Self::AccountId>;
	type CreateOrigin =
		AsEnsureOriginWithArg<EnsureRootWithSuccess<Self::AccountId, RootAccountId>>;
	type Locker = ();
	type CollectionDeposit = ConstU128<0>;
	type ItemDeposit = ConstU128<0>;
	type MetadataDepositBase = ConstU128<0>;
	type AttributeDepositBase = ConstU128<0>;
	type DepositPerByte = ConstU128<0>;
	type StringLimit = ConstU32<256>;
	type KeyLimit = ConstU32<256>;
	type ValueLimit = ConstU32<256>;
	type WeightInfo = pallet_uniques::weights::SubstrateWeight<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Proof = MultiSignature;
	type ManagerId = AssetId;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type ProcessorAssetRecovery = AcurastProcessorAssetRecovery;
	type MaxPairingUpdates = ConstU32<5>;
	type MaxProcessorsInSetUpdateInfo = ConstU32<100>;
	type Counter = u64;
	type PairingProofExpirationTime = ConstU128<600000>;
	type UnixTime = pallet_timestamp::Pallet<Test>;
	type Advertisement = ();
	type AdvertisementHandler = ();
	type WeightInfo = weights::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
	type Balance = Balance;
	type ProcessorRewardDistributor = ();
}

#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper<Test> for () {
	fn dummy_proof() -> <Test as Config>::Proof {
		MultiSignature::Sr25519(sp_core::sr25519::Signature::unchecked_from([0u8; 64]))
	}

	fn advertisement() -> <Test as Config>::Advertisement {}

	fn funded_account(index: u32) -> <Test as Config>::AccountId {
		let caller: T::AccountId = frame_benchmarking::account("token_account", index, SEED);
		<Balances as fungible::Mutate<_>>::set_balance(&caller.clone().into(), u32::MAX.into());

		caller
	}

	fn attest_account(account: &<Test>::AccountId) {}
}

pub struct AcurastManagerIdProvider;
impl ManagerIdProvider<Test> for AcurastManagerIdProvider {
	fn create_manager_id(
		id: <Test as Config>::ManagerId,
		owner: &<Test as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		if Uniques::collection_owner(0).is_none() {
			Uniques::create_collection(&0, &alice_account_id(), &alice_account_id())?;
		}
		Uniques::do_mint(0, id, owner.clone(), |_| Ok(()))
	}

	fn manager_id_for(
		owner: &<Test as frame_system::Config>::AccountId,
	) -> Result<<Test as Config>::ManagerId, frame_support::sp_runtime::DispatchError> {
		Uniques::owned_in_collection(&0, owner)
			.nth(0)
			.ok_or(frame_support::pallet_prelude::DispatchError::Other("Manager ID not found"))
	}

	fn owner_for(
		manager_id: <Test as Config>::ManagerId,
	) -> Result<<Test as frame_system::Config>::AccountId, frame_support::sp_runtime::DispatchError>
	{
		Uniques::owner(0, manager_id).ok_or(frame_support::pallet_prelude::DispatchError::Other(
			"Onwer for provided Manager ID not found",
		))
	}
}

pub struct AcurastProcessorAssetRecovery;
impl ProcessorAssetRecovery<Test> for AcurastProcessorAssetRecovery {
	fn recover_assets(
		processor: &<Test as frame_system::Config>::AccountId,
		destination_account: &<Test as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		let usable_balance = <Balances as Inspect<_>>::reducible_balance(
			processor,
			Preservation::Preserve,
			Fortitude::Polite,
		);
		if usable_balance > 0 {
			let burned = <Balances as Mutate<_>>::burn_from(
				processor,
				usable_balance,
				Preservation::Preserve,
				Precision::BestEffort,
				Fortitude::Polite,
			)?;
			Balances::mint_into(destination_account, burned)?;
		}
		Ok(())
	}
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}
