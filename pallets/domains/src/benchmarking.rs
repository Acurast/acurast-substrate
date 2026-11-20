use frame_benchmarking::v2::*;
use frame_support::{
	pallet_prelude::*,
	traits::fungible::{Inspect, Mutate},
};
use frame_system::RawOrigin;
use sp_runtime::traits::Saturating;
use sp_std::prelude::*;

use acurast_common::{ListUpdate, ListUpdateOperation};

use crate::*;

#[benchmarks(where T::Currency: Mutate<T::AccountId>)]
mod benches {
	use super::*;

	const ZONE: ZoneId = 0;

	fn label_of(i: u32) -> Label {
		let mut label = vec![b'a'; MAX_LABEL_LENGTH as usize];
		label[0] = b'a' + (i % 26) as u8;
		label[1] = b'a' + (i / 26 % 26) as u8;
		label.try_into().unwrap()
	}

	fn max_host(len: usize) -> Vec<u8> {
		let mut host = Vec::with_capacity(len);
		while host.len() < len {
			if !host.is_empty() {
				host.push(b'.');
			}
			let remaining = len - host.len();
			let mut n = remaining.min(MAX_LABEL_LENGTH as usize);
			if remaining - n == 1 {
				n -= 1;
			}
			host.extend(sp_std::iter::repeat_n(b'a', n));
		}
		host
	}

	fn add<I>(item: I) -> ListUpdate<I>
	where
		I: Encode + Decode + DecodeWithMemTracking + TypeInfo + MaxEncodedLen + Clone + PartialEq,
	{
		ListUpdate { operation: ListUpdateOperation::Add, item }
	}

	fn propose<T: Config>(owner: &T::AccountId, i: u32) {
		Pallet::<T>::propose_transfer(
			RawOrigin::Signed(owner.clone()).into(),
			ZONE,
			label_of(i),
			account("proposed", i, 0),
		)
		.unwrap();
	}

	fn deposit<T: Config>() -> BalanceFor<T> {
		T::Currency::minimum_balance().saturating_mul(10u32.into())
	}

	/// Longest suffix for which `label.suffix` still fits a zone suffix, so `claim` checks `ZoneIdOf`.
	fn zone_suffix() -> Vec<u8> {
		max_host((MAX_SUFFIX_LENGTH - MAX_LABEL_LENGTH - 1) as usize)
	}

	fn setup_zone<T: Config>() {
		Pallet::<T>::add_zone(
			RawOrigin::Root.into(),
			zone_suffix().try_into().unwrap(),
			deposit::<T>(),
		)
		.unwrap();
	}

	fn funded<T: Config>(who: &T::AccountId) -> T::AccountId
	where
		T::Currency: Mutate<T::AccountId>,
	{
		let amount = deposit::<T>().saturating_mul((T::MaxDomainsPerAccount::get() + 10).into());
		T::Currency::set_balance(who, amount);
		who.clone()
	}

	/// Claims `count` labels for `who`, starting at label index `offset`.
	fn claim_many<T: Config>(who: &T::AccountId, offset: u32, count: u32) {
		for i in offset..offset + count {
			Pallet::<T>::claim(RawOrigin::Signed(who.clone()).into(), ZONE, label_of(i)).unwrap();
		}
	}

