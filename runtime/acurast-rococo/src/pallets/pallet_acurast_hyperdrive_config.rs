use frame_support::{
	instances::Instance1, pallet_prelude::DispatchResultWithPostInfo, parameter_types,
};
use polkadot_core_primitives::BlakeTwo256;

use acurast_runtime_common::{
	constants::UNIT,
	types::{AccountId, Balance},
	weight,
};
use pallet_acurast::{MessageBody, MessageProcessor, ProxyAcurastChain};
use pallet_acurast_hyperdrive_ibc::{LayerFor, SubjectFor};

use crate::{
	AcurastAccountId, AcurastHyperdriveIbc, AcurastHyperdriveToken, Balances, EnsureCouncilOrRoot,
	HyperdriveTokenEthereumFeeVault, HyperdriveTokenEthereumVault, HyperdriveTokenPalletAccount,
	HyperdriveTokenSolanaFeeVault, HyperdriveTokenSolanaVault, IncomingTTL,
	MinDeliveryConfirmationSignatures, MinReceiptConfirmationSignatures, MinTTL,
	OperationalFeeAccount, OutgoingTransferTTL, ParachainInfo, Runtime, RuntimeHoldReason,
};

parameter_types! {
	pub const MinFee: Balance = UNIT / 10;
	pub const MinTransferAmount: Balance = UNIT;
	pub const SelfChain: ProxyAcurastChain = ProxyAcurastChain::AcurastCanary;
}

impl pallet_acurast_hyperdrive_ibc::Config<Instance1> for Runtime {
	type MinTTL = MinTTL;
	type IncomingTTL = IncomingTTL;
	type MinDeliveryConfirmationSignatures = MinDeliveryConfirmationSignatures;
	type MinReceiptConfirmationSignatures = MinReceiptConfirmationSignatures;
	type MinFee = MinFee;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MessageIdHashing = BlakeTwo256;
	type MessageProcessor = HyperdriveMessageProcessor;
	type UpdateOrigin = EnsureCouncilOrRoot;
	type ParachainId = ParachainInfo;
	type SelfChain = SelfChain;
	type WeightInfo = weight::pallet_acurast_hyperdrive_ibc::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = IbcBenchmarkHelper;
}

impl pallet_acurast_hyperdrive_token::Config<Instance1> for Runtime {
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
	type OperationalFeeAccount = OperationalFeeAccount;
	type DefaultOutgoingTransferTTL = OutgoingTransferTTL;
	type UpdateOrigin = EnsureCouncilOrRoot;
	type OperatorOrigin = EnsureCouncilOrRoot;
	type MinTransferAmount = MinTransferAmount;

	type WeightInfo = weight::pallet_acurast_hyperdrive_token::WeightInfo<Runtime>;
}

/// The [`HyperdriveTokenPalletAccount`] as recipient subject on [`SelfChain`].
fn token_pallet_subject() -> SubjectFor<Runtime> {
	let layer = LayerFor::<Runtime>::Extrinsic(HyperdriveTokenPalletAccount::get());
	match SelfChain::get() {
		ProxyAcurastChain::Acurast => SubjectFor::<Runtime>::Acurast(layer),
		ProxyAcurastChain::AcurastCanary => SubjectFor::<Runtime>::AcurastCanary(layer),
	}
}

/// Controls routing for incoming HyperdriveIBC messages.
///
/// Forwards messages with
/// * recipient [`HyperdriveTokenPalletAccount`] to AcurastHyperdriveToken pallet.
pub struct HyperdriveMessageProcessor;
impl MessageProcessor<AccountId, AccountId> for HyperdriveMessageProcessor {
	fn process(message: impl MessageBody<AccountId, AccountId>) -> DispatchResultWithPostInfo {
		if &token_pallet_subject() == message.recipient() {
			AcurastHyperdriveToken::process(message)
		} else {
			// Unknown recipient (e.g. removed job-operation route): no-op.
			Ok(().into())
		}
	}
}

/// Sets `receive_message` up to take the token route, the only one [`HyperdriveMessageProcessor`]
/// does any work for: an Ethereum transfer paid out of the vault to a new account.
#[cfg(feature = "runtime-benchmarks")]
pub struct IbcBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_acurast_hyperdrive_ibc::BenchmarkHelper<Runtime, Instance1> for IbcBenchmarkHelper {
	fn worst_case_incoming_message(
	) -> Option<(SubjectFor<Runtime>, SubjectFor<Runtime>, sp_std::vec::Vec<u8>)> {
		use frame_support::{assert_ok, traits::fungible::Mutate};
		use pallet_acurast::{AccountId20, ContractCall, Layer};

		let contract = AccountId20([1; 20]);
		pallet_acurast_hyperdrive_token::EthereumContract::<Runtime, Instance1>::put(contract);

		let amount = MinTransferAmount::get();
		// twice the amount, so the vault keeps its existential deposit
		assert_ok!(Balances::mint_into(&HyperdriveTokenEthereumVault::get(), 2 * amount));

		// `TransferToken` (action 0) of `amount` of the native asset (0), transfer nonce 0, to `dest`.
		let dest: AccountId = frame_benchmarking::account("bridge_dest", 0, 0);
		let mut payload = [0u8; 64];
		payload[4..20].copy_from_slice(&amount.to_be_bytes());
		payload[32..64].copy_from_slice(AsRef::<[u8; 32]>::as_ref(&dest));

		Some((
			SubjectFor::<Runtime>::Ethereum(Layer::Contract(ContractCall {
				contract,
				selector: None,
			})),
			token_pallet_subject(),
			payload.to_vec(),
		))
	}
}
