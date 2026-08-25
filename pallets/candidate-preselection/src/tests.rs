use frame_support::{assert_ok, traits::ValidatorRegistration};
use parity_scale_codec::Encode;

use crate::mock::*;

pub fn account_id() -> AccountId {
	[0; 32].into()
}

#[test]
fn test_add_remove_candidate() {
	ExtBuilder.build().execute_with(|| {
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
	ExtBuilder.build().execute_with(|| {
		let alice = account(1);
		let bob = account(2);

		assert_ok!(CandidatePreselection::add_candidate(RuntimeOrigin::root(), alice.clone()));
		assert_ok!(CandidatePreselection::add_candidate(RuntimeOrigin::root(), bob.clone()));

		let alice_keys = MockSessionKeys::generate(&alice.encode(), None);
		assert_ok!(Session::set_keys(
			RuntimeOrigin::signed(alice.clone()),
			alice_keys.keys,
			alice_keys.proof.encode(),
		));
		let bob_keys = MockSessionKeys::generate(&bob.encode(), None);
		assert_ok!(Session::set_keys(
			RuntimeOrigin::signed(bob.clone()),
			bob_keys.keys,
			bob_keys.proof.encode(),
		));

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
