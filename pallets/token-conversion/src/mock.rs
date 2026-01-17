use frame_support::{
	derive_impl, parameter_types,
	traits::{tokens::imbalance::ResolveTo, ConstU16, ConstU32, ConstU64},
	PalletId,
};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::{
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
	AccountId32, BuildStorage,
};

use acurast_common::{Layer, Subject};

use crate::SubjectFor;

pub type AccountId = AccountId32;
type Block = frame_system::mocking::MockBlock<Test>;
pub type Balance = u128;
pub type BlockNumber = u32;

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;
pub const UNIT: Balance = 1_000_000;
pub const MILLIUNIT: Balance = UNIT / 1_000;

pub const MILLISECS_PER_BLOCK: u64 = 6000;

pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>} = 0,
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		AcurastTokenConversion: crate::{Pallet, Call, Storage, Event<T>, HoldReason}
	}
);

parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	pub const MinimumPeriod: u64 = 6000;
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const MaxReserves: u32 = 50;
	pub const MaxLocks: u32 = 50;
	pub const ConversionPallelId: PalletId = PalletId(*b"cnvrspid");
	pub ConversionPalletAccountId: AccountId = ConversionPallelId::get().into_account_truncating();
	pub const TokenConversionPalletId: PalletId = PalletId(*b"cvrsnpid");
	pub SendTo: Option<SubjectFor<Test>> = Some(Subject::Acurast(Layer::Extrinsic(TokenConversionPalletId::get().into_account_truncating())));
	pub ReceiveFrom: Option<SubjectFor<Test>> = Some(Subject::Acurast(Layer::Extrinsic(TokenConversionPalletId::get().into_account_truncating())));
	pub const Liquidity: Balance = UNIT / 100;
	pub const MinLockDuration: BlockNumber = 3 * 28 * DAYS;
	pub const MaxLockDuration: BlockNumber = 48 * 28 * DAYS;
	pub OutgoingTransferTTL: BlockNumber = 15;
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
	type AccountData = pallet_balances::AccountData<Balance>;
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
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxFreezes = ConstU32<50>;
	type DoneSlashHandler = ();
}

impl crate::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = ConversionPallelId;
	type SendTo = SendTo;
	type ReceiveFrom = ReceiveFrom;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type Liquidity = Liquidity;
	type MinLockDuration = MinLockDuration;
	type MaxLockDuration = MaxLockDuration;
	type MessageSender = ();
	type MessageIdHasher = BlakeTwo256;
	type OnSlash = ResolveTo<ConversionPalletAccountId, Balances>;
	type ConvertTTL = OutgoingTransferTTL;
	type EnableOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = crate::weights::WeightInfo<Self>;
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}
