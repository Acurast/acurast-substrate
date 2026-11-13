use frame_benchmarking::v2::*;
use frame_support::{
	pallet_prelude::*,
	sp_runtime::traits::{Hash, Saturating},
	traits::fungible::{Mutate, MutateHold},
};
use frame_system::{pallet_prelude::BlockNumberFor, Pallet as System, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use acurast_p256_crypto::application_crypto::p256::Pair as P256Pair;
use pallet_acurast::{AccountId20, ContractCall, Layer, MultiOrigin, ProxyAcurastChain, Subject};

use crate::{
	types::BENCHMARK_P256_SIGNATURE, ActivityWindow, BalanceOf, BenchmarkHelper, Call, Config,
	HoldReason, IncomingMessageWithMetaFor, IncomingMessages, IncomingMessagesLookup, MessageFor,
	MessageNonce, OraclePublicKeys, OraclePublicKeysV2, OracleUpdate, OracleUpdateV2,
	OutgoingMessageWithMetaFor, OutgoingMessages, OutgoingMessagesLookup, P256Public,
	P256Signature, Pallet, Payload, Public, Signatures, SignaturesV2, SubjectFor,
	MESSAGES_CLEANUP_MAX_LENGTH, ORACLE_UPDATES_MAX_LENGTH, PUBLIC_KEY_SERIALIZED_SIZE,
	SIGNATURES_MAX_LENGTH,
};

fn set_block<T: Config<I>, I: 'static>(n: BlockNumberFor<T>) {
	System::<T>::set_block_number(n);
}

fn seed_active_oracles<T: Config<I>, I: 'static>(count: u8) -> Vec<Public>
where
	BlockNumberFor<T>: From<u32>,
{
	let publics: Vec<Public> =
		(0..count).map(|i| Public::from_raw([i; PUBLIC_KEY_SERIALIZED_SIZE])).collect();
	let window = active_window::<T, I>();
	for public in &publics {
		OraclePublicKeys::<T, I>::insert(public, window.clone());
	}
	publics
}

/// Like [`seed_active_oracles`], but with valid P-256 public keys: P-256 verification decodes
/// the signer's key before recovering, so invalid key bytes would skip the recovery.
fn seed_active_p256_oracles<T: Config<I>, I: 'static>(count: u8) -> Vec<P256Public>
where
	BlockNumberFor<T>: From<u32>,
{
	let publics: Vec<P256Public> = (0..count)
		.map(|i| {
			P256Pair::generate_from_seed_bytes(&[i.saturating_add(1); 32])
				.expect("valid P-256 secret key; qed")
				.get_public()
		})
		.collect();
	let window = active_window::<T, I>();
	for public in &publics {
		OraclePublicKeysV2::<T, I>::insert(public, window.clone());
	}
	publics
}

fn active_window<T: Config<I>, I: 'static>() -> ActivityWindow<BlockNumberFor<T>>
where
	BlockNumberFor<T>: From<u32>,
{
	let now = System::<T>::block_number();
	ActivityWindow { start_block: now, end_block: Some(now.saturating_add(10_000u32.into())) }
}

/// Lowest signature count benchmarked: the highest `Min*ConfirmationSignatures` of the runtimes.
///
/// A runtime requiring more fails the benchmark with `NotEnoughSignaturesProvided` rather than
/// having its samples below the minimum measure the minimum and flatten the per-signature cost.
const MIN_BENCHMARKED_SIGNATURES: u32 = 3;

/// `n` signatures from distinct active oracles.
///
/// The signature is a well-formed one (sp-core's ecdsa test vector), so each verify does a full
/// public key recovery; benchmark builds skip the check that the recovered key matches.
fn oracle_signatures<T: Config<I>, I: 'static>(n: u32) -> Signatures
where
	BlockNumberFor<T>: From<u32>,
{
	const SIGNATURE: [u8; 65] = hex!("3dde91174bd9359027be59a428b8146513df80a2a3c7eda2194f64de04a69ab97b753169e94db6ffd50921a2668a48b94ca11e3d32c1ff19cfe88890aa7e8f3c00");
	seed_active_oracles::<T, I>(n as u8)
		.into_iter()
		.map(|p| (SIGNATURE.into(), p))
		.collect::<Vec<_>>()
		.try_into()
		.unwrap()
}

