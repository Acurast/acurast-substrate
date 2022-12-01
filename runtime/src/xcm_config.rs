use super::{
	AccountId, AcurastAssetAmount, Assets, Balances, Call, Event, Origin, ParachainInfo,
	ParachainSystem, PolkadotXcm, Runtime, WeightToFee, XcmpQueue,
};
use core::marker::PhantomData;
use frame_support::traits::{Get, OriginTrait};
use frame_support::{
	log, match_types, parameter_types,
	traits::{Everything, Nothing},
};
use pallet_acurast::xcm_adapters::{AssetTransactor, MultiAssetConverter};
use pallet_xcm::XcmPassthrough;
use polkadot_parachain::primitives::Sibling;
use polkadot_runtime_common::impls::ToAuthor;
use xcm::latest::{prelude::*, Weight as XCMWeight};
use xcm_builder::{
	AccountId32Aliases, AllowTopLevelPaidExecutionFrom, AllowUnpaidExecutionFrom,
	AsPrefixedGeneralIndex, Case, ConvertedConcreteAssetId, CurrencyAdapter, EnsureXcmOrigin,
	FixedRateOfFungible, FixedWeightBounds, IsConcrete, LocationInverter, NativeAsset,
	ParentIsPreset, RelayChainAsNative, SiblingParachainAsNative, SiblingParachainConvertsVia,
	SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation, TakeWeightCredit,
	UsingComponents,
};
use xcm_executor::traits::{ConvertOrigin, FilterAssetLocation, JustTry};
use xcm_executor::{traits::ShouldExecute, XcmExecutor};

parameter_types! {
	pub const RelayLocation: MultiLocation = MultiLocation::parent();
	pub const RelayNetwork: NetworkId = NetworkId::Any;
	pub RelayChainOrigin: Origin = cumulus_pallet_xcm::Origin::Relay.into();
	pub Ancestry: MultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
	pub CheckingAccount: AccountId = PolkadotXcm::check_account();
}

/// Type for specifying how a `MultiLocation` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
	// The parent (Relay-chain) origin converts to the parent `AccountId`.
	ParentIsPreset<AccountId>,
	// Sibling parachain origins convert to AccountId via the `ParaId::into`.
	SiblingParachainConvertsVia<Sibling, AccountId>,
	// Straight up local `AccountId32` origins just alias directly to `AccountId`.
	AccountId32Aliases<RelayNetwork, AccountId>,
);

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// biases the kind of local `Origin` it will become.
pub type XcmOriginToTransactDispatchOrigin = (
	// Sovereign account converter; this attempts to derive an `AccountId` from the origin location
	// using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
	// foreign chains who want to have a local sovereign account on this chain which they control.
	SovereignSignedViaLocation<LocationToAccountId, Origin>,
	// Native converter for Relay-chain (Parent) location; will converts to a `Relay` origin when
	// recognized.
	RelayChainAsNative<RelayChainOrigin, Origin>,
	// Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
	// recognized.
	SiblingParachainAsNative<cumulus_pallet_xcm::Origin, Origin>,
	// Native signed account converter; this just converts an `AccountId32` origin into a normal
	// `Origin::Signed` origin of the same 32-byte value.
	SignedAccountId32AsNative<RelayNetwork, Origin>,
	// Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
	SignedAccountId32FromXcm<Origin>,
	XcmPassthrough<Origin>,
);

pub struct SignedAccountId32FromXcm<Origin>(PhantomData<Origin>);
impl<Origin: OriginTrait> ConvertOrigin<Origin> for SignedAccountId32FromXcm<Origin>
where
	Origin::AccountId: From<[u8; 32]>,
{
	fn convert_origin(
		origin: impl Into<MultiLocation>,
		kind: OriginKind,
	) -> Result<Origin, MultiLocation> {
		let origin = origin.into();
		log::trace!(
			target: "xcm::origin_conversion",
			"SignedAccountId32AsNative origin: {:?}, kind: {:?}",
			origin, kind,
		);
		match (kind, origin) {
			(
				OriginKind::Xcm,
				MultiLocation { parents: 1, interior: X2(Parachain(_pid), AccountId32 { id, .. }) },
			) => Ok(Origin::signed(id.into())),
			(_, origin) => Err(origin),
		}
	}
}
parameter_types! {
	// One XCM operation is 1_000_000_000 weight - almost certainly a conservative estimate.
	pub UnitWeightCost: u64 = 1_000_000_000;
	pub const MaxInstructions: u32 = 100;
}

match_types! {
	pub type ParentOrParentsExecutivePlurality: impl Contains<MultiLocation> = {
		MultiLocation { parents: 1, interior: Here } |
		MultiLocation { parents: 1, interior: X1(Plurality { id: BodyId::Executive, .. }) }
	};
}

//TODO: move DenyThenTry to polkadot's xcm module.
/// Deny executing the xcm message if it matches any of the Deny filter regardless of anything else.
/// If it passes the Deny, and matches one of the Allow cases then it is let through.
pub struct DenyThenTry<Deny, Allow>(PhantomData<Deny>, PhantomData<Allow>)
where
	Deny: ShouldExecute,
	Allow: ShouldExecute;

