use crate::*;

/// Runtime configuration for pallet_timestamp.
impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = weight::pallet_timestamp::WeightInfo<Runtime>;
}
