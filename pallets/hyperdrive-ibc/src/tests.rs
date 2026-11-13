use hex_literal::hex;
use sp_core::{
	crypto::{AccountId32, Ss58Codec},
	ecdsa::{Public, Signature},
	ByteArray, Encode,
};
use sp_runtime::traits::Verify;

use pallet_acurast::{AccountId20, ContractCall, Layer, Subject};

// Assuming these are in scope:
use crate::{Message, MessageId, MessageNonce, Payload};

#[test]
fn encodes_specific_message_correctly() {
	// {id:'0x7ba7902e9de1360ea56072e7715f4a924cd1260a06f11e377efbb37c4ac35ea4',sender:{Ethereum:{Contract:{contract:'0x7f44ad0fd6c15cfba6f417c33924c8cf0c751d23',selector:null}}},nonce:'0xe8e77626586f73b955364c7b4bbf0bb7f7685ebd40e852b164633a4acbd3244c',recipient:{Acurast:{Extrinsic:'5EYCAe5h8kmzoA4mxYQmkSEPPrDy93poMdg9Lh1d8SehErVo'}},payload:'0x00000000000000000000000000000000000003e8000000000000000000000000185a8b5f92ecd348ed9b12a047ca2b28488b1398065a8dff8dcf886245f9280b'}

	let id = MessageId::from_slice(&hex!(
		"7ba7902e9de1360ea56072e7715f4a924cd1260a06f11e377efbb37c4ac35ea4"
	));

	let sender_contract: AccountId20 =
		AccountId20(hex!("7f44ad0fd6c15cfba6f417c33924c8cf0c751d23"));

	let sender: Subject<AccountId32, AccountId32> =
		Subject::Ethereum(Layer::Contract(ContractCall {
			contract: sender_contract,
			selector: None,
		}));

	let nonce = MessageNonce::from_slice(&hex!(
		"e8e77626586f73b955364c7b4bbf0bb7f7685ebd40e852b164633a4acbd3244c"
	));

	let recipient_account =
		AccountId32::from_ss58check("5EYCAe5h8kmzoA4mxYQmkSEPPrDy93poMdg9Lh1d8SehErVo").unwrap();

	let recipient = Subject::Acurast(Layer::Extrinsic(recipient_account));

	let payload = Payload::try_from(hex!(
        "00000000000000000000000000000000000003e80000000000000000185a8b5f92ecd348ed9b12a047ca2b28488b1398065a8dff8dcf886245f9280b"
    ).to_vec()).unwrap();

	let message = Message::<AccountId32, AccountId32> { id, sender, nonce, recipient, payload };

	let encoded = message.encode();
	let hex_output = hex::encode(&encoded);

	println!("Encoded hex: {}", hex_output);

	// Paste the hex output here after first run
	let expected_hex = "7ba7902e9de1360ea56072e7715f4a924cd1260a06f11e377efbb37c4ac35ea403017f44ad0fd6c15cfba6f417c33924c8cf0c751d2300e8e77626586f73b955364c7b4bbf0bb7f7685ebd40e852b164633a4acbd3244c00006d6f646c687970746f6b656e0000000000000000000000000000000000000000f000000000000000000000000000000000000003e80000000000000000185a8b5f92ecd348ed9b12a047ca2b28488b1398065a8dff8dcf886245f9280b";

	assert_eq!(hex_output, expected_hex);

	let signature = Signature::from_slice(&hex!("adf8fe546ddb7d579d28bb10d035485ed1a5ce85348fbfed76ff7b45e96350074c8380f028cd726c9456f21b8328e2835f9526b30a49c1e1e2354493301c65b100")).unwrap();
	let public = Public::from_slice(&hex!(
		"03a5764c39b53ed3a71806749ed4ca0e0fc5688f6d03ebb116b484d8546d6bd5c7"
	))
	.unwrap();
	assert!(signature.verify(&message.encode()[..], &public));
}

mod receive {
	use acurast_p256_crypto::application_crypto::p256::Pair as P256Pair;
	use frame_support::{assert_err, assert_ok, BoundedVec};
	use hex_literal::hex;
	use pallet_acurast::{AccountId20, ContractCall, Layer, MultiOrigin, Subject};
	use parity_scale_codec::Encode;
	use sp_core::{ecdsa, Pair as _, H256};
	use sp_runtime::AccountId32;

	use crate::{
		mock::*, ActivityWindow, Error, Event, IncomingMessages, MessageFor, MessageNonce,
		OraclePublicKeys, OraclePublicKeysV2, OracleUpdate, OracleUpdateV2, OutgoingMessages,
		P256Public, P256Signature, Payload, SubjectFor,
	};

	fn sender() -> SubjectFor<Test> {
		Subject::Ethereum(Layer::Contract(ContractCall {
			contract: AccountId20(hex!("7f44ad0fd6c15cfba6f417c33924c8cf0c751d23")),
			selector: None,
		}))
	}

	fn recipient() -> SubjectFor<Test> {
		Subject::Acurast(Layer::Extrinsic(AccountId32::new([7u8; 32])))
	}

