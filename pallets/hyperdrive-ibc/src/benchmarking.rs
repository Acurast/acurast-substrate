use frame_benchmarking::v2::*;
use frame_support::{
	pallet_prelude::*,
	sp_runtime::traits::{Hash, Saturating},
	traits::fungible::{Mutate, MutateHold},
};
use frame_system::{pallet_prelude::BlockNumberFor, Pallet as System, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use pallet_acurast::{AccountId20, ContractCall, Layer, MultiOrigin, ProxyAcurastChain, Subject};

use crate::{
	ActivityWindow, BalanceOf, Call, Config, HoldReason, IncomingMessageWithMetaFor,
	IncomingMessages, IncomingMessagesLookup, MessageFor, MessageNonce, OraclePublicKeys,
	OracleUpdate, OutgoingMessageWithMetaFor, OutgoingMessages, OutgoingMessagesLookup, Pallet,
	Payload, Public, Signatures, SubjectFor, MESSAGES_CLEANUP_MAX_LENGTH,
	ORACLE_UPDATES_MAX_LENGTH, PUBLIC_KEY_SERIALIZED_SIZE,
};

fn set_block<T: Config<I>, I: 'static>(n: BlockNumberFor<T>) {
	System::<T>::set_block_number(n);
}

fn seed_active_oracles<T: Config<I>, I: 'static>(count: u8) -> Vec<Public>
where
	BlockNumberFor<T>: From<u32>,
{
	let now = System::<T>::block_number();
	let start = now;
	let end = Some(now.saturating_add(10_000u32.into()));
	let mut pubs = Vec::with_capacity(count as usize);
	for i in 0..count {
		let pk = [i; PUBLIC_KEY_SERIALIZED_SIZE];
		let public: Public = pk.into();
		OraclePublicKeys::<T, I>::insert(
			public,
			ActivityWindow { start_block: start, end_block: end },
		);
		pubs.push(public);
	}
	pubs
}

fn seed_outgoing_message<T: Config<I>, I: 'static>(
	sender: SubjectFor<T>,
	payer: T::AccountId,
	nonce: MessageNonce,
	recipient: SubjectFor<T>,
	payload: Vec<u8>,
	ttl: BlockNumberFor<T>,
	fee: BalanceOf<T, I>,
) -> OutgoingMessageWithMetaFor<T, I> {
	let current_block = frame_system::Pallet::<T>::block_number();
	let id = Pallet::<T, I>::message_id(&sender, nonce);
	let message = MessageFor::<T> {
		id,
		sender: sender.clone(),
		nonce,
		recipient,
		payload: Payload::try_from(payload).expect("bench payload within limit; qed"),
	};
	let msg = OutgoingMessageWithMetaFor::<T, I> {
		message,
		current_block,
		ttl_block: current_block.saturating_add(ttl),
		fee,
		payer: payer.clone(),
	};
	OutgoingMessages::<T, I>::insert(id, &msg);
	OutgoingMessagesLookup::<T, I>::insert(&sender, nonce, id);
	let _ = T::Currency::hold(&HoldReason::<I>::OutgoingMessageFee.into(), &payer, fee);
	msg
}

fn seed_incoming_message<T: Config<I>, I: 'static>(
	born_at: BlockNumberFor<T>,
	sender: SubjectFor<T>,
	nonce: MessageNonce,
	recipient: SubjectFor<T>,
	payload: Vec<u8>,
	relayer: MultiOrigin<T::AccountId>,
) -> IncomingMessageWithMetaFor<T> {
	let id = Pallet::<T, I>::message_id(&sender, nonce);
	let msg = MessageFor::<T> {
		id,
		sender,
		nonce,
		recipient: recipient.clone(),
		payload: Payload::try_from(payload).expect("bench payload within limit; qed"),
	};
	let m =
		IncomingMessageWithMetaFor::<T> { message: msg.clone(), current_block: born_at, relayer };
	IncomingMessages::<T, I>::insert(id, m.clone());
	IncomingMessagesLookup::<T, I>::insert(&recipient, id, ());
	m
}

fn default_subjects<T: Config<I>, I: 'static>() -> (SubjectFor<T>, SubjectFor<T>) {
	let who = account::<T::AccountId>("sender", 0, 0);
	let layer = Layer::Extrinsic(who);
	let sender = match T::SelfChain::get() {
		ProxyAcurastChain::Acurast => Subject::Acurast(layer),
		ProxyAcurastChain::AcurastCanary => Subject::AcurastCanary(layer),
	};
	let recipient = Subject::Ethereum(Layer::Contract(ContractCall {
		contract: AccountId20(hex!("7F44aD0fD6c15CfBA6f417C33924c8cF0C751d23")),
		selector: None,
	}));
	(sender, recipient)
}

fn mint_to<T: Config<I>, I: 'static>(who: &T::AccountId, amount: BalanceOf<T, I>) {
	_ = <<T as crate::Config<I>>::Currency as Mutate<T::AccountId>>::mint_into(who, amount);
}

#[instance_benchmarks(
	where
		BlockNumberFor<T>: IsType<u32>,
		BalanceOf<T, I>: IsType<u128>,
)]
mod benches {
	use super::*;