impl<Deny, Allow> ShouldExecute for DenyThenTry<Deny, Allow>
where
	Deny: ShouldExecute,
	Allow: ShouldExecute,
{
	fn should_execute<Call>(
		origin: &MultiLocation,
		message: &mut Xcm<Call>,
		max_weight: XCMWeight,
		weight_credit: &mut XCMWeight,
	) -> Result<(), ()> {
		Deny::should_execute(origin, message, max_weight, weight_credit)?;
		Allow::should_execute(origin, message, max_weight, weight_credit)
	}
}

// See issue #5233
pub struct DenyReserveTransferToRelayChain;
impl ShouldExecute for DenyReserveTransferToRelayChain {
	fn should_execute<Call>(
		origin: &MultiLocation,
		message: &mut Xcm<Call>,
		_max_weight: XCMWeight,
		_weight_credit: &mut XCMWeight,
	) -> Result<(), ()> {
		if message.0.iter().any(|inst| {
			matches!(
				inst,
				InitiateReserveWithdraw {
					reserve: MultiLocation { parents: 1, interior: Here },
					..
				} | DepositReserveAsset { dest: MultiLocation { parents: 1, interior: Here }, .. }
					| TransferReserveAsset {
						dest: MultiLocation { parents: 1, interior: Here },
						..
					}
			)
		}) {
			return Err(()); // Deny
		}

		// An unexpected reserve transfer has arrived from the Relay Chain. Generally, `IsReserve`
		// should not allow this, but we just log it here.
		if matches!(origin, MultiLocation { parents: 1, interior: Here })
			&& message.0.iter().any(|inst| matches!(inst, ReserveAssetDeposited { .. }))
		{
			log::warn!(
				target: "xcm::barriers",
				"Unexpected ReserveAssetDeposited from the Relay Chain",
			);
		}
		// Permit everything else
		Ok(())
	}
}

pub type Barrier = DenyThenTry<
	DenyReserveTransferToRelayChain,
	(
		TakeWeightCredit,
		AllowTopLevelPaidExecutionFrom<Everything>,
		AllowUnpaidExecutionFrom<ParentOrParentsExecutivePlurality>,
		// ^^^ Parent and its exec plurality get free execution
	),
>;

//- From PR https://github.com/paritytech/cumulus/pull/936
fn matches_prefix(prefix: &MultiLocation, loc: &MultiLocation) -> bool {
	prefix.parent_count() == loc.parent_count()
		&& loc.len() >= prefix.len()
		&& prefix
			.interior()
			.iter()
			.zip(loc.interior().iter())
			.all(|(prefix_junction, junction)| prefix_junction == junction)
}

pub type OpenBarrier = AllowUnpaidExecutionFrom<Everything>;
pub struct ReserveAssetsFrom<T>(PhantomData<T>);
impl<T: Get<MultiLocation>> FilterAssetLocation for ReserveAssetsFrom<T> {
	fn filter_asset_location(asset: &MultiAsset, origin: &MultiLocation) -> bool {
		let prefix = T::get();
		log::trace!(target: "xcm::AssetsFrom", "prefix: {:?}, origin: {:?}", prefix, origin);
		&prefix == origin
			&& match asset {
				MultiAsset { id: Concrete(asset_loc), fun: Fungible(_a) } => {
					matches_prefix(&prefix, asset_loc)
				},
				_ => false,
			}
	}
}

parameter_types! {
	pub StatemintChainId: u32 = 1000;
	pub StatemintAssetsPalletIndex: u8 = 50;
	pub StatemintLocation: MultiLocation = MultiLocation::new(1, X1(Parachain(StatemintChainId::get())));
	// ALWAYS ensure that the index in PalletInstance stays up-to-date with
	// Statemint's Assets pallet index
	pub StatemintAssetsPalletLocation: MultiLocation =
		MultiLocation::new(1, X2(Parachain(StatemintChainId::get()), PalletInstance(StatemintAssetsPalletIndex::get())));

	pub StatemintNativeAsset : MultiLocation = MultiLocation::new(1, X3(Parachain(StatemintChainId::get()), PalletInstance(StatemintAssetsPalletIndex::get()), GeneralIndex(42)));
	pub StatemintNativePerSecond: (xcm::v1::AssetId, u128) = (
		MultiLocation::new(1, X3(Parachain(StatemintChainId::get()), PalletInstance(StatemintAssetsPalletIndex::get()), GeneralIndex(42))).into(),
		super::constants::default_fee_per_second()
	);
}

parameter_types! {
	pub StatemintDot: (MultiAssetFilter, MultiLocation) = (
		Wild(AllOf {
			id: Concrete( MultiLocation{ parents: 1, interior: Here }),
			fun: WildFungibility::Fungible
		}),

		MultiLocation::new(1, X1(Parachain(StatemintChainId::get()))),
	);
}

