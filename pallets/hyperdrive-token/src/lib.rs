#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

pub use pallet::*;
pub use traits::*;
pub use types::*;

#[cfg(test)]
mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod traits;

pub mod chain;

mod types;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
	use chain::{
		ethereum::{EthereumActionDecoder, EthereumActionEncoder},
		ActionDecoderError, ActionEncoderError,
	};
	use frame_support::{
		pallet_prelude::{StorageDoubleMap, *},
		sp_runtime::{traits::AtLeast32BitUnsigned, SaturatedConversion},
		traits::{
			tokens::{
				fungible::{self, Mutate},
				Preservation,
			},
			Get,
		},
		Identity,
	};
	use frame_system::pallet_prelude::*;
	use pallet_acurast::{AccountId20, MultiOrigin, ProxyChain};
	use pallet_acurast_hyperdrive_ibc::{
		BalanceOf, ContractCall, Layer, MessageBody, Subject, SubjectFor,
	};
	use sp_core::crypto::AccountId32;
	use sp_runtime::traits::{Hash, Zero};
	use sp_std::{prelude::*, vec};

	use super::*;

	/// A instantiable pallet for receiving secure state synchronizations into Acurast.
	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	/// Configures the pallet.
	#[pallet::config]
	pub trait Config<I: 'static = ()>:
		frame_system::Config + pallet_acurast_hyperdrive_ibc::Config<I>
	{
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type PalletAccount: Get<Self::AccountId>;
		type ParsableAccountId: Into<Self::AccountId> + TryFrom<Vec<u8>>;
		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ From<u128>
			+ Into<u128>
			+ Zero
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo
			+ Into<BalanceOf<Self, I>>;

		#[pallet::constant]
		type EthereumVault: Get<Self::AccountId>;
		#[pallet::constant]
		type EthereumFeeVault: Get<Self::AccountId>;
		#[pallet::constant]
		type SolanaVault: Get<Self::AccountId>;
		#[pallet::constant]
		type SolanaFeeVault: Get<Self::AccountId>;

		#[pallet::constant]
		type OutgoingTransferTTL: Get<BlockNumberFor<Self>>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		TransferToProxy {
			source: T::AccountId,
			dest: MultiOrigin<T::AccountId>,
			amount: T::Balance,
		},
		TransferFromProxy {
			source: ProxyChain,
			dest: T::AccountId,
			amount: T::Balance,
		},
		EthereumContractUpdated {
			contract: AccountId20,
		},
		SolanaContractUpdated {
			contract: AccountId32,
		},
		PalletEnabled {
			enabled: bool,
		},
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Nested Hyperdrive (IBC) error.
		PalletHyperdriveIBC(pallet_acurast_hyperdrive_ibc::Error<T, I>),
		EthereumActionDecoderError(u8),
		EthereumMessageEncoderError(u8),
		InvalidSender,
		InvalidRecipient,
		UnsupportedAssetId,
		TransferAlreadyReceived,
		TransferToVaultFailed,
		TransferToFeeVaultFailed,
		UnknownTransferRetry,
		InvalidTransferRetry,
		MissingContractConfiguration,
		NotEnabled,
	}

	impl<T: Config<I>, I: 'static> From<pallet_acurast_hyperdrive_ibc::Error<T, I>> for Error<T, I> {
		fn from(e: pallet_acurast_hyperdrive_ibc::Error<T, I>) -> Self {
			Error::<T, I>::PalletHyperdriveIBC(e)
		}
	}

	impl<T: Config<I>, I: 'static> From<ActionDecoderError> for Error<T, I> {
		fn from(e: ActionDecoderError) -> Self {
			Error::<T, I>::EthereumActionDecoderError(e as u8)
		}
	}

	impl<T: Config<I>, I: 'static> From<ActionEncoderError> for Error<T, I> {
		fn from(e: ActionEncoderError) -> Self {
			Error::<T, I>::EthereumMessageEncoderError(e as u8)
		}
	}

	#[pallet::storage]
	#[pallet::getter(fn enabled)]
	pub type Enabled<T: Config<I>, I: 'static = ()> = StorageValue<_, bool, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn ethereum_contract)]
	pub type EthereumContract<T: Config<I>, I: 'static = ()> =
		StorageValue<_, AccountId20, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn solana_contract)]
	pub type SolanaContract<T: Config<I>, I: 'static = ()> =
		StorageValue<_, AccountId32, OptionQuery>;

	/// Transfer nonces seen in processed incoming messages from proxies, uniquely identifying every transfer made _to_ Acurast.
	///
	/// The nonce orders all transfers from **just** ethereum proxy to this pallet. This fact they are ordered and sequential if all transfers are relayed is currently not used,
	/// but it could be used to optimize the storage required for detecting duplicate transfers.
	#[pallet::storage]
	#[pallet::getter(fn outgoing_transfers)]
	pub type OutgoingTransfers<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Identity,
		ProxyChain,
		Identity,
		TransferNonce,
		(T::AccountId, MultiOrigin<T::AccountId>, T::Balance),
	>;

	/// Next nonce per proxy. The nonce sequentializes all transfers _to_ a proxy, separate for each proxy.
	///
	/// The latest used nonce is the stored value - 1.
	///
	/// This fact they are ordered and sequential if all transfers are relayed is currently not used,
	/// but it could be used to optimize the storage required for detecting duplicate transfers.
	#[pallet::storage]
	#[pallet::getter(fn next_transfer_nonce)]
	pub type NextTransferNonce<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, ProxyChain, TransferNonce, ValueQuery>;

	/// Transfer nonces seen in processed incoming messages from proxies, uniquely identifying every transfer made _to_ Acurast.
	///
	/// The nonce orders all transfers from **just** ethereum proxy to this pallet. This fact they are ordered and sequential if all transfers are relayed is currently not used,
	/// but it could be used to optimize the storage required for detecting duplicate transfers.
	#[pallet::storage]
	#[pallet::getter(fn incoming_transfer_nonces)]
	pub type IncomingTransferNonces<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Identity, ProxyChain, Identity, TransferNonce, ()>;

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Transfers tokens over Hyperdrive (IBC) to the proxy on recipient chain.
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config<I>>::WeightInfo::transfer_native())]
		pub fn transfer_native(
			origin: OriginFor<T>,
			dest: MultiOrigin<T::AccountId>,
			amount: T::Balance,
			fee: T::Balance,
		) -> DispatchResult {
			Self::ensure_enabled()?;
			let source = ensure_signed(origin)?;
			let proxy: ProxyChain = (&dest).into();
			let transfer_nonce =
				Self::do_transfer_native(source.clone(), dest.clone(), amount, fee, None)?;
			OutgoingTransfers::<T, I>::insert(proxy, transfer_nonce, (source, dest, amount));
			Ok(())
		}

		/// Retransfers tokens over Hyperdrive (IBC) to the proxy on recipient chain.
		///
		/// * Cannot change the amount nor recipient since the transfer might be processed already.
		/// * May specify a different `fee` than the original transfer and always restarts the `ttl` of the Hyperdrive (IBC) message.
		#[pallet::call_index(1)]
		#[pallet::weight(< T as Config<I>>::WeightInfo::retry_transfer_native())]
		pub fn retry_transfer_native(
			origin: OriginFor<T>,
			proxy: ProxyChain,
			transfer_nonce: TransferNonce,
			fee: T::Balance,
		) -> DispatchResult {
			Self::ensure_enabled()?;
			let source = ensure_signed(origin)?;
			let (prev_source, prev_recipient, prev_amount) =
				OutgoingTransfers::<T, I>::get(proxy, transfer_nonce)
					.ok_or(Error::<T, I>::UnknownTransferRetry)?;
			if prev_source != source {
				Err(Error::<T, I>::InvalidTransferRetry)?;
			}
			let _ = Self::do_transfer_native(
				source,
				prev_recipient,
				prev_amount,
				fee,
				Some(transfer_nonce),
			)?;
			// no need to reinsert into OutgoingTransfers since no stored properties changed
			Ok(())
		}

		/// Updates the Ethereum (target chain) contract address in storage. Can only be called by a privileged/root account.
		#[pallet::call_index(2)]
		#[pallet::weight(< T as Config<I>>::WeightInfo::update_ethereum_contract())]
		pub fn update_ethereum_contract(
			origin: OriginFor<T>,
			contract: AccountId20,
		) -> DispatchResult {
			ensure_root(origin)?;
			<EthereumContract<T, I>>::set(Some(contract.clone()));
			Self::deposit_event(Event::EthereumContractUpdated { contract });
			Ok(())
		}

		/// Updates the Vara (target chain) contract address in storage. Can only be called by a privileged/root account.
		#[pallet::call_index(3)]
		#[pallet::weight(< T as Config<I>>::WeightInfo::update_solana_contract())]
		pub fn update_solana_contract(
			origin: OriginFor<T>,
			contract: AccountId32,
		) -> DispatchResult {
			ensure_root(origin)?;
			<SolanaContract<T, I>>::set(Some(contract.clone()));
			Self::deposit_event(Event::SolanaContractUpdated { contract });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config<I>>::WeightInfo::set_enabled())]
		pub fn set_enabled(origin: OriginFor<T>, enabled: bool) -> DispatchResult {
			ensure_root(origin)?;
			<Enabled<T, I>>::set(Some(enabled));
			Self::deposit_event(Event::PalletEnabled { enabled });
			Ok(())
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I>
	where
		T: pallet_acurast_hyperdrive_ibc::Config<I>,
	{
		fn proxy_params(
			proxy: &ProxyChain,
		) -> Result<(SubjectFor<T>, T::AccountId, T::AccountId), Error<T, I>> {
			match proxy {
				ProxyChain::Ethereum => Ok((
					Subject::Ethereum(Layer::Contract(ContractCall {
						contract: Self::ethereum_contract()
							.ok_or(Error::<T, I>::MissingContractConfiguration)?,
						selector: None,
					})),
					T::EthereumVault::get(),
					T::EthereumFeeVault::get(),
				)),
				_ => Err(Error::InvalidRecipient),
			}
		}

		fn ensure_enabled() -> Result<(), Error<T, I>> {
			let enabled = Self::enabled().unwrap_or_default();
			if !enabled {
				return Err(Error::<T, I>::NotEnabled);
			}
			Ok(())
		}

		/// Sends a message with a [`Action::TransferToken`] over Hyperdrive.
		///
		/// NOTE: the account triggering this message is the payer account, which is getting charged for the amount and the fee; however the sender of the message is a constant pallet account [`T::PalletAccount`]).
		pub fn do_transfer_native(
			source: T::AccountId,
			dest: MultiOrigin<T::AccountId>,
			amount: T::Balance,
			fee: T::Balance,
			// if provided, this is a transfer retry
			transfer_nonce: Option<TransferNonce>,
		) -> Result<TransferNonce, Error<T, I>> {
			let proxy: ProxyChain = (&dest).into();
			// recipient is the message recipient, not the recipient of amount which is `dest`
			let (recipient, vault, fee_vault) = Self::proxy_params(&proxy)?;

			if transfer_nonce.is_none() {
				// since this is a new transfer, we lock the amount
				// but not if this is a retry (however we lock the fee for both new transfers and retries below)
				if !amount.is_zero() {
					T::Currency::transfer(
						&source,
						&vault,
						amount
							.saturated_into::<<T::Currency as fungible::Inspect<T::AccountId>>::Balance>(
							),
						Preservation::Preserve,
					)
					.map_err(|e| {
						log::error!(
							target: "runtime::acurast_hyperdrive_token",
							"error in do_transfer_native; transfer {:?} to vault: {:?}",
							amount,
							e,
						);
						Error::<T, I>::TransferToVaultFailed
					})?;
				}
			}

			let transfer_nonce = transfer_nonce.unwrap_or_else(|| {
				let n = Self::next_transfer_nonce(proxy);
				NextTransferNonce::<T, I>::insert(proxy, n + 1);
				n
			});

			// we lock the fee for both new transfers and retries below;
			// lock amount + fee on their respective vault accounts (derived from configured vault IDs)
			if !fee.is_zero() {
				// hyperdrive-ibc reserves the fee on the payer's account, but we want the reserves being on a central per-proxy pallet account,
				// since these fees can never be recovered to the source of a transfer (can only be retried with usually higher fee/ttl)
				T::Currency::transfer(
                &source,
                    &fee_vault,
                    fee.saturated_into::<<T::Currency as fungible::Inspect<T::AccountId>>::Balance>(),
                    Preservation::Preserve,
                ).map_err(|e| {
                    log::error!(
                        target: "runtime::acurast_hyperdrive_token",
                        "error in do_transfer_native; transfer to fee_vault: {:?}",
                        e,
                    );
                    Error::<T, I>::TransferToVaultFailed
                })?;
			}

			let action =
				Action::TransferToken(amount.into(), None, transfer_nonce as u32, dest.clone());
			let encoded = <EthereumActionEncoder as ActionEncoder<T::AccountId>>::encode(&action)?;

			let _message = pallet_acurast_hyperdrive_ibc::Pallet::<T, I>::do_send_message(
				Subject::Acurast(Layer::Extrinsic(T::PalletAccount::get())),
				// payer is the fee pallet account (see transfer above)
				&fee_vault.into(),
				T::MessageIdHashing::hash_of(&transfer_nonce),
				recipient,
				encoded,
				T::OutgoingTransferTTL::get(),
				fee.into(),
			)?;

			Self::deposit_event(Event::TransferToProxy { source, dest, amount });

			Ok(transfer_nonce)
		}

		/// Executes an parsed action from a _valid_ incoming Hyperdrive (IBC) message to this pallet.
		///
		/// NOTE: _valid_ means that the Hyperdrive (IBC) sender and recipient have already been validated in [`MessageProcessor::process`] implementation of this pallet.
		/// So we know the messages originates from the proxy contract counterpart to this pallet.
		fn execute(proxy: ProxyChain, action: Action<T::AccountId>) -> DispatchResultWithPostInfo {
			match action {
				Action::TransferToken(amount, asset_id, transfer_nonce, dest) => {
					if asset_id.is_some() {
						Err(Error::<T, I>::UnsupportedAssetId)?;
					}

					if Self::incoming_transfer_nonces(proxy, transfer_nonce).is_some() {
						Err(Error::<T, I>::TransferAlreadyReceived)?;
					}
					IncomingTransferNonces::<T, I>::insert(proxy, transfer_nonce, ());

					match dest {
						MultiOrigin::Acurast(dest_account_id) => {
							if !amount.is_zero() {
								T::Currency::transfer(
									&T::EthereumVault::get(),
									&dest_account_id,
									amount
										.saturated_into::<<T::Currency as fungible::Inspect<T::AccountId>>::Balance>(
										),
									Preservation::Protect,
								)
								.map_err(|e| {
									log::error!(
										target: "runtime::acurast_hyperdrive_token",
										"error in execute action; transfer to dest: {:?}",
										e,
									);
									Error::<T, I>::TransferToVaultFailed
								})?;
							}
							Self::deposit_event(Event::TransferFromProxy {
								source: proxy,
								dest: dest_account_id,
								amount: amount.into(),
							});
							Ok(().into())
						},
						_ => Err(Error::<T, I>::InvalidRecipient)?,
					}
				},
				Action::Noop => Ok(().into()),
			}
		}
	}

	impl<T: Config<I>, I: 'static>
		pallet_acurast_hyperdrive_ibc::MessageProcessor<T::AccountId, T::AccountId> for Pallet<T, I>
	{
		fn process(message: MessageBody<T::AccountId, T::AccountId>) -> DispatchResultWithPostInfo {
			let (proxy, action) = match message.sender {
				SubjectFor::<T>::Ethereum(Layer::Contract(contract_call)) => {
					if contract_call.contract
						!= Self::ethereum_contract()
							.ok_or(Error::<T, I>::MissingContractConfiguration)?
					{
						Err(Error::<T, I>::InvalidSender)?
					}
					let action = <EthereumActionDecoder<
					I,
					T::ParsableAccountId,
					T::AccountId,
				> as types::ActionDecoder<T::AccountId>>::decode(
					&message.payload
				)
				.map_err(|e| Error::<T, I>::EthereumActionDecoderError(e as u8))?;
					Ok((ProxyChain::Ethereum, action))
				},
				// TODO implement solana
				SubjectFor::<T>::Solana(Layer::Contract(contract_call)) => {
					if contract_call.contract
						!= Self::solana_contract()
							.ok_or(Error::<T, I>::MissingContractConfiguration)?
					{
						Err(Error::<T, I>::InvalidSender)?
					}
					// TODO complete implementation for Solana
					// Ok((ProxyChain::Solana, action))
					Err(Error::<T, I>::InvalidSender)?
				},
				_ => Err(Error::<T, I>::InvalidSender),
			}?;

			Self::execute(proxy, action)?;

			Ok(().into())
		}
	}
}