	#[benchmark]
	fn update_oracles(n: Linear<1, ORACLE_UPDATES_MAX_LENGTH>) {
		let now = frame_system::Pallet::<T>::block_number();
		let window = ActivityWindow::<BlockNumberFor<T>> {
			start_block: now.saturating_sub(1u32.into()),
			end_block: Some(now.saturating_add(10u32.into())),
		};

		let mut updates: Vec<OracleUpdate<BlockNumberFor<T>>> = vec![];
		for i in 0..n {
			let pkb = [i as u8; PUBLIC_KEY_SERIALIZED_SIZE];
			let public: Public = pkb.into();
			updates.push(OracleUpdate::Add(public, window.clone()));
		}

		#[extrinsic_call]
		_(RawOrigin::Root, updates.try_into().unwrap());
	}

	#[benchmark]
	fn send_test_message() {
		let caller: T::AccountId = whitelisted_caller();

		mint_to::<T, I>(&caller, 1_000_000_000_000u128.into());

		let (_sender, recipient) = default_subjects::<T, I>();
		let ttl = T::MinTTL::get();
		let fee = T::MinFee::get();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), recipient, ttl, fee);
	}

	#[benchmark]
	fn confirm_message_delivery() -> Result<(), BenchmarkError> {
		let relayer: T::AccountId = whitelisted_caller();
		let payer: T::AccountId = account("payer", 0, 0);

		mint_to::<T, I>(&payer, 10_000_000_000_000u128.into());
		T::Currency::hold(
			&HoldReason::OutgoingMessageFee.into(),
			&payer,
			5_000_000_000_000u128.into(),
		)?;

		let (sender, recipient) = default_subjects::<T, I>();
		let ttl = T::MinTTL::get().saturating_add(10u32.into());
		let fee = T::MinFee::get();
		let nonce: MessageNonce = T::MessageIdHashing::hash_of(&b"nonce".as_slice());
		let payload = b"bench-msg".to_vec();

		let msg = seed_outgoing_message::<T, I>(sender, payer, nonce, recipient, payload, ttl, fee);

		let need = T::MinDeliveryConfirmationSignatures::get() as u8;
		let public_keys = seed_active_oracles::<T, I>(need.max(1));
		let signatures: Signatures = public_keys
			.into_iter()
			.map(|p| ([0; 65].into(), p))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		let id = msg.message.id;

		#[extrinsic_call]
		_(RawOrigin::Signed(relayer), id, signatures);

		Ok(())
	}

	#[benchmark]
	fn remove_message() {
		let caller: T::AccountId = whitelisted_caller();
		let (sender, recipient) = default_subjects::<T, I>();
		let ttl = T::MinTTL::get();
		let fee = T::MinFee::get();
		let nonce: MessageNonce = T::MessageIdHashing::hash_of(&b"nonce".as_slice());

		set_block::<T, I>(1u32.into());
		let msg = seed_outgoing_message::<T, I>(
			sender.clone(),
			caller.clone(),
			nonce,
			recipient.clone(),
			b"outgoing".to_vec(),
			ttl,
			fee,
		);
		set_block::<T, I>(msg.ttl_block.saturating_add(1u32.into()));
		let id = msg.message.id;

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id);
	}

	#[benchmark]
	fn receive_message() {
		let caller: T::AccountId = whitelisted_caller();
		let (recipient, sender) = default_subjects::<T, I>();
		let relayer = MultiOrigin::Acurast(account::<T::AccountId>("relayer", 0, 0));

		let nonce: MessageNonce = T::MessageIdHashing::hash_of(&b"nonce".as_slice());
		let payload = b"incoming".to_vec();

		let need = T::MinReceiptConfirmationSignatures::get() as u8;
		let public_keys = seed_active_oracles::<T, I>(need.max(1));
		let signatures: Signatures = public_keys
			.into_iter()
			.map(|p| ([0; 65].into(), p))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();

		#[extrinsic_call]
		_(
			RawOrigin::Signed(caller),
			sender,
			nonce,
			recipient,
			payload.try_into().unwrap(),
			relayer,
			signatures,
		);
	}

	#[benchmark]
	fn clean_incoming(x: Linear<1, MESSAGES_CLEANUP_MAX_LENGTH>) {
		let caller: T::AccountId = whitelisted_caller();
		let (sender, recipient) = default_subjects::<T, I>();
		let relayer = MultiOrigin::Acurast(account::<T::AccountId>("rel", 0, 0));

		let ttl = T::IncomingTTL::get();
		set_block::<T, I>(10_000u32.into());
		let born_expired =
			System::<T>::block_number().saturating_sub(ttl.saturating_add(1u32.into()));

		let mut ids = vec![];
		for i in 0..x {
			let m = seed_incoming_message::<T, I>(
				born_expired,
				sender.clone(),
				T::MessageIdHashing::hash(&[i as u8]),
				recipient.clone(),
				b"old".to_vec(),
				relayer.clone(),
			);
			ids.push(m.message.id);
		}

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), ids.try_into().unwrap());
	}
}