	fn nonce() -> MessageNonce {
		H256::repeat_byte(1)
	}

	fn payload() -> Payload {
		Payload::try_from(b"hello".to_vec()).unwrap()
	}

	fn relayer() -> MultiOrigin<AccountId> {
		MultiOrigin::Acurast(AccountId32::new([9u8; 32]))
	}

	/// The bytes the oracles sign for an incoming message.
	fn signed_payload() -> Vec<u8> {
		let id = AcurastHyperdriveIbc::message_id(&sender(), nonce());
		let message = MessageFor::<Test> {
			id,
			sender: sender(),
			nonce: nonce(),
			recipient: recipient(),
			payload: payload(),
		};
		(ParachainId::get(), message).encode()
	}

	fn p256_pair(seed: u8) -> P256Pair {
		P256Pair::from_seed(&[seed; 32])
	}

	fn add_oracle(public: P256Public) {
		assert_ok!(AcurastHyperdriveIbc::update_oracles_v2(
			RuntimeOrigin::root(),
			vec![OracleUpdateV2::Add(public, ActivityWindow::default())].try_into().unwrap(),
		));
	}

	#[allow(deprecated)]
	fn add_legacy_oracle(public: ecdsa::Public) {
		assert_ok!(AcurastHyperdriveIbc::update_oracles(
			RuntimeOrigin::root(),
			vec![OracleUpdate::Add(public, ActivityWindow::default())].try_into().unwrap(),
		));
	}

	fn receive_v2(signatures: Vec<(P256Signature, P256Public)>) -> sp_runtime::DispatchResult {
		AcurastHyperdriveIbc::receive_message_v2(
			RuntimeOrigin::signed(AccountId32::new([1u8; 32])),
			sender(),
			nonce(),
			recipient(),
			payload(),
			relayer(),
			BoundedVec::truncate_from(signatures),
		)
	}

	fn assert_received() {
		let id = AcurastHyperdriveIbc::message_id(&sender(), nonce());
		let message = IncomingMessages::<Test>::get(id).expect("message stored");
		System::assert_last_event(Event::MessageProcessed { message }.into());
	}

	#[test]
	fn receive_message_v2_accepts_p256_oracle_signature() {
		new_test_ext().execute_with(|| {
			let pair = p256_pair(1);
			add_oracle(pair.public());

			let signature = pair.sign(&signed_payload());
			assert_ok!(receive_v2(vec![(signature, pair.public())]));
			assert_received();

			// replay is rejected
			let signature = pair.sign(&signed_payload());
			assert_err!(
				receive_v2(vec![(signature, pair.public())]),
				Error::<Test>::MessageAlreadyReceived
			);
		});
	}

	// benchmark builds skip the signature validity check
	#[cfg(not(feature = "runtime-benchmarks"))]
	#[test]
	fn receive_message_v2_rejects_signature_by_other_key() {
		new_test_ext().execute_with(|| {
			let oracle = p256_pair(1);
			add_oracle(oracle.public());

			let signature = p256_pair(2).sign(&signed_payload());
			assert_err!(
				receive_v2(vec![(signature, oracle.public())]),
				Error::<Test>::SignatureInvalid
			);
		});
	}

	#[test]
	fn receive_message_v2_rejects_unknown_oracle() {
		new_test_ext().execute_with(|| {
			let pair = p256_pair(1);

			let signature = pair.sign(&signed_payload());
			assert_err!(
				receive_v2(vec![(signature, pair.public())]),
				Error::<Test>::NotEnoughSignaturesValid
			);
		});
	}

	/// Oracles registered via the deprecated `update_oracles` are unknown to the v2 calls, even
	/// with their key bytes reinterpreted as P-256.
	#[test]
	fn receive_message_v2_ignores_legacy_oracles() {
		new_test_ext().execute_with(|| {
			let legacy = ecdsa::Pair::from_seed(&[1u8; 32]);
			add_legacy_oracle(legacy.public());

			let signature = P256Signature(legacy.sign(&signed_payload()).0);
			assert_err!(
				receive_v2(vec![(signature, P256Public::from_raw(legacy.public().0))]),
				Error::<Test>::NotEnoughSignaturesValid
			);
		});
	}

	/// P-256 oracles registered via `update_oracles_v2` are unknown to the deprecated calls.
	#[allow(deprecated)]
	#[test]
	fn deprecated_receive_message_ignores_p256_oracles() {
		new_test_ext().execute_with(|| {
			let pair = p256_pair(1);
			add_oracle(pair.public());

			let signature = ecdsa::Signature::from_raw(pair.sign(&signed_payload()).0);
			assert_err!(
				AcurastHyperdriveIbc::receive_message(
					RuntimeOrigin::signed(AccountId32::new([1u8; 32])),
					sender(),
					nonce(),
					recipient(),
					payload(),
					relayer(),
					vec![(signature, ecdsa::Public::from_raw(pair.public().0))].try_into().unwrap(),
				),
				Error::<Test>::NotEnoughSignaturesValid
			);
		});
	}

