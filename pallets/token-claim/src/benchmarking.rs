use frame_benchmarking::v2::*;
use frame_support::{
	pallet_prelude::*,
	sp_runtime::traits::{IdentifyAccount, StaticLookup, Verify},
	traits::{Currency, VestedTransfer},
};
use frame_system::RawOrigin;
use sp_std::prelude::*;

use crate::{BalanceFor, Call, ClaimProofFor, Config, Pallet};

pub trait BenchmarkHelper<T: Config> {
	fn dummy_signature() -> T::Signature;
}

#[benchmarks(
	where BalanceFor<T>: IsType<u128> + From<u64>,
	T::AccountId: IsType<<<T::Signature as Verify>::Signer as IdentifyAccount>::AccountId>,
	<<T as frame_system::Config>::Lookup as StaticLookup>::Source: From<<<T::Signature as Verify>::Signer as IdentifyAccount>::AccountId>,
)]
mod benches {

	use super::*;

	// helper inside the benchmark module so `T` is injected by the macro
	fn mint_to<T: Config>(who: &T::AccountId, amount: BalanceFor<T>) {
		let _ = <<<T as crate::Config>::VestedTransferer as VestedTransfer<T::AccountId>>::Currency as Currency<T::AccountId>>::deposit_into_existing(who, amount);
	}

	/// convert
	#[benchmark]
	fn claim() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = account("origin", 0, 0);
		let initial_funds: BalanceFor<T> = 1_000_000_000_000_000u128.into();

		mint_to::<T>(&T::Funder::get(), initial_funds);

		let amount: BalanceFor<T> = 100_000_000_000_000u128.into();
		let proof = ClaimProofFor::<T>::new(amount, T::BenchmarkHelper::dummy_signature());

		// measured extrinsic call — **bare** call expression, first arg must be origin
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), proof, caller.clone().into().into());

		Ok(())
	}
}
