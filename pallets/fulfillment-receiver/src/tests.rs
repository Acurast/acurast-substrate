#![cfg(test)]

use frame_support::{assert_err, assert_ok, sp_runtime::DispatchError};

use crate::{
    mock::{
        events, fulfillment_for, script, AcurastFulfillmentReceiver, ExtBuilder, RuntimeEvent,
        RuntimeOrigin,
    },
    stub::{alice_account_id, bob_account_id},
};

#[test]
fn test_job_fulfillment() {
    ExtBuilder::default().build().execute_with(|| {
        let fulfillment = fulfillment_for(script());

        assert_ok!(AcurastFulfillmentReceiver::fulfill(
            RuntimeOrigin::signed(bob_account_id()).into(),
            fulfillment.clone(),
        ));

        assert_eq!(
            events(),
            [RuntimeEvent::AcurastFulfillmentReceiver(
                crate::Event::FulfillReceived(bob_account_id(), fulfillment)
            ),]
        );
    });
}

#[test]
fn test_job_fulfillment_reject() {
    ExtBuilder::default().build().execute_with(|| {
        let fulfillment = fulfillment_for(script());

        assert_err!(
            AcurastFulfillmentReceiver::fulfill(
                RuntimeOrigin::signed(alice_account_id()).into(),
                fulfillment.clone(),
            ),
            DispatchError::BadOrigin
        );

        assert_eq!(events(), []);
    });
}
