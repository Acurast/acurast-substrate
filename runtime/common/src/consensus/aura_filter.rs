use frame_support::{
	inherent::Vec,
	pallet_prelude::{Get, PhantomData},
};

pub struct AuraCanAuthor<T, PotentialAuthors>(PhantomData<(T, PotentialAuthors)>);

pub fn can_author<T>(
	author: &T::AccountId,
	slot: &u32,
	selected_authors: &Vec<T::AccountId>,
) -> bool
where
	T: frame_system::Config + pallet_collator_selection::Config,
	T::AccountId: From<[u8; 32]> + Into<[u8; 32]>,
{
	// Relay chain block time: 6s (slot <=> new relay chain block)
	// Acurast block time: 12s (2 * 6s)
	let new_slot = *slot as usize >> 1;

	// Fallback: During migration from PoA to PoS, the pallet staking will be empty.
	let selected_authors = if selected_authors.is_empty() {
		pallet_collator_selection::Pallet::<T>::candidates()
			.iter()
			.map(|info| info.who.clone())
			.collect::<Vec<T::AccountId>>()
	} else {
		selected_authors.to_vec()
	};

	// Aura works by having a list of authorities `A` who are expected to roughly agree
	// on the current time. Time is divided up into discrete slots of `t` seconds each.
	// For each slot `s`, the author of that slot is A[ s % length_of(A) ].
	let active_author = &selected_authors[new_slot % selected_authors.len()];

	author == active_author
}

impl<T, PotentialAuthors> nimbus_primitives::CanAuthor<T::AccountId>
	for AuraCanAuthor<T, PotentialAuthors>
where
	T: frame_system::Config + pallet_collator_selection::Config,
	T::AccountId: From<[u8; 32]> + Into<[u8; 32]>,
	PotentialAuthors: Get<Vec<T::AccountId>>,
{
	/// Determine whether this account is eligible to author in this slot.
	#[cfg(not(feature = "try-runtime"))]
	fn can_author(account: &T::AccountId, slot: &u32) -> bool {
		let selected_authors: Vec<T::AccountId> = PotentialAuthors::get();

		can_author::<T>(account, slot, &selected_authors)
	}
}
