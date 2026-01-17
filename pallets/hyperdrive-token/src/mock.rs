#![allow(unused_imports)]

use crate::stub::*;
use crate::{stub::AcurastAccountId, weights};
use cumulus_primitives_core::ParaId;
use derive_more::{Display, From, Into};
use frame_support::{
	derive_impl,
	pallet_prelude::*,
	parameter_types,
	traits::{ConstU16, ConstU64},
	Deserialize, PalletId, Serialize,
};
use frame_system::{self as system, EnsureRoot};
use hex_literal::hex;
use pallet_acurast::{MessageBody, MessageProcessor, ProxyAcurastChain, ProxyChain, CU32};
use pallet_acurast_hyperdrive_ibc::{HoldReason, LayerFor, SubjectFor};
use sp_core::*;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup, Keccak256},
	AccountId32, BuildStorage, MultiSignature,
};
use sp_std::prelude::*;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		AcurastHyperdriveIbc: pallet_acurast_hyperdrive_ibc::{Pallet, Call, Storage, Event<T>},
		AcurastHyperdriveToken: crate::{Pallet, Call, Storage, Event<T>},
	}
);

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId32;
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

parameter_types! {
	pub const MinTTL: BlockNumber = 15;
	pub const IncomingTTL: BlockNumber = 50;
	pub const MinDeliveryConfirmationSignatures: u32 = 1;
	pub const MinReceiptConfirmationSignatures: u32 = 1;
	pub const MinFee: Balance = UNIT / 10;
	pub const ParachainId: ParaId = ParaId::new(2000);
	pub const SelfChain: ProxyAcurastChain = ProxyAcurastChain::Acurast;
}

impl pallet_acurast_hyperdrive_ibc::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MinTTL = MinTTL;
	type IncomingTTL = IncomingTTL;
	type MinDeliveryConfirmationSignatures = MinDeliveryConfirmationSignatures;
	type MinReceiptConfirmationSignatures = MinReceiptConfirmationSignatures;
	type MinFee = MinFee;
	type Currency = Balances;
	type RuntimeHoldReason = HoldReason;
	type MessageIdHashing = BlakeTwo256;
	type MessageProcessor = HyperdriveMessageProcessor;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type ParachainId = ParachainId;
	type SelfChain = SelfChain;
	type WeightInfo = pallet_acurast_hyperdrive_ibc::weights::WeightInfo<Test>;
}

parameter_types! {
	pub HyperdriveTokenPalletAccount: AccountId = PalletId(*b"hyptoken").into_account_truncating();
	pub HyperdriveTokenEthereumVault: AccountId = PalletId(*b"hyptveth").into_account_truncating();
	pub HyperdriveTokenEthereumFeeVault: AccountId = PalletId(*b"hyptfeth").into_account_truncating();
	pub HyperdriveTokenSolanaVault: AccountId = PalletId(*b"hyptvsol").into_account_truncating();
	pub HyperdriveTokenSolanaFeeVault: AccountId = PalletId(*b"hyptfsol").into_account_truncating();
	pub HyperdriveTokenOperationalFeeAccount: AccountId = PalletId(*b"hyptofac").into_account_truncating();
	pub OutgoingTransferTTL: BlockNumber = 15;
	pub const MinTransferAmount: Balance = UNIT;
}

impl crate::Config for Test {
	type RuntimeEvent = RuntimeEvent;

	type PalletAccount = HyperdriveTokenPalletAccount;
	type ParsableAccountId = AcurastAccountId;
	type Balance = Balance;
	type Currency = Balances;
	type MessageSender = AcurastHyperdriveIbc;
	type MessageIdHasher = BlakeTwo256;

	type EthereumVault = HyperdriveTokenEthereumVault;
	type EthereumFeeVault = HyperdriveTokenEthereumFeeVault;
	type SolanaVault = HyperdriveTokenSolanaVault;
	type SolanaFeeVault = HyperdriveTokenSolanaFeeVault;
	type DefaultOutgoingTransferTTL = OutgoingTransferTTL;
	type OperationalFeeAccount = HyperdriveTokenOperationalFeeAccount;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type OperatorOrigin = EnsureRoot<Self::AccountId>;
	type MinTransferAmount = MinTransferAmount;

	type WeightInfo = weights::WeightInfo<Test>;
}

/// Controls routing for incoming HyperdriveIBC messages.
///
/// Forwards messages with
/// * recipient [`HyperdriveTokenPalletAccount`] to AcurastHyperdriveToken pallet.
pub struct HyperdriveMessageProcessor;
impl MessageProcessor<AccountId, AccountId> for HyperdriveMessageProcessor {
	fn process(message: impl MessageBody<AccountId, AccountId>) -> DispatchResultWithPostInfo {
		if &SubjectFor::<Test>::Acurast(LayerFor::<Test>::Extrinsic(
			HyperdriveTokenPalletAccount::get(),
		)) == message.recipient()
		{
			AcurastHyperdriveToken::process(message)
		} else {
			// TODO fail this?
			Ok(().into())
		}
	}
}

parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
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
	type FreezeIdentifier = ();
	type RuntimeHoldReason = HoldReason;
	type RuntimeFreezeReason = ();
	type MaxFreezes = ConstU32<0>;
	type DoneSlashHandler = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = system::GenesisConfig::<Test>::default().build_storage().unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

// pub fn events() -> Vec<RuntimeEvent> {
// 	log::debug!("{:#?}", System::events());
// 	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

// 	System::reset_events();

// 	evt
// }
