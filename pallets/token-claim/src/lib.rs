#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

pub use pallet::*;
pub use traits::*;
pub use types::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod traits;

mod types;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
pub use benchmarking::BenchmarkHelper;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		dispatch::DispatchResult,
		pallet_prelude::*,
		traits::{Get, VestedTransfer},
		Blake2_128Concat,
	};
	use frame_system::pallet_prelude::{BlockNumberFor, *};
	use sp_runtime::traits::{Convert, IdentifyAccount, One, StaticLookup, Verify};
	use sp_std::{prelude::*, vec};

	use super::*;

	/// A instantiable pallet for receiving secure state synchronizations into Acurast.
	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	/// Configures the pallet.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type VestedTransferer: VestedTransfer<Self::AccountId, Moment = BlockNumberFor<Self>>;
		type Signature: Parameter + Member + Verify + MaxEncodedLen;
		type Signer: Get<Self::AccountId>;
		type Funder: Get<Self::AccountId>;
		type VestingDuration: Get<BlockNumberFor<Self>>;
		type BlockNumberToBalance: Convert<BlockNumberFor<Self>, BalanceFor<Self>>;
		type WeightInfo: WeightInfo;
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: BenchmarkHelper<Self>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Claimed { account: T::AccountId, amount: BalanceFor<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		AlreadyClaimed,
		InvalidClaim,
	}

	#[pallet::storage]
	#[pallet::getter(fn claimed)]
	pub type Claimed<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, ProcessedClaimFor<T>>;

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		T::AccountId: IsType<<<T::Signature as Verify>::Signer as IdentifyAccount>::AccountId>,
	{
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config>::WeightInfo::claim())]
		pub fn claim(
			origin: OriginFor<T>,
			proof: ClaimProofFor<T>,
			destination: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			if Self::claimed(&who).is_some() {
				return Err(Error::<T>::AlreadyClaimed)?;
			}

			if !proof.validate(&who, T::Signer::get()) {
				#[cfg(not(feature = "runtime-benchmarks"))]
				return Err(Error::<T>::InvalidClaim)?;
			}

			let amount = proof.amount;
			let destination = <T::Lookup as StaticLookup>::lookup(destination)?;

			let length_as_balance = T::BlockNumberToBalance::convert(T::VestingDuration::get());
			let per_block = amount / length_as_balance.max(One::one());
			let current_block = <frame_system::Pallet<T>>::block_number();

			T::VestedTransferer::vested_transfer(
				&T::Funder::get(),
				&destination,
				amount,
				per_block,
				current_block,
			)?;

			Claimed::<T>::insert(&who, ProcessedClaimFor::<T> { proof, destination });
			Self::deposit_event(Event::<T>::Claimed { account: who, amount });

			Ok(())
		}
	}
}
