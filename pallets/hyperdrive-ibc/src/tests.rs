use hex_literal::hex;
use pallet_acurast::AccountId20;
use sp_core::crypto::AccountId32;
use sp_core::crypto::Ss58Codec;
use sp_core::ecdsa::{Public, Signature};
use sp_core::ByteArray;
use sp_core::Encode;
use sp_runtime::traits::Verify;

// Assuming these are in scope:
use crate::{ContractCall, Layer, Message, MessageId, MessageNonce, Payload, Subject};

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