	#[benchmark]
	fn claim() {
		setup_zone::<T>();
		let caller = funded::<T>(&whitelisted_caller());
		claim_many::<T>(&caller, 1, T::MaxDomainsPerAccount::get() - 1);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), ZONE, label_of(0));

		assert!(Reservations::<T>::contains_key(ZONE, label_of(0)));
	}

	#[benchmark]
	fn release() {
		setup_zone::<T>();
		let caller = funded::<T>(&whitelisted_caller());
		claim_many::<T>(&caller, 0, T::MaxDomainsPerAccount::get());
		propose::<T>(&caller, 0);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), ZONE, label_of(0));

		assert!(!Reservations::<T>::contains_key(ZONE, label_of(0)));
	}

	#[benchmark]
	fn propose_transfer() {
		setup_zone::<T>();
		let caller = funded::<T>(&whitelisted_caller());
		let recipient: T::AccountId = account("recipient", 0, 0);
		claim_many::<T>(&caller, 0, 1);
		propose::<T>(&caller, 0);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), ZONE, label_of(0), recipient.clone());

		assert!(ProposedTransfers::<T>::contains_key(recipient, (ZONE, label_of(0))));
	}

	#[benchmark]
	fn accept_transfer() {
		setup_zone::<T>();
		let max = T::MaxDomainsPerAccount::get();
		let owner = funded::<T>(&account("owner", 0, 0));
		let caller = funded::<T>(&whitelisted_caller());
		claim_many::<T>(&owner, 0, max);
		claim_many::<T>(&caller, max, max - 1);
		Pallet::<T>::propose_transfer(
			RawOrigin::Signed(owner).into(),
			ZONE,
			label_of(0),
			caller.clone(),
		)
		.unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), ZONE, label_of(0));

		assert_eq!(Reservations::<T>::get(ZONE, label_of(0)).unwrap().owner, caller);
	}

	#[benchmark]
	fn reject_transfer() {
		setup_zone::<T>();
		let owner = funded::<T>(&account("owner", 0, 0));
		let caller: T::AccountId = whitelisted_caller();
		claim_many::<T>(&owner, 0, 1);
		Pallet::<T>::propose_transfer(
			RawOrigin::Signed(owner).into(),
			ZONE,
			label_of(0),
			caller.clone(),
		)
		.unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), ZONE, label_of(0));

		assert!(!ProposedTransfers::<T>::contains_key(caller, (ZONE, label_of(0))));
	}

	#[benchmark]
	fn add_zone() {
		setup_zone::<T>();
		let suffix: Suffix =
			[&label_of(0)[..], b".", &zone_suffix()[..]].concat().try_into().unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, suffix, deposit::<T>());

		assert!(Zones::<T>::contains_key(ZONE + 1));
	}

	#[benchmark]
	fn update_zone() {
		setup_zone::<T>();

		#[extrinsic_call]
		_(RawOrigin::Root, ZONE, Some(deposit::<T>()), Some(false));

		assert!(!Zones::<T>::get(ZONE).unwrap().active);
	}

	#[benchmark]
	fn remove_zone() {
		setup_zone::<T>();
		Pallet::<T>::add_zone(
			RawOrigin::Root.into(),
			b"other.acu.run".to_vec().try_into().unwrap(),
			deposit::<T>(),
		)
		.unwrap();
		let owner = funded::<T>(&account("owner", 0, 0));
		Pallet::<T>::claim(RawOrigin::Signed(owner).into(), ZONE + 1, label_of(0)).unwrap();
		Pallet::<T>::update_banned_labels(RawOrigin::Root.into(), ZONE + 1, add(label_of(1)))
			.unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, ZONE);

		assert!(!Zones::<T>::contains_key(ZONE));
	}

	#[benchmark]
	fn update_reserved_labels() {
		#[extrinsic_call]
		_(RawOrigin::Root, add(label_of(0)));

		assert!(ReservedLabels::<T>::contains_key(label_of(0)));
	}

	#[benchmark]
	fn update_banned_labels() {
		setup_zone::<T>();
		let owner = funded::<T>(&account("owner", 0, 0));
		claim_many::<T>(&owner, 0, T::MaxDomainsPerAccount::get());
		propose::<T>(&owner, 0);

		#[extrinsic_call]
		_(RawOrigin::Root, ZONE, add(label_of(0)));

		assert!(BannedLabels::<T>::contains_key(ZONE, label_of(0)));
		assert!(!Reservations::<T>::contains_key(ZONE, label_of(0)));
	}

	#[benchmark]
	fn force_release() {
		setup_zone::<T>();
		let owner = funded::<T>(&account("owner", 0, 0));
		claim_many::<T>(&owner, 0, T::MaxDomainsPerAccount::get());
		propose::<T>(&owner, 0);

		#[extrinsic_call]
		_(RawOrigin::Root, ZONE, label_of(0));

		assert!(!Reservations::<T>::contains_key(ZONE, label_of(0)));
	}

	#[benchmark]
	fn update_denied_hosts() {
		let host: Host = max_host(MAX_HOST_LENGTH as usize).try_into().unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, add(host.clone()));

		assert!(DeniedHosts::<T>::contains_key(host));
	}

	#[benchmark]
	fn update_denied_deployers() {
		let denied: T::AccountId = account("denied", 0, 0);

		#[extrinsic_call]
		_(RawOrigin::Root, add(denied.clone()));

		assert!(DeniedDeployers::<T>::contains_key(denied));
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
