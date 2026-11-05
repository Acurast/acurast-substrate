use cumulus_primitives_core::ParaId;
use frame_support::{derive_impl, pallet_prelude::*, parameter_types, traits::ConstU128};
use frame_system::EnsureRoot;
use pallet_acurast::{MessageBody, MessageProcessor, ProxyAcurastChain};
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	AccountId32, BuildStorage,
};

// u32 block numbers, as required by the benchmarks
type Block = frame_system::mocking::MockBlockU32<Test>;
pub type AccountId = AccountId32;
pub type Balance = u128;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		AcurastHyperdriveIbc: crate,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type Balance = Balance;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
}

parameter_types! {
	pub const MinTTL: u32 = 15;
	pub const IncomingTTL: u32 = 50;
	pub const MinDeliveryConfirmationSignatures: u32 = 1;
	pub const MinReceiptConfirmationSignatures: u32 = 1;
	pub const MinFee: Balance = 1;
	pub const ParachainId: ParaId = ParaId::new(2000);
	pub const SelfChain: ProxyAcurastChain = ProxyAcurastChain::Acurast;
}

/// Accepts every message without side effects.
pub struct NoopMessageProcessor;
impl MessageProcessor<AccountId, AccountId> for NoopMessageProcessor {
	fn process(_message: impl MessageBody<AccountId, AccountId>) -> DispatchResultWithPostInfo {
		Ok(().into())
	}
}

impl crate::Config for Test {
	type MinTTL = MinTTL;
	type IncomingTTL = IncomingTTL;
	type MinDeliveryConfirmationSignatures = MinDeliveryConfirmationSignatures;
	type MinReceiptConfirmationSignatures = MinReceiptConfirmationSignatures;
	type MinFee = MinFee;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MessageIdHashing = BlakeTwo256;
	type MessageProcessor = NoopMessageProcessor;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type ParachainId = ParachainId;
	type SelfChain = SelfChain;
	type WeightInfo = crate::weights::WeightInfo<Test>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
