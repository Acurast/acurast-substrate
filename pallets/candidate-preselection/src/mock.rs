use core::marker::PhantomData;

use frame_support::{
	derive_impl, parameter_types,
	traits::{ConstU16, ConstU32, ConstU64, ValidatorRegistration},
};
use sp_core::H256;
use sp_io;
use sp_runtime::{traits::IdentityLookup, AccountId32, BuildStorage};

use crate::*;

pub type AccountId = AccountId32;
type Block = frame_system::mocking::MockBlock<Test>;

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

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {}
	}
}

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>} = 0,
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		ParachainInfo: parachain_info::{Pallet, Storage, Config<T>},
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
	type AccountData = ();
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

impl crate::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = AccountId;
	type ValidatorRegistration = ValReg<Self>;
	type WeightInfo = ();
}

pub struct ValReg<T: Config>(PhantomData<T>);
impl<T: Config> ValidatorRegistration<T::ValidatorId> for ValReg<T> {
	fn is_registered(_id: &T::ValidatorId) -> bool {
		true
	}
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}
