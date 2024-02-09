#![cfg(test)]

use crate::{mock::*, stub::*, Error};
use frame_support::{assert_err, assert_ok};

#[test]
fn test_create_mapped_asset() {
    ExtBuilder::default().build().execute_with(|| {
        let call = AcurastAssetManager::create(
            RuntimeOrigin::signed(alice_account_id()),
            codec::Compact(0),
            xcm::latest::AssetId::Abstract([0; 32]),
            alice_account_id().into(),
            1,
        );
        assert_ok!(call);
        assert_eq!(
            AcurastAssetManager::asset_index(0),
            Some(xcm::latest::AssetId::Abstract([0; 32]))
        )
    });
}

#[test]
fn test_create_mapped_asset_failure_1() {
    ExtBuilder::default().build().execute_with(|| {
        _ = AcurastAssetManager::create(
            RuntimeOrigin::signed(alice_account_id()),
            codec::Compact(0),
            xcm::latest::AssetId::Abstract([0; 32]),
            alice_account_id().into(),
            1,
        );
        let call = AcurastAssetManager::create(
            RuntimeOrigin::signed(alice_account_id()),
            codec::Compact(0),
            xcm::latest::AssetId::Abstract([1; 32]),
            alice_account_id().into(),
            1,
        );
        assert_err!(call, Error::<Test>::IdAlreadyUsed);
    });
}

#[test]
fn test_create_mapped_asset_failure_2() {
    ExtBuilder::default().build().execute_with(|| {
        _ = AcurastAssetManager::create(
            RuntimeOrigin::signed(alice_account_id()),
            codec::Compact(0),
            xcm::latest::AssetId::Abstract([0; 32]),
            alice_account_id().into(),
            1,
        );
        let call = AcurastAssetManager::create(
            RuntimeOrigin::signed(alice_account_id()),
            codec::Compact(1),
            xcm::latest::AssetId::Abstract([0; 32]),
            alice_account_id().into(),
            1,
        );
        assert_err!(call, Error::<Test>::AssetAlreadyIndexed);
    });
}
