use crate as fee_manager;
use frame_support::{
	derive_impl, parameter_types,
	traits::{ConstU16, ConstU64},
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{traits::IdentityLookup, BuildStorage};
use system::EnsureRoot;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		FeeManager: fee_manager,
	}
);

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = u64;
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
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const DefaultFeePercentage: sp_arithmetic::Percent = sp_arithmetic::Percent::from_percent(30);
}

impl fee_manager::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type DefaultFeePercentage = DefaultFeePercentage;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;

	type WeightInfo = crate::weights::WeightInfo<Self>;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}
