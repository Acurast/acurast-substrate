#![cfg(test)]

use frame_support::{assert_ok, traits::ValidatorRegistration};

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
				RuntimeEvent::CandidatePreselection(crate::Event::CandidateAdded(
					alice_account_id()
				)),
				RuntimeEvent::CandidatePreselection(crate::Event::CandidateRemoved(
					alice_account_id()
				)),
			]
		);
	});
}
