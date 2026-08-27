use frame_support::{assert_ok, traits::ValidatorRegistration};

use crate::mock::*;

pub fn account_id() -> AccountId {
	[0; 32].into()
}

#[test]
fn test_add_remove_candidate() {
	ExtBuilder::default().build().execute_with(|| {
		let add_call = CandidatePreselection::add_candidate(RuntimeOrigin::root(), account_id());
		assert_ok!(add_call);

		assert!(CandidatePreselection::is_registered(&account_id()));

		let remove_call =
			CandidatePreselection::remove_candidate(RuntimeOrigin::root(), account_id());
		assert_ok!(remove_call);

		assert!(!CandidatePreselection::is_registered(&account_id()));

		assert_eq!(
			events(),
			[
				RuntimeEvent::CandidatePreselection(crate::Event::CandidateAdded(account_id())),
				RuntimeEvent::CandidatePreselection(crate::Event::CandidateRemoved(account_id())),
			]
		);
	});
}

#[test]
fn removed_candidate_is_evicted_from_session_validators() {
	ExtBuilder::default().build().execute_with(|| {
		let alice = account(1);
		let bob = account(2);

		assert_ok!(CandidatePreselection::add_candidate(RuntimeOrigin::root(), alice.clone()));
		assert_ok!(CandidatePreselection::add_candidate(RuntimeOrigin::root(), bob.clone()));

		set_keys(&alice);
		set_keys(&bob);

		assert_ok!(CollatorSelection::register_as_candidate(RuntimeOrigin::signed(alice.clone())));
		assert_ok!(CollatorSelection::register_as_candidate(RuntimeOrigin::signed(bob.clone())));

		// after two rotations both candidates are active session validators
		initialize_to_block(2 * Period::get() + 1);
		let validators = Session::validators();
		assert!(validators.contains(&alice));
		assert!(validators.contains(&bob));

		// the council revokes bob's preselection
		assert_ok!(CandidatePreselection::remove_candidate(RuntimeOrigin::root(), bob.clone()));

		// after the next rotations bob is no longer in the session validator set,
		// while alice keeps authoring
		initialize_to_block(4 * Period::get() + 1);
		let validators = Session::validators();
		assert!(validators.contains(&alice));
		assert!(!validators.contains(&bob));
	});
}

#[test]
fn invulnerables_are_exempt_from_preselection() {
	let invulnerable = account(3);

	ExtBuilder::default()
		.with_invulnerables(vec![invulnerable.clone()])
		.build()
		.execute_with(|| {
			// invulnerables are appointed by governance and never register as candidates, so
			// they are deliberately absent from the preselection list
			assert!(!CandidatePreselection::is_registered(&invulnerable));
			set_keys(&invulnerable);

			initialize_to_block(2 * Period::get() + 1);

			assert!(Session::validators().contains(&invulnerable));
		});
}

#[test]
fn empty_preselection_keeps_current_validator_set() {
	ExtBuilder::default().build().execute_with(|| {
		let alice = account(1);
		let bob = account(2);

		assert_ok!(CandidatePreselection::add_candidate(RuntimeOrigin::root(), alice.clone()));
		assert_ok!(CandidatePreselection::add_candidate(RuntimeOrigin::root(), bob.clone()));
		set_keys(&alice);
		set_keys(&bob);
		assert_ok!(CollatorSelection::register_as_candidate(RuntimeOrigin::signed(alice.clone())));
		assert_ok!(CollatorSelection::register_as_candidate(RuntimeOrigin::signed(bob.clone())));

		initialize_to_block(2 * Period::get() + 1);
		let before = Session::validators();
		assert_eq!(before.len(), 2);

		// the council revokes every preselection
		assert_ok!(CandidatePreselection::remove_candidate(RuntimeOrigin::root(), alice.clone()));
		assert_ok!(CandidatePreselection::remove_candidate(RuntimeOrigin::root(), bob.clone()));

		// the filtered collator set is empty; the current validators must be kept rather than
		// an empty set queued, which `pallet_aura` would ignore and never rotate out of
		initialize_to_block(4 * Period::get() + 1);
		assert_eq!(Session::validators(), before);
	});
}

#[test]
fn genesis_preselection_is_seeded() {
	let alice = account(1);

	ExtBuilder::default()
		.with_preselected(vec![alice.clone()])
		.build()
		.execute_with(|| {
			assert!(CandidatePreselection::is_registered(&alice));
			assert!(!CandidatePreselection::is_registered(&account(2)));

			set_keys(&alice);
			assert_ok!(CollatorSelection::register_as_candidate(RuntimeOrigin::signed(
				alice.clone()
			)));

			initialize_to_block(2 * Period::get() + 1);

			assert!(Session::validators().contains(&alice));
		});
}
