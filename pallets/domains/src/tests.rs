use frame_support::{assert_noop, assert_ok, traits::fungible::InspectHold};
use sp_runtime::{traits::BadOrigin, TokenError};

use acurast_common::{ListUpdate, ListUpdateOperation};
use frame_support::pallet_prelude::{
	Decode, DecodeWithMemTracking, Encode, MaxEncodedLen, TypeInfo,
};

use crate::{
	mock::*,
	utils::{is_valid_host, is_valid_label},
	*,
};

const ZONE: ZoneId = 0;
const OTHER_ZONE: ZoneId = 1;

fn label(s: &str) -> Label {
	s.as_bytes().to_vec().try_into().unwrap()
}

fn suffix(s: &str) -> Suffix {
	s.as_bytes().to_vec().try_into().unwrap()
}

fn host(s: &str) -> Host {
	s.as_bytes().to_vec().try_into().unwrap()
}

fn add<I>(item: I) -> ListUpdate<I>
where
	I: Encode + Decode + DecodeWithMemTracking + TypeInfo + MaxEncodedLen + Clone + PartialEq,
{
	ListUpdate { operation: ListUpdateOperation::Add, item }
}

fn remove<I>(item: I) -> ListUpdate<I>
where
	I: Encode + Decode + DecodeWithMemTracking + TypeInfo + MaxEncodedLen + Clone + PartialEq,
{
	ListUpdate { operation: ListUpdateOperation::Remove, item }
}

fn held(who: &AccountId) -> Balance {
	Balances::balance_on_hold(&HoldReason::DomainDeposit.into(), who)
}

fn setup_zones() {
	assert_ok!(AcurastDomains::add_zone(RuntimeOrigin::root(), suffix("canary.acu.run"), DEPOSIT));
	assert_ok!(AcurastDomains::add_zone(RuntimeOrigin::root(), suffix("mainnet.acu.run"), DEPOSIT));
	assert_eq!(ZoneIdOf::<Test>::get(suffix("canary.acu.run")), Some(ZONE));
	assert_eq!(ZoneIdOf::<Test>::get(suffix("mainnet.acu.run")), Some(OTHER_ZONE));
}

fn claim(who: &AccountId, zone_id: ZoneId, name: &str) -> sp_runtime::DispatchResult {
	AcurastDomains::claim(RuntimeOrigin::signed(who.clone()), zone_id, label(name))
}

fn last_event() -> RuntimeEvent {
	System::events().pop().expect("an event").event
}

#[test]
fn label_rules() {
	for ok in ["abc", "my-app", "a1b", "0123456789abcdeg", "0123456789abcde", &"a".repeat(63)] {
		assert!(is_valid_label(ok.as_bytes()), "{ok}");
	}
	for bad in [
		"ab",
		"-abc",
		"abc-",
		"ABC",
		"my_app",
		"my.app",
		"xn--abc",
		"0123456789abcdef",
		"deadbeefdeadbeef",
		&"a".repeat(64),
	] {
		assert!(!is_valid_label(bad.as_bytes()), "{bad}");
	}
}

#[test]
fn host_rules() {
	for ok in ["evil.com", "a.b-c.d", "localhost", "canary.acu.run"] {
		assert!(is_valid_host(ok.as_bytes()), "{ok}");
	}
	for bad in
		["", ".evil.com", "evil.com.", "evil..com", "Evil.com", "-a.com", "a-.com", "*.a.com"]
	{
		assert!(!is_valid_host(bad.as_bytes()), "{bad}");
	}
}

#[test]
fn claim_holds_deposit() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_ok!(claim(&ALICE, ZONE, "myapp"));

		assert_eq!(held(&ALICE), DEPOSIT);
		assert_eq!(
			Reservations::<Test>::get(ZONE, label("myapp")),
			Some(Reservation {
				owner: ALICE,
				deposit: DEPOSIT,
				registered_at: 1,
				pending_owner: None
			})
		);
		assert_eq!(DomainsOf::<Test>::get(ALICE).into_inner(), vec![(ZONE, label("myapp"))]);
		assert_eq!(
			last_event(),
			Event::DomainClaimed { zone_id: ZONE, label: label("myapp"), owner: ALICE }.into()
		);

		// same label is independent per zone
		assert_ok!(claim(&BOB, OTHER_ZONE, "myapp"));
	});
}

