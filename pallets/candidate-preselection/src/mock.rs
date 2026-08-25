use core::marker::PhantomData;

use frame_support::{
	derive_impl, parameter_types,
	traits::{ConstU16, ConstU32, ConstU64, ValidatorRegistration},
	PalletId,
};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::{
	testing::UintAuthorityId,
	traits::{IdentityLookup, OpaqueKeys},
	AccountId32, BuildStorage, KeyTypeId, RuntimeAppPublic,
};

use crate::*;

pub type AccountId = AccountId32;
type Block = frame_system::mocking::MockBlock<Test>;

pub fn account(id: u8) -> AccountId {
	[id; 32].into()
}

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let parachain_info_config =
			parachain_info::GenesisConfig { parachain_id: 2000.into(), ..Default::default() };

		<parachain_info::GenesisConfig<Test> as BuildStorage>::assimilate_storage(
			&parachain_info_config,
			&mut t,
		)
		.unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: vec![(account(1), 100), (account(2), 100)],
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		// collator selection must be initialized before session.
		pallet_collator_selection::GenesisConfig::<Test> {
			invulnerables: vec![],
			candidacy_bond: 10,
			desired_candidates: 2,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_session::GenesisConfig::<Test> { keys: vec![], ..Default::default() }
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
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		ParachainInfo: parachain_info::{Pallet, Storage, Config<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Session: pallet_session::{Pallet, Call, Storage, Config<T>, Event<T>, HoldReason},
		CollatorSelection: pallet_collator_selection::{Pallet, Call, Storage, Config<T>, Event<T>},
		CandidatePreselection: crate::{Pallet, Call, Storage, Event<T>}
	}
);

parameter_types! {
	pub const MinimumPeriod: u64 = 6000;
}

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Nonce = u64;
	type Hash = H256;
	type Block = Block;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type AccountData = pallet_balances::AccountData<u64>;
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

impl parachain_info::Config for Test {}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type AccountStore = System;
}

sp_runtime::impl_opaque_keys! {
	pub struct MockSessionKeys {
		pub aura: UintAuthorityId,
	}
}

pub struct TestSessionHandler;
impl pallet_session::SessionHandler<AccountId> for TestSessionHandler {
	const KEY_TYPE_IDS: &'static [KeyTypeId] = &[UintAuthorityId::ID];

	fn on_genesis_session<Ks: OpaqueKeys>(_keys: &[(AccountId, Ks)]) {}
	fn on_new_session<Ks: OpaqueKeys>(
		_changed: bool,
		_keys: &[(AccountId, Ks)],
		_queued_keys: &[(AccountId, Ks)],
	) {
	}
	fn on_before_session_ending() {}
	fn on_disabled(_: u32) {}
}

parameter_types! {
	pub const Period: u64 = 10;
	pub const Offset: u64 = 0;
}

impl pallet_session::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = PreselectionSessionManager<Test, CollatorSelection>;
	type SessionHandler = TestSessionHandler;
	type Keys = MockSessionKeys;
	type DisablingStrategy = ();
	type WeightInfo = ();
	type Currency = Balances;
	type KeyDeposit = ();
}

parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
	// large enough so that stale-candidate kicking never triggers in tests
	pub const KickThreshold: u64 = 1000;
}

impl pallet_collator_selection::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type UpdateOrigin = EnsureRoot<AccountId>;
	type PotId = PotId;
	type MaxCandidates = ConstU32<20>;
	type MinEligibleCollators = ConstU32<1>;
	type MaxInvulnerables = ConstU32<20>;
	type KickThreshold = KickThreshold;
	type ValidatorId = AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = CandidatePreselection;
	type WeightInfo = ();
}

impl crate::Config for Test {
	type ValidatorId = AccountId;
	type ValidatorRegistration = ValReg<Self>;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

pub struct ValReg<T: Config>(PhantomData<T>);
impl<T: Config> ValidatorRegistration<T::ValidatorId> for ValReg<T> {
	fn is_registered(_id: &T::ValidatorId) -> bool {
		true
	}
}

pub fn initialize_to_block(n: u64) {
	for i in System::block_number() + 1..=n {
		System::set_block_number(i);
		<AllPalletsWithSystem as frame_support::traits::OnInitialize<u64>>::on_initialize(i);
	}
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}