/// `n` secp256r1 (P-256) signatures from distinct active oracles.
///
/// Like [`oracle_signatures`], the signature is well-formed (see the `p256_signature_vector`
/// test) so each verify does a full public key recovery.
fn oracle_signatures_v2<T: Config<I>, I: 'static>(n: u32) -> SignaturesV2
where
	BlockNumberFor<T>: From<u32>,
{
	seed_active_p256_oracles::<T, I>(n as u8)
		.into_iter()
		.map(|p| (P256Signature(BENCHMARK_P256_SIGNATURE), p))
		.collect::<Vec<_>>()
		.try_into()
		.unwrap()
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
	T::Currency::hold(&HoldReason::<I>::OutgoingMessageFee.into(), &payer, fee)
		.expect("bench setup: payer must be funded for the message fee; qed");
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

fn fund_for_fee<T: Config<I>, I: 'static>(who: &T::AccountId, fee: BalanceOf<T, I>) {
	mint_to::<T, I>(who, fee.saturating_mul(1_000u32.into()).saturating_add(u32::MAX.into()));
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
			let public = Public::from_raw([i as u8; PUBLIC_KEY_SERIALIZED_SIZE]);
			updates.push(OracleUpdate::Add(public, window.clone()));
		}

		#[extrinsic_call]
		_(RawOrigin::Root, updates.try_into().unwrap());
	}

	#[benchmark]
	fn update_oracles_v2(n: Linear<1, ORACLE_UPDATES_MAX_LENGTH>) {
		let now = frame_system::Pallet::<T>::block_number();
		let window = ActivityWindow::<BlockNumberFor<T>> {
			start_block: now.saturating_sub(1u32.into()),
			end_block: Some(now.saturating_add(10u32.into())),
		};

		let mut updates: Vec<OracleUpdateV2<BlockNumberFor<T>>> = vec![];
		for i in 0..n {
			let public = P256Public::from_raw([i as u8; PUBLIC_KEY_SERIALIZED_SIZE]);
			updates.push(OracleUpdateV2::Add(public, window.clone()));
		}

		#[extrinsic_call]
		_(RawOrigin::Root, updates.try_into().unwrap());
	}

	#[benchmark]
	fn send_test_message() {
		let caller: T::AccountId = whitelisted_caller();

		let (_sender, recipient) = default_subjects::<T, I>();
		let ttl = T::MinTTL::get();
		let fee = T::MinFee::get();

		fund_for_fee::<T, I>(&caller, fee);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), recipient, ttl, fee);
	}

	#[benchmark]
	fn confirm_message_delivery(
		n: Linear<MIN_BENCHMARKED_SIGNATURES, SIGNATURES_MAX_LENGTH>,
	) -> Result<(), BenchmarkError> {
		let relayer: T::AccountId = whitelisted_caller();
		let payer: T::AccountId = account("payer", 0, 0);

		let (sender, recipient) = default_subjects::<T, I>();
		let ttl = T::MinTTL::get().saturating_add(10u32.into());
		let fee = T::MinFee::get();

		fund_for_fee::<T, I>(&payer, fee);
		let nonce: MessageNonce = T::MessageIdHashing::hash_of(&b"nonce".as_slice());
		let payload = b"bench-msg".to_vec();

		let msg = seed_outgoing_message::<T, I>(sender, payer, nonce, recipient, payload, ttl, fee);

		let signatures = oracle_signatures::<T, I>(n);
		let id = msg.message.id;

		#[extrinsic_call]
		_(RawOrigin::Signed(relayer), id, signatures);

		Ok(())
	}

	#[benchmark]
	fn confirm_message_delivery_v2(
		n: Linear<MIN_BENCHMARKED_SIGNATURES, SIGNATURES_MAX_LENGTH>,
	) -> Result<(), BenchmarkError> {
		let relayer: T::AccountId = whitelisted_caller();
		let payer: T::AccountId = account("payer", 0, 0);

		let (sender, recipient) = default_subjects::<T, I>();
		let ttl = T::MinTTL::get().saturating_add(10u32.into());
		let fee = T::MinFee::get();

		fund_for_fee::<T, I>(&payer, fee);
		let nonce: MessageNonce = T::MessageIdHashing::hash_of(&b"nonce".as_slice());
		let payload = b"bench-msg".to_vec();

		let msg = seed_outgoing_message::<T, I>(sender, payer, nonce, recipient, payload, ttl, fee);

		let signatures = oracle_signatures_v2::<T, I>(n);
		let id = msg.message.id;

		#[extrinsic_call]
		_(RawOrigin::Signed(relayer), id, signatures);

		ensure!(
			OutgoingMessages::<T, I>::get(id).is_none(),
			BenchmarkError::Stop("message not confirmed")
		);

		Ok(())
	}

	#[benchmark]
	fn remove_message() {
		let caller: T::AccountId = whitelisted_caller();
		let (_, recipient) = default_subjects::<T, I>();
		// only the sender is allowed to remove its message
		let sender = Pallet::<T, I>::sender_subject(&caller);
		let ttl = T::MinTTL::get();
		let fee = T::MinFee::get();
		let nonce: MessageNonce = T::MessageIdHashing::hash_of(&b"nonce".as_slice());

		fund_for_fee::<T, I>(&caller, fee);

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
	fn receive_message(
		n: Linear<MIN_BENCHMARKED_SIGNATURES, SIGNATURES_MAX_LENGTH>,
	) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let (sender, recipient, payload) = T::BenchmarkHelper::worst_case_incoming_message()
			.unwrap_or_else(|| {
				let (recipient, sender) = default_subjects::<T, I>();
				(sender, recipient, b"incoming".to_vec())
			});
		let relayer = MultiOrigin::Acurast(account::<T::AccountId>("relayer", 0, 0));

		let nonce: MessageNonce = T::MessageIdHashing::hash_of(&b"nonce".as_slice());

		let signatures = oracle_signatures::<T, I>(n);
		let id = Pallet::<T, I>::message_id(&sender, nonce);

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

		// The call succeeds even if processing fails, so make sure the setup took the intended route
		// rather than silently measuring an early processing error.
		let message =
			IncomingMessages::<T, I>::get(id).ok_or(BenchmarkError::Stop("not stored"))?;
		System::<T>::assert_last_event(crate::Event::<T, I>::MessageProcessed { message }.into());

		Ok(())
	}

	#[benchmark]
	fn receive_message_v2(
		n: Linear<MIN_BENCHMARKED_SIGNATURES, SIGNATURES_MAX_LENGTH>,
	) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let (sender, recipient, payload) = T::BenchmarkHelper::worst_case_incoming_message()
			.unwrap_or_else(|| {
				let (recipient, sender) = default_subjects::<T, I>();
				(sender, recipient, b"incoming".to_vec())
			});
		let relayer = MultiOrigin::Acurast(account::<T::AccountId>("relayer", 0, 0));

		let nonce: MessageNonce = T::MessageIdHashing::hash_of(&b"nonce".as_slice());

		let signatures = oracle_signatures_v2::<T, I>(n);
		let id = Pallet::<T, I>::message_id(&sender, nonce);

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

		// The call succeeds even if processing fails, so make sure the setup took the intended route
		// rather than silently measuring an early processing error.
		let message =
			IncomingMessages::<T, I>::get(id).ok_or(BenchmarkError::Stop("not stored"))?;
		System::<T>::assert_last_event(crate::Event::<T, I>::MessageProcessed { message }.into());

		Ok(())
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

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
