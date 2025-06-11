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

			Error::CalculationOverflow => false,
			Error::UnexpectedCheckedCalculation => false,
			Error::JobRegistrationZeroDuration => false,
			Error::JobRegistrationScheduleExceedsMaximumExecutions => false,
			Error::JobRegistrationScheduleContainsZeroExecutions => false,
			Error::JobRegistrationDurationExceedsInterval => false,
			Error::JobRegistrationStartInPast => false,
			Error::JobRegistrationEndBeforeStart => false,
			Error::JobRegistrationZeroSlots => false,
			Error::JobRegistrationIntervalBelowMinimum => false,
			Error::JobStatusNotFound => false,
			Error::JobRegistrationUnmodifiable => false,
			Error::CannotFinalizeJob(_) => false,
			Error::CannotAcknowledgeWhenNotMatched => false,
			Error::CannotAcknowledgeForOtherThanCurrentExecution => false,
			Error::CannotReportWhenNotAcknowledged => false,
			Error::AdvertisementNotFound => false,
			Error::AdvertisementPricingNotFound => false,
			Error::TooManyAllowedConsumers => false,
			Error::TooFewAllowedConsumers => false,
			Error::TooManySlots => false,
			Error::CannotDeleteAdvertisementWhileMatched => false,
			Error::FailedToPay => false,
			Error::AssetNotAllowedByBarrier => false,
			Error::ReportFromUnassignedSource => false,
			Error::MoreReportsThanExpected => false,
			Error::ReportOutsideSchedule => false,
			Error::ReputationNotFound => false,
			Error::JobNotAssigned => false,
			Error::JobCannotBeFinalized => false,
			Error::ProcessorMinMetricsNotMet(_) => true,

			Error::__Ignore(_, _) => false,
		}
	}
}