#[test]
fn claim_rejections() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_noop!(claim(&ALICE, 9, "myapp"), Error::<Test>::ZoneNotFound);
		assert_noop!(claim(&ALICE, ZONE, "xn--abc"), Error::<Test>::InvalidLabel);
		assert_noop!(claim(&POOR, ZONE, "myapp"), TokenError::FundsUnavailable);

		assert_ok!(claim(&ALICE, ZONE, "myapp"));
		assert_noop!(claim(&BOB, ZONE, "myapp"), Error::<Test>::DomainAlreadyClaimed);
		assert_noop!(claim(&ALICE, ZONE, "myapp"), Error::<Test>::DomainAlreadyClaimed);

		assert_ok!(AcurastDomains::update_zone(RuntimeOrigin::root(), ZONE, None, Some(false)));
		assert_noop!(claim(&BOB, ZONE, "other"), Error::<Test>::ZoneInactive);

		assert_ok!(AcurastDomains::update_denied_deployers(RuntimeOrigin::root(), add(BOB)));
		assert_noop!(claim(&BOB, OTHER_ZONE, "other"), Error::<Test>::DeployerDenied);
		assert_ok!(AcurastDomains::update_denied_deployers(RuntimeOrigin::root(), remove(BOB)));
		assert_ok!(claim(&BOB, OTHER_ZONE, "other"));
	});
}

#[test]
fn claim_respects_max_domains() {
	new_test_ext().execute_with(|| {
		setup_zones();
		for name in ["one", "two", "three"] {
			assert_ok!(claim(&ALICE, ZONE, name));
		}
		assert_noop!(claim(&ALICE, ZONE, "four"), Error::<Test>::TooManyDomains);
	});
}

#[test]
fn release_refunds_deposit() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_ok!(claim(&ALICE, ZONE, "myapp"));
		assert_noop!(
			AcurastDomains::release(RuntimeOrigin::signed(BOB), ZONE, label("myapp")),
			Error::<Test>::NotDomainOwner
		);
		assert_noop!(
			AcurastDomains::release(RuntimeOrigin::signed(ALICE), ZONE, label("nope")),
			Error::<Test>::DomainNotFound
		);

		// deposit is fixed at claim time
		assert_ok!(AcurastDomains::update_zone(
			RuntimeOrigin::root(),
			ZONE,
			Some(2 * DEPOSIT),
			None
		));
		assert_ok!(AcurastDomains::release(RuntimeOrigin::signed(ALICE), ZONE, label("myapp")));

		assert_eq!(held(&ALICE), 0);
		assert_eq!(Balances::free_balance(ALICE), INITIAL_BALANCE);
		assert!(!Reservations::<Test>::contains_key(ZONE, label("myapp")));
		assert!(!DomainsOf::<Test>::contains_key(ALICE));
	});
}

fn propose(from: &AccountId, to: &AccountId) -> sp_runtime::DispatchResult {
	AcurastDomains::propose_transfer(
		RuntimeOrigin::signed(from.clone()),
		ZONE,
		label("myapp"),
		to.clone(),
	)
}

fn proposed_to(who: &AccountId) -> Vec<DomainKey> {
	ProposedTransfers::<Test>::iter_key_prefix(who).collect()
}

fn accept(who: &AccountId) -> sp_runtime::DispatchResult {
	AcurastDomains::accept_transfer(RuntimeOrigin::signed(who.clone()), ZONE, label("myapp"))
}

fn reject(who: &AccountId) -> sp_runtime::DispatchResult {
	AcurastDomains::reject_transfer(RuntimeOrigin::signed(who.clone()), ZONE, label("myapp"))
}

