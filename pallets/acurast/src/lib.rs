#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod migration;
mod traits;
pub mod utils;
pub mod weights;

pub use acurast_common::*;
#[cfg(feature = "runtime-benchmarks")]
pub use benchmarking::BenchmarkHelper;
pub use pallet::*;
pub use traits::*;

pub type JobRegistrationFor<T> = JobRegistration<
	<T as frame_system::Config>::AccountId,
	<T as Config>::MaxAllowedSources,
	<T as Config>::RegistrationExtra,
>;

pub type EnvironmentFor<T> = Environment<
	<T as Config>::MaxEnvVars,
	<T as Config>::EnvKeyMaxSize,
	<T as Config>::EnvValueMaxSize,
>;

#[frame_support::pallet]
pub mod pallet {
	#[cfg(feature = "runtime-benchmarks")]
	use super::BenchmarkHelper;
	use acurast_common::*;
	use core::ops::AddAssign;
	use frame_support::{
		dispatch::DispatchResultWithPostInfo, ensure, pallet_prelude::*, traits::UnixTime,
		Blake2_128Concat, PalletId,
	};
	use frame_system::pallet_prelude::*;
	use sp_std::prelude::*;

	use crate::{traits::*, utils::*, EnvironmentFor, JobRegistrationFor};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Extra structure to include in the registration of a job.
		type RegistrationExtra: Parameter + Member + MaxEncodedLen;
		/// The max length of the allowed sources list for a registration.
		#[pallet::constant]
		type MaxAllowedSources: Get<u32> + ParameterBound;
		#[pallet::constant]
		type MaxCertificateRevocationListUpdates: Get<u32>;
		/// The maximum allowed slots and therefore maximum length of the planned executions per job.
		#[pallet::constant]
		type MaxSlots: Get<u32> + ParameterBound;
		/// The ID for this pallet
		#[pallet::constant]
		type PalletId: Get<PalletId>;
		#[pallet::constant]
		type MaxEnvVars: Get<u32> + ParameterBound;
		#[pallet::constant]
		type EnvKeyMaxSize: Get<u32> + ParameterBound;
		#[pallet::constant]
		type EnvValueMaxSize: Get<u32> + ParameterBound;
		/// Barrier for submit_attestation extrinsic call.
		type KeyAttestationBarrier: KeyAttestationBarrier<Self>;
		/// Timestamp
		type UnixTime: UnixTime;
		/// Hooks used by tightly coupled subpallets.
		type JobHooks: JobHooks<Self>;
		/// The type to use to represent a processor version.
		type ProcessorVersion: PartialOrd + Parameter;
		#[pallet::constant]
		type MaxVersions: Get<u32> + ParameterBound;
		/// Weight Info for extrinsics. Needs to include weight of hooks called. The weights in this pallet or only correct when using the default hooks [()].
		type WeightInfo: WeightInfo;

		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: BenchmarkHelper<Self>;
	}

	pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(5);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// A unique job identifier sequence for jobs created directly from this pallet.
	#[pallet::storage]
	#[pallet::getter(fn job_id_sequence)]
	pub type LocalJobIdSequence<T: Config> = StorageValue<_, JobIdSequence, ValueQuery>;

	/// The storage for [JobRegistration]s. They are stored by the origin chain address and job identifier.
	#[pallet::storage]
	#[pallet::getter(fn stored_job_registration)]
	pub type StoredJobRegistration<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		MultiOrigin<T::AccountId>,
		Blake2_128Concat,
		JobIdSequence,
		JobRegistrationFor<T>,
	>;

	/// Env variables as a map [`JobId`] -> [`AccountId`] `(source)` -> [`EnvVars`].
	#[pallet::storage]
	#[pallet::getter(fn execution_environment)]
	pub type ExecutionEnvironment<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		JobId<T::AccountId>,
		Blake2_128Concat,
		T::AccountId,
		EnvironmentFor<T>,
	>;

	/// The storage for [Attestation]s. They are stored by [AccountId].
	#[pallet::storage]
	#[pallet::getter(fn stored_attestation)]
	pub type StoredAttestation<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Attestation>;

	/// Certificate revocation list storage.
	#[pallet::storage]
	#[pallet::getter(fn stored_revoked_certificate)]
	pub type StoredRevokedCertificate<T: Config> =
		StorageMap<_, Blake2_128Concat, SerialNumber, ()>;

	#[pallet::storage]
	#[pallet::getter(fn v5_migration_state)]
	pub type V5MigrationState<T: Config> =
		StorageValue<_, Option<BoundedVec<u8, ConstU32<80>>>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A registration was successfully stored. [registration, job_id]
		JobRegistrationStored(JobRegistrationFor<T>, JobId<T::AccountId>),
		/// A registration was successfully removed. [job_id]
		JobRegistrationRemoved(JobId<T::AccountId>),
		/// The allowed sources have been updated. [who, old_registration, updates]
		AllowedSourcesUpdated(
			JobId<T::AccountId>,
			JobRegistrationFor<T>,
			BoundedVec<AllowedSourcesUpdate<T::AccountId>, <T as Config>::MaxAllowedSources>,
		),
		/// An attestation was successfully stored. [attestation, who]
		AttestationStored(Attestation, T::AccountId),
		/// The certificate revocation list has been updated. [who, updates]
		CertificateRevocationListUpdated,
		/// The execution environment has been updated. [job_id, sources]
		ExecutionEnvironmentsUpdated(JobId<T::AccountId>, Vec<T::AccountId>),
		/// Migration started.
		V5MigrationStarted,
		/// Migration progressed. [migrations]
		V5MigrationProgress(u32),
		/// Migration completed.
		V5MigrationCompleted,
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Fulfill was executed for a not registered job.
		JobRegistrationNotFound,
		/// The source of fulfill is not allowed for the job.
		FulfillSourceNotAllowed,
		/// The source of fulfill is not verified. The source does not have a valid attestation submitted.
		FulfillSourceNotVerified,
		/// The allowed sources list for a registration exceeded the max length.
		TooManyAllowedSources,
		/// The allowed sources list for a registration cannot be empty if provided.
		TooFewAllowedSources,
		/// The provided script value is not valid. The value needs to be and ipfs:// url.
		InvalidScriptValue,
		/// The provided attestation could not be parsed or is invalid.
		AttestationUsageExpired,
		/// The certificate chain provided in the submit_attestation call is not long enough.
		CertificateChainTooShort,
		/// The submitted attestation root certificate is not valid.
		RootCertificateValidationFailed,
		/// The submitted attestation certificate chain is not valid.
		CertificateChainValidationFailed,
		/// The submitted attestation certificate is not valid
		AttestationCertificateNotValid,
		/// Failed to extract the attestation.
		AttestationExtractionFailed,
		/// Cannot get the attestation issuer name.
		CannotGetAttestationIssuerName,
		/// Cannot get the attestation serial number.
		CannotGetAttestationSerialNumber,
		/// Cannot get the certificate ID.
		CannotGetCertificateId,
		/// Failed to convert the attestation to its bounded type.
		AttestationToBoundedTypeConversionFailed,
		/// Attestation was rejected by [Config::KeyAttestationBarrier].
		AttestationRejected,
		/// Timestamp error.
		FailedTimestampConversion,
		/// Certificate was revoked.
		RevokedCertificate,
		/// Origin is not allowed to update the certificate revocation list.
		CertificateRevocationListUpdateNotAllowed,
		/// The attestation was issued for an unsupported public key type.
		UnsupportedAttestationPublicKeyType,
		/// The submitted attestation public key does not match the source.
		AttestationPublicKeyDoesNotMatchSource,
		/// Calling a job hook produced an error.
		JobHookFailed,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> frame_support::weights::Weight {
			crate::migration::migrate::<T>()
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Registers a job by providing a [JobRegistration]. If a job for the same script was previously registered, it will be overwritten.
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config >::WeightInfo::register())]
		pub fn register(
			origin: OriginFor<T>,
			registration: JobRegistrationFor<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let multi_origin = MultiOrigin::Acurast(who);
			let job_id = (multi_origin, Self::next_job_id());
			Self::register_for(job_id, registration)
		}

		/// Deregisters a job for the given script.
		#[pallet::call_index(1)]
		#[pallet::weight(< T as Config >::WeightInfo::deregister())]
		pub fn deregister(
			origin: OriginFor<T>,
			local_job_id: JobIdSequence,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let multi_origin = MultiOrigin::Acurast(who);
			let job_id = (multi_origin, local_job_id);
			Self::deregister_for(job_id)?;
			Ok(().into())
		}

		/// Updates the allowed sources list of a [JobRegistration].
		#[pallet::call_index(2)]
		#[pallet::weight(< T as Config >::WeightInfo::update_allowed_sources(updates.len() as u32))]
		pub fn update_allowed_sources(
			origin: OriginFor<T>,
			local_job_id: JobIdSequence,
			updates: BoundedVec<
				AllowedSourcesUpdate<T::AccountId>,
				<T as Config>::MaxAllowedSources,
			>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let multi_origin = MultiOrigin::Acurast(who.clone());
			let job_id: JobId<T::AccountId> = (multi_origin, local_job_id);
			let registration = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1)
				.ok_or(Error::<T>::JobRegistrationNotFound)?;

			let mut current_allowed_sources =
				registration.allowed_sources.clone().unwrap_or_default().into_inner();
			for update in &updates {
				let position =
					current_allowed_sources.iter().position(|value| value == &update.item);
				match (position, update.operation) {
					(None, ListUpdateOperation::Add) => {
						current_allowed_sources.push(update.item.clone())
					},
					(Some(pos), ListUpdateOperation::Remove) => {
						current_allowed_sources.remove(pos);
					},
					_ => {},
				}
			}
			let allowed_sources = if current_allowed_sources.is_empty() {
				None
			} else {
				Some(
					AllowedSources::try_from(current_allowed_sources)
						.map_err(|_| Error::<T>::TooManyAllowedSources)?,
				)
			};
			<StoredJobRegistration<T>>::insert(
				&job_id.0,
				job_id.1,
				JobRegistration { allowed_sources, ..registration.clone() },
			);

			<T as Config>::JobHooks::update_allowed_sources_hook(&who, &job_id, &updates)?;

			Self::deposit_event(Event::AllowedSourcesUpdated(job_id, registration, updates));

			Ok(().into())
		}

		/// Submits an attestation given a valid certificate chain.
		///
		/// - As input a list of binary certificates is expected.
		/// - The list must be ordered, starting from one of the known [trusted root certificates](https://developer.android.com/training/articles/security-key-attestation#root_certificate).
		/// - If the represented chain is valid, the [Attestation] details are stored. An existing attestion for signing account gets overwritten.
		///
		/// Revocation: Each atttestation is stored with the unique IDs of the certificates on the chain proofing the attestation's validity.
		#[pallet::call_index(5)]
		#[pallet::weight(< T as Config >::WeightInfo::submit_attestation())]
		pub fn submit_attestation(
			origin: OriginFor<T>,
			attestation_chain: AttestationChain,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(
				attestation_chain.certificate_chain.len() >= 2,
				Error::<T>::CertificateChainTooShort,
			);

			let attestation = validate_and_extract_attestation::<T>(&who, &attestation_chain)?;

			if !T::KeyAttestationBarrier::accept_attestation_for_origin(&who, &attestation) {
				#[cfg(not(feature = "runtime-benchmarks"))]
				return Err(Error::<T>::AttestationRejected.into());
			}

			ensure_not_expired::<T>(&attestation)?;
			ensure_not_revoked::<T>(&attestation)?;

			<StoredAttestation<T>>::insert(&who, attestation.clone());
			Self::deposit_event(Event::AttestationStored(attestation, who));

			Ok(().into())
		}

		/// Updates the certificate revocation list by adding or removing a revoked certificate serial number. Attestations signed
		/// by a revoked certificate will not be considered valid anymore. The `RevocationListUpdateBarrier` configured in [Config] can be used to
		/// customize who can execute this action.
		#[pallet::weight(<T as Config>::WeightInfo::update_certificate_revocation_list())]
		#[pallet::call_index(6)]
		pub fn update_certificate_revocation_list(
			origin: OriginFor<T>,
			updates: BoundedVec<
				CertificateRevocationListUpdate,
				T::MaxCertificateRevocationListUpdates,
			>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			for update in &updates {
				match &update.operation {
					ListUpdateOperation::Add => {
						<StoredRevokedCertificate<T>>::insert(&update.item, ());
					},
					ListUpdateOperation::Remove => {
						<StoredRevokedCertificate<T>>::remove(&update.item);
					},
				}
			}
			Self::deposit_event(Event::CertificateRevocationListUpdated);
			Ok(().into())
		}

		#[pallet::weight(<T as Config>::WeightInfo::set_environment(environment.variables.len() as u32))]
		#[pallet::call_index(7)]
		pub fn set_environment(
			origin: OriginFor<T>,
			job_id_seq: JobIdSequence,
			source: T::AccountId,
			environment: EnvironmentFor<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let multi_origin = MultiOrigin::Acurast(who);
			let job_id: JobId<T::AccountId> = (multi_origin, job_id_seq);
			Self::set_environment_for(
				job_id.clone(),
				BoundedVec::truncate_from(vec![(source.clone(), environment)]),
			)?;

			Ok(().into())
		}

		#[pallet::weight(<T as Config>::WeightInfo::set_environments(environments.len() as u32, environments.iter().map(|(_, env)| env.variables.len() as u32).max().unwrap_or(0u32)))]
		#[pallet::call_index(8)]
		pub fn set_environments(
			origin: OriginFor<T>,
			job_id_seq: JobIdSequence,
			environments: BoundedVec<(T::AccountId, EnvironmentFor<T>), T::MaxSlots>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let multi_origin = MultiOrigin::Acurast(who.clone());
			let job_id: JobId<T::AccountId> = (multi_origin, job_id_seq);
			Self::set_environment_for(job_id, environments)?;

			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Get and update the next job identifier in the sequence.
		pub fn next_job_id() -> JobIdSequence {
			<LocalJobIdSequence<T>>::mutate(|job_id_seq| {
				job_id_seq.add_assign(1);
				*job_id_seq
			})
		}

		/// Registers a job for the given [`multi_origin`].
		///
		/// It assumes the caller was already authorized and is intended to be used from
		/// * The [`Self::register`] extrinsic of this pallet
		/// * An interchain communication protocol like Hyperdrive
		pub fn register_for(
			job_id: JobId<T::AccountId>,
			registration: JobRegistrationFor<T>,
		) -> DispatchResultWithPostInfo {
			ensure!(is_valid_script(&registration.script), Error::<T>::InvalidScriptValue);
			if let Some(allowed_sources) = &registration.allowed_sources {
				let max_allowed_sources_len = T::MaxAllowedSources::get() as usize;
				ensure!(allowed_sources.len() > 0, Error::<T>::TooFewAllowedSources);
				ensure!(
					allowed_sources.len() <= max_allowed_sources_len,
					Error::<T>::TooManyAllowedSources
				);
			}

			<StoredJobRegistration<T>>::insert(&job_id.0, job_id.1, registration.clone());

			<T as Config>::JobHooks::register_hook(&job_id.0, &job_id, &registration)?;

			Self::deposit_event(Event::JobRegistrationStored(registration, job_id.clone()));
			Ok(().into())
		}

		pub fn deregister_for(job_id: JobId<T::AccountId>) -> DispatchResultWithPostInfo {
			<T as Config>::JobHooks::deregister_hook(&job_id)?;
			Self::clear_environment_for(&job_id);
			<StoredJobRegistration<T>>::remove(&job_id.0, job_id.1);
			Self::deposit_event(Event::JobRegistrationRemoved(job_id));
			Ok(().into())
		}

		pub fn set_environment_for(
			job_id: JobId<T::AccountId>,
			environments: BoundedVec<(T::AccountId, EnvironmentFor<T>), T::MaxSlots>,
		) -> Result<(), Error<T>> {
			let sources = environments
				.into_iter()
				.map(|(source, env)| {
					let _registration = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1)
						.ok_or(Error::<T>::JobRegistrationNotFound)?;
					<ExecutionEnvironment<T>>::insert(&job_id, &source, env);
					Ok(source)
				})
				.collect::<Result<Vec<T::AccountId>, Error<T>>>()?;

			Self::deposit_event(Event::ExecutionEnvironmentsUpdated(job_id, sources));

			Ok(())
		}

		pub fn clear_environment_for(job_id: &JobId<T::AccountId>) {
			let _ = <ExecutionEnvironment<T>>::clear_prefix(job_id, T::MaxSlots::get(), None);
		}
	}
}