	#[test]
	fn update_oracles_v2_only_touches_v2_storage() {
		new_test_ext().execute_with(|| {
			let pair = p256_pair(1);
			add_oracle(pair.public());
			assert_eq!(OraclePublicKeysV2::<Test>::get(pair.public()), Some(Default::default()));
			assert_eq!(OraclePublicKeys::<Test>::iter_keys().count(), 0);

			assert_ok!(AcurastHyperdriveIbc::update_oracles_v2(
				RuntimeOrigin::root(),
				vec![OracleUpdateV2::Remove(pair.public())].try_into().unwrap(),
			));
			assert_eq!(OraclePublicKeysV2::<Test>::get(pair.public()), None);
			System::assert_last_event(
				Event::OracleUpdatedV2 {
					added: vec![],
					updated: vec![],
					removed: vec![pair.public()],
				}
				.into(),
			);
		});
	}

	#[allow(deprecated)]
	#[test]
	fn deprecated_receive_message_accepts_legacy_secp256k1_oracle() {
		new_test_ext().execute_with(|| {
			let legacy = ecdsa::Pair::from_seed(&[1u8; 32]);
			add_legacy_oracle(legacy.public());

			let signature = legacy.sign(&signed_payload());
			assert_ok!(AcurastHyperdriveIbc::receive_message(
				RuntimeOrigin::signed(AccountId32::new([1u8; 32])),
				sender(),
				nonce(),
				recipient(),
				payload(),
				relayer(),
				vec![(signature, legacy.public())].try_into().unwrap(),
			));
			assert_received();
		});
	}

	/// Checks the well-formed P-256 signature used by the benchmarks.
	#[test]
	fn p256_signature_vector() {
		let pair = p256_pair(1);
		assert!(<P256Pair as sp_core::Pair>::verify(
			&P256Signature(crate::types::BENCHMARK_P256_SIGNATURE),
			b"acurast",
			&pair.public()
		));
	}

	const RELAYER: AccountId32 = AccountId32::new([3u8; 32]);
	const FEE: Balance = 10;

	/// Sends a test message and returns it with the bytes the oracles sign to confirm its
	/// delivery by [`RELAYER`].
	fn send_outgoing() -> (MessageFor<Test>, Vec<u8>) {
		let who = AccountId32::new([2u8; 32]);
		assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), who.clone(), 1_000));
		assert_ok!(AcurastHyperdriveIbc::send_test_message(
			RuntimeOrigin::signed(who),
			sender(),
			MinTTL::get(),
			FEE,
		));
		let message = OutgoingMessages::<Test>::iter_values().next().expect("sent").message;
		let signed = (ParachainId::get(), &message, RELAYER).encode();
		(message, signed)
	}

	fn assert_confirmed(message: &MessageFor<Test>) {
		assert!(OutgoingMessages::<Test>::get(message.id).is_none());
		assert_eq!(Balances::free_balance(RELAYER), FEE);
		System::assert_last_event(Event::MessageDelivered { id: message.id }.into());
	}

	#[test]
	fn confirm_message_delivery_v2_accepts_p256_oracle_signature() {
		new_test_ext().execute_with(|| {
			let pair = p256_pair(1);
			add_oracle(pair.public());
			let (message, signed) = send_outgoing();

			assert_ok!(AcurastHyperdriveIbc::confirm_message_delivery_v2(
				RuntimeOrigin::signed(RELAYER),
				message.id,
				vec![(pair.sign(&signed), pair.public())].try_into().unwrap(),
			));
			assert_confirmed(&message);
		});
	}

	// benchmark builds skip the signature validity check
	#[cfg(not(feature = "runtime-benchmarks"))]
	#[test]
	fn confirm_message_delivery_v2_rejects_signature_for_other_relayer() {
		new_test_ext().execute_with(|| {
			let pair = p256_pair(1);
			add_oracle(pair.public());
			let (message, _) = send_outgoing();
			let for_other = (ParachainId::get(), &message, AccountId32::new([4u8; 32])).encode();

			assert_err!(
				AcurastHyperdriveIbc::confirm_message_delivery_v2(
					RuntimeOrigin::signed(RELAYER),
					message.id,
					vec![(pair.sign(&for_other), pair.public())].try_into().unwrap(),
				),
				Error::<Test>::SignatureInvalid
			);
		});
	}

	#[allow(deprecated)]
	#[test]
	fn deprecated_confirm_message_delivery_accepts_legacy_secp256k1_oracle() {
		new_test_ext().execute_with(|| {
			let legacy = ecdsa::Pair::from_seed(&[1u8; 32]);
			add_legacy_oracle(legacy.public());
			let (message, signed) = send_outgoing();

			assert_ok!(AcurastHyperdriveIbc::confirm_message_delivery(
				RuntimeOrigin::signed(RELAYER),
				message.id,
				vec![(legacy.sign(&signed), legacy.public())].try_into().unwrap(),
			));
			assert_confirmed(&message);
		});
	}
}