#[test]
fn transfer_needs_acceptance() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_ok!(claim(&ALICE, ZONE, "myapp"));
		assert_ok!(claim(&ALICE, ZONE, "keep"));
		assert_ok!(propose(&ALICE, &BOB));

		let reservation = Reservations::<Test>::get(ZONE, label("myapp")).unwrap();
		assert_eq!((reservation.owner, reservation.pending_owner), (ALICE, Some(BOB)));
		assert_eq!(proposed_to(&BOB), vec![(ZONE, label("myapp"))]);
		assert_eq!(held(&BOB), 0);

		assert_ok!(AcurastDomains::update_zone(
			RuntimeOrigin::root(),
			ZONE,
			Some(2 * DEPOSIT),
			None
		));
		assert_noop!(accept(&CHARLIE), Error::<Test>::NoPendingTransfer);
		assert_ok!(accept(&BOB));

		assert!(proposed_to(&BOB).is_empty());
		assert_eq!(held(&ALICE), DEPOSIT);
		assert_eq!(held(&BOB), 2 * DEPOSIT);
		assert_eq!(Balances::free_balance(ALICE), INITIAL_BALANCE - DEPOSIT);
		assert_eq!(Balances::free_balance(BOB), INITIAL_BALANCE - 2 * DEPOSIT);
		let reservation = Reservations::<Test>::get(ZONE, label("myapp")).unwrap();
		assert_eq!(
			(reservation.owner, reservation.deposit, reservation.pending_owner),
			(BOB, 2 * DEPOSIT, None)
		);
		assert_eq!(DomainsOf::<Test>::get(ALICE).into_inner(), vec![(ZONE, label("keep"))]);
		assert_eq!(DomainsOf::<Test>::get(BOB).into_inner(), vec![(ZONE, label("myapp"))]);
		assert_noop!(accept(&BOB), Error::<Test>::NoPendingTransfer);

		assert_ok!(AcurastDomains::release(RuntimeOrigin::signed(BOB), ZONE, label("myapp")));
		assert_eq!(Balances::free_balance(BOB), INITIAL_BALANCE);
	});
}

#[test]
fn transfer_can_be_rejected_or_replaced() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_ok!(claim(&ALICE, ZONE, "myapp"));

		assert_noop!(
			AcurastDomains::accept_transfer(RuntimeOrigin::signed(BOB), ZONE, label("nope")),
			Error::<Test>::DomainNotFound
		);
		assert_noop!(
			AcurastDomains::reject_transfer(RuntimeOrigin::signed(BOB), ZONE, label("nope")),
			Error::<Test>::DomainNotFound
		);

		assert_ok!(propose(&ALICE, &BOB));
		assert_noop!(reject(&CHARLIE), Error::<Test>::NotTransferParty);
		assert_ok!(reject(&BOB));
		assert!(proposed_to(&BOB).is_empty());
		assert_noop!(accept(&BOB), Error::<Test>::NoPendingTransfer);

		assert_ok!(propose(&ALICE, &BOB));
		assert_ok!(reject(&ALICE));
		assert_noop!(reject(&ALICE), Error::<Test>::NoPendingTransfer);

		assert_ok!(propose(&ALICE, &BOB));
		assert_ok!(propose(&ALICE, &CHARLIE));
		assert!(proposed_to(&BOB).is_empty());
		assert_eq!(proposed_to(&CHARLIE), vec![(ZONE, label("myapp"))]);
		assert_noop!(accept(&BOB), Error::<Test>::NoPendingTransfer);
		assert_ok!(accept(&CHARLIE));
		assert_eq!(Reservations::<Test>::get(ZONE, label("myapp")).unwrap().owner, CHARLIE);
	});
}

#[test]
fn release_drops_pending_transfer() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_ok!(claim(&ALICE, ZONE, "myapp"));
		assert_ok!(propose(&ALICE, &BOB));
		assert_ok!(AcurastDomains::force_release(RuntimeOrigin::root(), ZONE, label("myapp")));
		assert!(proposed_to(&BOB).is_empty());

		assert_ok!(claim(&CHARLIE, ZONE, "myapp"));
		assert_noop!(accept(&BOB), Error::<Test>::NoPendingTransfer);
	});
}

