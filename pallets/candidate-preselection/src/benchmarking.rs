use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

use super::*;
use crate::Pallet as CandidatePreselection;

#[benchmarks(
	where
		T::AccountId: From<[u8; 32]>
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn add_candidate() -> Result<(), BenchmarkError> {
		let account_id: T::AccountId = [0; 32].into();

		#[extrinsic_call]
		_(
			RawOrigin::Root,
			account_id
				.try_into()
				.map_err(|_| BenchmarkError::Stop("Error creating validator id"))?,
		);

		Ok(())
	}

	#[benchmark]
	fn remove_candidate() -> Result<(), BenchmarkError> {
		let account_id: T::AccountId = [0; 32].into();
		CandidatePreselection::<T>::add_candidate(
			RawOrigin::Root.into(),
			account_id
				.clone()
				.try_into()
				.map_err(|_| BenchmarkError::Stop("Error creating validator id"))?,
		)?;

		#[extrinsic_call]
		_(
			RawOrigin::Root,
			account_id
				.try_into()
				.map_err(|_| BenchmarkError::Stop("Error creating validator id"))?,
		);

		Ok(())
	}
}
