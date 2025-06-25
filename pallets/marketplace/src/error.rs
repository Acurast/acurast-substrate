use crate::{Config, Error};

impl<T> From<pallet_acurast::Error<T>> for Error<T> {
	fn from(e: pallet_acurast::Error<T>) -> Self {
		Error::<T>::PalletAcurast(e)
	}
}

impl<T: Config> From<Error<T>> for pallet_acurast::Error<T> {
	fn from(_: Error<T>) -> Self {
		Self::JobHookFailed
	}
}

impl<T> Error<T> {
	/// Returns true if the error is due to invalid matching proposal, i.e. *not* a hard internal error.
	pub(crate) fn is_matching_error(&self) -> bool {
		match self {
			Error::OverdueMatch => true,
			Error::UnderdueMatch => true,
			Error::IncorrectSourceCountInMatch => true,
			Error::IncorrectExecutionIndex => true,
			Error::DuplicateSourceInMatch => true,
			Error::UnverifiedSourceInMatch => true,
			Error::SchedulingWindowExceededInMatch => true,
			Error::MaxMemoryExceededInMatch => true,
			Error::NetworkRequestQuotaExceededInMatch => true,
			Error::InsufficientStorageCapacityInMatch => true,
			Error::SourceNotAllowedInMatch => true,
			Error::ConsumerNotAllowedInMatch => true,
			Error::InsufficientRewardInMatch => true,
			Error::InsufficientReputationInMatch => true,
			Error::ScheduleOverlapInMatch => true,
			Error::ModuleNotAvailableInMatch => true,
			Error::PalletAcurast(e) => matches!(
				*e,
				pallet_acurast::Error::FulfillSourceNotAllowed
					| pallet_acurast::Error::FulfillSourceNotVerified
					| pallet_acurast::Error::AttestationCertificateNotValid
					| pallet_acurast::Error::AttestationUsageExpired
					| pallet_acurast::Error::RevokedCertificate
			),
			Error::CapacityNotFound => true,
			Error::ProcessorVersionMismatch => true,
			Error::ProcessorCpuScoreMismatch => true,
			Error::ProcessorMinMetricsNotMet(_) => true,
			_ => false,
		}
	}
}