#[test]
fn transfer_rejections() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_ok!(claim(&ALICE, ZONE, "myapp"));

		assert_noop!(propose(&BOB, &CHARLIE), Error::<Test>::NotDomainOwner);
		assert_noop!(propose(&ALICE, &ALICE), Error::<Test>::TransferToSelf);

		assert_ok!(propose(&ALICE, &POOR));
		assert_noop!(accept(&POOR), TokenError::FundsUnavailable);

		assert_ok!(AcurastDomains::update_denied_deployers(RuntimeOrigin::root(), add(BOB)));
		assert_noop!(propose(&ALICE, &BOB), Error::<Test>::DeployerDenied);
		assert_ok!(AcurastDomains::update_denied_deployers(RuntimeOrigin::root(), add(ALICE)));
		assert_noop!(propose(&ALICE, &CHARLIE), Error::<Test>::DeployerDenied);
		assert_ok!(AcurastDomains::update_denied_deployers(RuntimeOrigin::root(), remove(ALICE)));
		assert_ok!(AcurastDomains::update_denied_deployers(RuntimeOrigin::root(), remove(BOB)));

		// deny list is checked again on accept
		assert_ok!(propose(&ALICE, &BOB));
		assert_ok!(AcurastDomains::update_denied_deployers(RuntimeOrigin::root(), add(BOB)));
		assert_noop!(accept(&BOB), Error::<Test>::DeployerDenied);
		assert_ok!(AcurastDomains::update_denied_deployers(RuntimeOrigin::root(), remove(BOB)));

		for name in ["one", "two", "three"] {
			assert_ok!(claim(&CHARLIE, OTHER_ZONE, name));
		}
		assert_ok!(propose(&ALICE, &CHARLIE));
		assert_noop!(accept(&CHARLIE), Error::<Test>::TooManyDomains);
	});
}

#[test]
fn zone_management() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_noop!(
			AcurastDomains::add_zone(RuntimeOrigin::signed(ALICE), suffix("x.acu.run"), 0),
			BadOrigin
		);
		assert_noop!(
			AcurastDomains::add_zone(RuntimeOrigin::root(), suffix("canary.acu.run"), 0),
			Error::<Test>::SuffixAlreadyRegistered
		);
		assert_noop!(
			AcurastDomains::add_zone(RuntimeOrigin::root(), suffix("*.acu.run"), 0),
			Error::<Test>::InvalidSuffix
		);

		assert_ok!(claim(&ALICE, ZONE, "myapp"));
		assert_noop!(
			AcurastDomains::remove_zone(RuntimeOrigin::root(), ZONE),
			Error::<Test>::ZoneNotEmpty
		);
		assert_ok!(AcurastDomains::force_release(RuntimeOrigin::root(), ZONE, label("myapp")));

		assert_ok!(AcurastDomains::update_banned_labels(
			RuntimeOrigin::root(),
			ZONE,
			add(label("scam"))
		));
		assert_noop!(
			AcurastDomains::remove_zone(RuntimeOrigin::root(), ZONE),
			Error::<Test>::ZoneNotEmpty
		);
		assert_ok!(AcurastDomains::update_banned_labels(
			RuntimeOrigin::root(),
			ZONE,
			remove(label("scam"))
		));

		assert_ok!(AcurastDomains::remove_zone(RuntimeOrigin::root(), ZONE));
		assert!(!Zones::<Test>::contains_key(ZONE));
		assert!(!ZoneIdOf::<Test>::contains_key(suffix("canary.acu.run")));
		assert_noop!(
			AcurastDomains::update_zone(RuntimeOrigin::root(), ZONE, None, Some(true)),
			Error::<Test>::ZoneNotFound
		);

		// removed ids are not reused
		assert_ok!(AcurastDomains::add_zone(RuntimeOrigin::root(), suffix("canary.acu.run"), 0));
		assert_eq!(ZoneIdOf::<Test>::get(suffix("canary.acu.run")), Some(2));

		NextZoneId::<Test>::put(ZoneId::MAX);
		assert_noop!(
			AcurastDomains::add_zone(RuntimeOrigin::root(), suffix("x.acu.run"), 0),
			Error::<Test>::TooManyZones
		);
	});
}

