use acurast_runtime_common::{
	barrier::Barrier,
	types::{
		EnvKeyMaxSize, EnvValueMaxSize, ExtraFor, MaxAllowedSources, MaxEnvVars, MaxSlots,
		MaxVersions,
	},
	weight,
};
use frame_system::EnsureRoot;

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmarking;
use crate::{
	AcurastPalletId, BundleIds, CorePackageNames, CoreSignatureDigests, LitePackageNames,
	LiteSignatureDigests, PackageNames, Runtime, RuntimeEvent, SignatureDigests,
};

/// Runtime configuration for pallet_acurast.
impl pallet_acurast::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RegistrationExtra = ExtraFor<Self>;
	type MaxAllowedSources = MaxAllowedSources;
	type MaxCertificateRevocationListUpdates = frame_support::traits::ConstU32<10>;
	type MaxSlots = MaxSlots;
	type PalletId = AcurastPalletId;
	type MaxEnvVars = MaxEnvVars;
	type EnvKeyMaxSize = EnvKeyMaxSize;
	type EnvValueMaxSize = EnvValueMaxSize;
	type KeyAttestationBarrier = Barrier<
		Self,
		PackageNames,
		SignatureDigests,
		CorePackageNames,
		LitePackageNames,
		CoreSignatureDigests,
		LiteSignatureDigests,
		BundleIds,
	>;
	type UnixTime = pallet_timestamp::Pallet<Runtime>;
	type JobHooks = pallet_acurast_marketplace::Pallet<Runtime>;
	type ProcessorVersion = pallet_acurast::Version;
	type MaxVersions = MaxVersions;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = weight::pallet_acurast::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarking::AcurastBenchmarkHelper;
}