pub type Reserves = (
	NativeAsset,
	ReserveAssetsFrom<StatemintLocation>,
	ReserveAssetsFrom<RelayLocation>,
	Case<StatemintDot>,
);

pub struct StatemintAssetConverter;
impl MultiAssetConverter<<Runtime as pallet_assets::Config>::AssetId> for StatemintAssetConverter {
	type Error = XcmError;

	fn try_convert(
		asset: &MultiAsset,
	) -> Result<<Runtime as pallet_assets::Config>::AssetId, Self::Error> {
		match &asset.id {
			Concrete(location) => {
				if let Some(Junction::GeneralIndex(id)) =
					location.match_and_split(&StatemintAssetsPalletLocation::get())
				{
					(*id)
						.try_into()
						.map_err(|_| XcmError::FailedToTransactAsset("Asset not supported"))
				} else {
					Err(XcmError::FailedToTransactAsset("Asset not supported"))
				}
			},
			Abstract(_) => Err(XcmError::FailedToTransactAsset("Asset not supported")),
		}
	}
}

/// Means for transacting assets from Statemine.
/// We assume Statemine acts as reserve for all assets defined in its Assets pallet,
/// and the same asset ID is used locally.
/// (this is rather simplistic, a more refined implementation could implement
/// something like an "asset manager" where only assets that have been specifically
/// registered are considered for reserve-based asset transfers).
pub type StatemintFungiblesTransactor = AssetTransactor<
	Runtime,
	// Use this fungibles implementation:
	Assets,
	// Use this currency when it is a fungible asset matching the given location or name:
	ConvertedConcreteAssetId<
		parachains_common::AssetId,
		AcurastAssetAmount,
		AsPrefixedGeneralIndex<StatemintAssetsPalletLocation, parachains_common::AssetId, JustTry>,
		JustTry,
	>,
	// Convert an XCM MultiLocation into a local account id:
	LocationToAccountId,
	// Our chain's account ID type (we can't get away without mentioning it explicitly):
	AccountId,
	// We don't track any teleports of `Assets`.
	Nothing,
	// We don't track any teleports of `Assets`.
	CheckingAccount,
	StatemintAssetConverter,
>;

/// Means for transacting assets on this chain.
pub type LocalAssetTransactor = CurrencyAdapter<
	// Use this currency:
	Balances,
	// Use this currency when it is a fungible asset matching the given location or name:
	IsConcrete<RelayLocation>,
	// Do a simple punn to convert an AccountId32 MultiLocation into a native chain account ID:
	LocationToAccountId,
	// Our chain's account ID type (we can't get away without mentioning it explicitly):
	AccountId,
	// We don't track any teleports.
	(),
>;

pub type StatemintNativeAssetTransactor =
	CurrencyAdapter<Balances, IsConcrete<StatemintNativeAsset>, LocationToAccountId, AccountId, ()>;

// Means for transacting assets on this chain. StatemintNativeAssetTransactor should come before
// StatemintFungiblesTransactor so it gets executed first and we mint a native asset from an xcm
pub type AssetTransactors =
	(LocalAssetTransactor, StatemintNativeAssetTransactor, StatemintFungiblesTransactor);

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
	type Call = Call;
	type XcmSender = XcmRouter;
	// How to withdraw and deposit an asset.
	type AssetTransactor = AssetTransactors;
	type OriginConverter = XcmOriginToTransactDispatchOrigin;
	type IsReserve = Reserves;
	type IsTeleporter = (); // Teleporting is disabled.
	type LocationInverter = LocationInverter<Ancestry>;
	// type Barrier = OpenBarrier;
	type Barrier = Barrier;
	type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
	type Trader = (
		FixedRateOfFungible<StatemintNativePerSecond, ()>,
		UsingComponents<WeightToFee, RelayLocation, AccountId, Balances, ToAuthor<Runtime>>,
	);
	type ResponseHandler = PolkadotXcm;
	type AssetTrap = PolkadotXcm;
	type AssetClaims = PolkadotXcm;
	type SubscriptionService = PolkadotXcm;
}

/// No local origins on this chain are allowed to dispatch XCM sends/executions.
pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, RelayNetwork>;

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = (
	// Two routers - use UMP to communicate with the relay chain:
	cumulus_primitives_utility::ParentAsUmp<ParachainSystem, ()>,
	// ..and XCMP to communicate with the sibling chains.
	XcmpQueue,
);

impl pallet_xcm::Config for Runtime {
	type Event = Event;
	type SendXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type XcmRouter = XcmRouter;
	type ExecuteXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type XcmExecuteFilter = Nothing;
	// ^ Disable dispatchable execute on the XCM pallet.
	// Needs to be `Everything` for local testing.
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type XcmTeleportFilter = Everything;
	type XcmReserveTransferFilter = Nothing;
	type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
	type LocationInverter = LocationInverter<Ancestry>;
	type Origin = Origin;
	type Call = Call;

	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	// ^ Override for AdvertisedXcmVersion default
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
}

impl cumulus_pallet_xcm::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}