#[test]
fn nested_zones_cannot_collide_with_claims() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_ok!(AcurastDomains::add_zone(RuntimeOrigin::root(), suffix("acu.run"), DEPOSIT));
		let parent = ZoneIdOf::<Test>::get(suffix("acu.run")).unwrap();
		assert_noop!(claim(&ALICE, parent, "mainnet"), Error::<Test>::NameIsZone);
		assert_ok!(claim(&ALICE, parent, "myapp"));

		assert_noop!(
			AcurastDomains::add_zone(RuntimeOrigin::root(), suffix("myapp.acu.run"), DEPOSIT),
			Error::<Test>::SuffixClaimed
		);
		assert_ok!(AcurastDomains::add_zone(
			RuntimeOrigin::root(),
			suffix("x.myapp.acu.run"),
			DEPOSIT
		));
	});
}

#[test]
fn reserved_label_blocks_new_claims_only() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_ok!(claim(&ALICE, ZONE, "admin"));
		assert_ok!(AcurastDomains::update_reserved_labels(
			RuntimeOrigin::root(),
			add(label("admin"))
		));

		assert!(Reservations::<Test>::contains_key(ZONE, label("admin")));
		assert_noop!(claim(&BOB, OTHER_ZONE, "admin"), Error::<Test>::LabelReserved);
		assert_noop!(
			AcurastDomains::update_reserved_labels(RuntimeOrigin::root(), add(label("xn--abc"))),
			Error::<Test>::InvalidLabel
		);

		assert_ok!(AcurastDomains::update_reserved_labels(
			RuntimeOrigin::root(),
			remove(label("admin"))
		));
		assert_ok!(claim(&BOB, OTHER_ZONE, "admin"));
	});
}

#[test]
fn ban_releases_and_blocks_zone() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_ok!(claim(&ALICE, ZONE, "scam"));
		assert_ok!(AcurastDomains::update_banned_labels(
			RuntimeOrigin::root(),
			ZONE,
			add(label("scam"))
		));

		assert_eq!(held(&ALICE), 0);
		assert_eq!(Balances::free_balance(ALICE), INITIAL_BALANCE);
		assert!(!Reservations::<Test>::contains_key(ZONE, label("scam")));
		assert_noop!(claim(&ALICE, ZONE, "scam"), Error::<Test>::LabelBanned);
		assert_ok!(claim(&BOB, OTHER_ZONE, "scam"));

		assert_ok!(AcurastDomains::update_banned_labels(
			RuntimeOrigin::root(),
			ZONE,
			remove(label("scam"))
		));
		assert_ok!(claim(&ALICE, ZONE, "scam"));
	});
}

#[test]
fn force_release_refunds_owner() {
	new_test_ext().execute_with(|| {
		setup_zones();
		assert_ok!(claim(&ALICE, ZONE, "myapp"));
		assert_noop!(
			AcurastDomains::force_release(RuntimeOrigin::signed(BOB), ZONE, label("myapp")),
			BadOrigin
		);
		assert_ok!(AcurastDomains::force_release(RuntimeOrigin::root(), ZONE, label("myapp")));
		assert_eq!(Balances::free_balance(ALICE), INITIAL_BALANCE);
		assert_eq!(
			last_event(),
			Event::DomainReleased { zone_id: ZONE, label: label("myapp"), owner: ALICE }.into()
		);
		assert_noop!(
			AcurastDomains::force_release(RuntimeOrigin::root(), ZONE, label("myapp")),
			Error::<Test>::DomainNotFound
		);
	});
}

#[test]
fn deny_lists() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AcurastDomains::update_denied_hosts(RuntimeOrigin::root(), add(host("Evil.com"))),
			Error::<Test>::InvalidHost
		);
		assert_ok!(AcurastDomains::update_denied_hosts(
			RuntimeOrigin::root(),
			add(host("evil.com"))
		));
		assert!(DeniedHosts::<Test>::contains_key(host("evil.com")));
		assert_ok!(AcurastDomains::update_denied_hosts(
			RuntimeOrigin::root(),
			remove(host("evil.com"))
		));
		assert!(!DeniedHosts::<Test>::contains_key(host("evil.com")));

		assert_noop!(
			AcurastDomains::update_denied_deployers(RuntimeOrigin::signed(ALICE), add(BOB)),
			BadOrigin
		);
		assert_ok!(AcurastDomains::update_denied_deployers(RuntimeOrigin::root(), add(BOB)));
		assert!(DeniedDeployers::<Test>::contains_key(BOB));
	});
}
