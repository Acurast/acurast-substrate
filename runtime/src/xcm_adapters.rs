use frame_support::{
	sp_runtime::traits::Get,
	traits::{fungibles, Contains},
};
use sp_std::{marker::PhantomData, result::Result};
use xcm::latest::{Error as XcmError, MultiAsset, MultiLocation, Result as XcmResult};
use xcm_builder::{FungiblesMutateAdapter, FungiblesTransferAdapter};
use xcm_executor::traits::{Convert, MatchesFungibles, TransactAsset};

/// Wrapper around FungiblesAdapter. It does not add any custom behaviour now, but we might need to customize it later.
pub struct AssetTransactor<
	Assets,
	Matcher,
	AccountIdConverter,
	AccountId,
	CheckAsset,
	CheckingAccount,
>(PhantomData<(Assets, Matcher, AccountIdConverter, AccountId, CheckAsset, CheckingAccount)>);

impl<
		Assets: fungibles::Mutate<AccountId>
			+ fungibles::Transfer<AccountId>
			+ fungibles::Create<AccountId>,
		Matcher: MatchesFungibles<Assets::AssetId, Assets::Balance>,
		AccountIdConverter: Convert<MultiLocation, AccountId>,
		AccountId: Clone, // can't get away without it since Currency is generic over it.
		CheckAsset: Contains<Assets::AssetId>,
		CheckingAccount: Get<AccountId>,
	> TransactAsset
	for AssetTransactor<Assets, Matcher, AccountIdConverter, AccountId, CheckAsset, CheckingAccount>
{
	fn can_check_in(origin: &MultiLocation, what: &MultiAsset) -> XcmResult {
		FungiblesMutateAdapter::<
			Assets,
			Matcher,
			AccountIdConverter,
			AccountId,
			CheckAsset,
			CheckingAccount,
		>::can_check_in(origin, what)
	}

	fn check_in(origin: &MultiLocation, what: &MultiAsset) {
		FungiblesMutateAdapter::<
			Assets,
			Matcher,
			AccountIdConverter,
			AccountId,
			CheckAsset,
			CheckingAccount,
		>::check_in(origin, what)
	}

	fn check_out(dest: &MultiLocation, what: &MultiAsset) {
		FungiblesMutateAdapter::<
			Assets,
			Matcher,
			AccountIdConverter,
			AccountId,
			CheckAsset,
			CheckingAccount,
		>::check_out(dest, what)
	}

	fn deposit_asset(what: &MultiAsset, who: &MultiLocation) -> XcmResult {
		FungiblesMutateAdapter::<
			Assets,
			Matcher,
			AccountIdConverter,
			AccountId,
			CheckAsset,
			CheckingAccount,
		>::deposit_asset(what, who)
	}

	fn withdraw_asset(
		what: &MultiAsset,
		who: &MultiLocation,
	) -> Result<xcm_executor::Assets, XcmError> {
		FungiblesMutateAdapter::<
			Assets,
			Matcher,
			AccountIdConverter,
			AccountId,
			CheckAsset,
			CheckingAccount,
		>::withdraw_asset(what, who)
	}

	fn internal_transfer_asset(
		what: &MultiAsset,
		from: &MultiLocation,
		to: &MultiLocation,
	) -> Result<xcm_executor::Assets, XcmError> {
		FungiblesTransferAdapter::<Assets, Matcher, AccountIdConverter, AccountId>::internal_transfer_asset(
            what, from, to,
        )
	}
}
