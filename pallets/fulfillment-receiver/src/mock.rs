use crate::{
	stub::{bob_account_id, AccountId},
	traits::OnFulfillment,
	types::Fulfillment,
};
use acurast_common::Script;
use frame_support::{
	derive_impl, parameter_types,
	sp_runtime::{
		self,
		traits::{ConstU16, ConstU32, ConstU64, IdentityLookup},
		BuildStorage, DispatchError,
	},
	PalletId,
};
use hex_literal::hex;
use sp_core::H256;

pub type BlockNumber = u32;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>} = 0,
		AcurastFulfillmentReceiver: crate::{Pallet, Call, Event<T>}
	}
);

parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
}
parameter_types! {
	pub const MinimumPeriod: u64 = 6000;
	pub AllowedFulfillAccounts: Vec<AccountId> = vec![bob_account_id()];
}
parameter_types! {
	pub const MaxReserves: u32 = 50;
	pub const MaxLocks: u32 = 50;
}
parameter_types! {
	pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
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

impl crate::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type OnFulfillment = FulfillmentHandler;
	type WeightInfo = ();
}

pub struct FulfillmentHandler;
impl OnFulfillment<Test> for FulfillmentHandler {
	fn on_fulfillment(
		from: <Test as frame_system::Config>::AccountId,
		_fulfillment: Fulfillment,
	) -> sp_runtime::DispatchResultWithInfo<frame_support::dispatch::PostDispatchInfo> {
		if !AllowedFulfillAccounts::get().contains(&from) {
			return Err(DispatchError::BadOrigin.into());
		}
		Ok(().into())
	}
}

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

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

pub const SCRIPT_BYTES: [u8; 53] = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

pub fn script() -> Script {
	SCRIPT_BYTES.to_vec().try_into().unwrap()
}

pub fn fulfillment_for(script: Script) -> Fulfillment {
	Fulfillment { script, payload: hex!("00").to_vec() }
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}
