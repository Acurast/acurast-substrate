#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
pub use traits::*;
pub use types::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod traits;
mod types;
pub mod utils;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		dispatch::DispatchResultWithPostInfo,
		pallet_prelude::*,
		traits::{
			fungible::{Inspect, InspectHold, MutateHold},
			tokens::Precision,
			EnsureOrigin,
		},
	};
	use frame_system::pallet_prelude::*;

	use acurast_common::{ListUpdate, ListUpdateOperation};

	use super::*;
	use crate::utils::{is_valid_host, is_valid_label};

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeHoldReason: From<HoldReason>;
		type Currency: Inspect<Self::AccountId>
			+ InspectHold<Self::AccountId, Reason = Self::RuntimeHoldReason>
			+ MutateHold<Self::AccountId, Reason = Self::RuntimeHoldReason>;
		/// Origin for zone management, takedowns and the deny lists.
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		#[pallet::constant]
		type MaxDomainsPerAccount: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::composite_enum]
	pub enum HoldReason {
		/// Deposit backing a claimed domain.
		#[codec(index = 0)]
		DomainDeposit,
	}

	/// Id assigned to the next added zone. Ids are never reused.
	#[pallet::storage]
	pub type NextZoneId<T: Config> = StorageValue<_, ZoneId, ValueQuery>;

	#[pallet::storage]
	pub type Zones<T: Config> = StorageMap<_, Blake2_128Concat, ZoneId, Zone<BalanceFor<T>>>;

	/// Zone id by suffix, for resolving a hostname to its zone.
	#[pallet::storage]
	pub type ZoneIdOf<T: Config> = StorageMap<_, Blake2_128Concat, Suffix, ZoneId>;

	#[pallet::storage]
	pub type Reservations<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, ZoneId, Blake2_128Concat, Label, ReservationFor<T>>;

	#[pallet::storage]
	pub type DomainsOf<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<DomainKey, T::MaxDomainsPerAccount>,
		ValueQuery,
	>;

	/// Labels that cannot be claimed in any zone.
	#[pallet::storage]
	pub type ReservedLabels<T: Config> = StorageMap<_, Blake2_128Concat, Label, ()>;

	/// Labels that cannot be claimed in one zone.
	#[pallet::storage]
	pub type BannedLabels<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, ZoneId, Blake2_128Concat, Label, ()>;

	/// Hosts the relays refuse; an entry also covers all its subdomains.
	#[pallet::storage]
	pub type DeniedHosts<T: Config> = StorageMap<_, Blake2_128Concat, Host, ()>;

	/// Transfers proposed to an account, keyed by the proposed new owner.
	#[pallet::storage]
	pub type ProposedTransfers<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, DomainKey, ()>;

	/// Deployers the relays refuse; they also cannot claim or receive domains.
	#[pallet::storage]
	pub type DeniedDeployers<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ()>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DomainClaimed {
			zone_id: ZoneId,
			label: Label,
			owner: T::AccountId,
		},
		DomainReleased {
			zone_id: ZoneId,
			label: Label,
			owner: T::AccountId,
		},
		DomainTransferProposed {
			zone_id: ZoneId,
			label: Label,
			from: T::AccountId,
			to: T::AccountId,
		},
		DomainTransferRejected {
			zone_id: ZoneId,
			label: Label,
			to: T::AccountId,
		},
		DomainTransferred {
			zone_id: ZoneId,
			label: Label,
			from: T::AccountId,
			to: T::AccountId,
		},
		ZoneAdded {
			zone_id: ZoneId,
			suffix: Suffix,
			deposit: BalanceFor<T>,
		},
		ZoneUpdated {
			zone_id: ZoneId,
			deposit: BalanceFor<T>,
			active: bool,
		},
		ZoneRemoved {
			zone_id: ZoneId,
		},
		ReservedLabelsUpdated,
		BannedLabelsUpdated {
			zone_id: ZoneId,
		},
		DeniedHostsUpdated,
		DeniedDeployersUpdated,
	}

	#[pallet::error]
	pub enum Error<T> {
		/// No zone exists with the given id.
		ZoneNotFound,
		/// The zone does not accept new claims.
		ZoneInactive,
		/// The zone still has claimed or banned labels.
		ZoneNotEmpty,
		/// All zone ids are used.
		TooManyZones,
		/// Another zone already uses this suffix.
		SuffixAlreadyRegistered,
		/// The suffix is not a valid lowercase DNS name.
		InvalidSuffix,
		/// The host is not a valid lowercase DNS name.
		InvalidHost,
		/// The label breaks the label format rules.
		InvalidLabel,
		/// The label is reserved in all zones.
		LabelReserved,
		/// The label is banned in this zone.
		LabelBanned,
		/// The label is already claimed in this zone.
		DomainAlreadyClaimed,
		/// The label is not claimed in this zone.
		DomainNotFound,
		/// The caller does not own the domain.
		NotDomainOwner,
		/// The account already owns the maximum number of domains.
		TooManyDomains,
		/// The account is on the denied deployers list.
		DeployerDenied,
		/// The owner proposed a transfer to itself.
		TransferToSelf,
		/// The domain has no pending transfer to the caller.
		NoPendingTransfer,
		/// The claimed name is the suffix of a registered zone.
		NameIsZone,
		/// The new zone's suffix is a name already claimed in its parent zone.
		SuffixClaimed,
		/// The caller is neither the owner nor the proposed recipient.
		NotTransferParty,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Claims `label` in `zone_id` for the caller, holding the zone's deposit.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::claim())]
		pub fn claim(origin: OriginFor<T>, zone_id: ZoneId, label: Label) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!DeniedDeployers::<T>::contains_key(&who), Error::<T>::DeployerDenied);
			let zone = Zones::<T>::get(zone_id).ok_or(Error::<T>::ZoneNotFound)?;
			ensure!(zone.active, Error::<T>::ZoneInactive);
			ensure!(is_valid_label(&label), Error::<T>::InvalidLabel);
			ensure!(!ReservedLabels::<T>::contains_key(&label), Error::<T>::LabelReserved);
			ensure!(!BannedLabels::<T>::contains_key(zone_id, &label), Error::<T>::LabelBanned);
			ensure!(
				!Reservations::<T>::contains_key(zone_id, &label),
				Error::<T>::DomainAlreadyClaimed
			);
			let name = [&label[..], b".", &zone.suffix[..]].concat();
			ensure!(
				Suffix::try_from(name).map_or(true, |name| !ZoneIdOf::<T>::contains_key(name)),
				Error::<T>::NameIsZone
			);

			Self::add_domain_of(&who, (zone_id, label.clone()))?;
			T::Currency::hold(&HoldReason::DomainDeposit.into(), &who, zone.deposit)?;
			Reservations::<T>::insert(
				zone_id,
				&label,
				Reservation {
					owner: who.clone(),
					deposit: zone.deposit,
					registered_at: frame_system::Pallet::<T>::block_number(),
					pending_owner: None,
				},
			);

			Self::deposit_event(Event::DomainClaimed { zone_id, label, owner: who });
			Ok(())
		}

		/// Releases a domain owned by the caller and refunds its deposit.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::release())]
		pub fn release(origin: OriginFor<T>, zone_id: ZoneId, label: Label) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let reservation =
				Reservations::<T>::take(zone_id, &label).ok_or(Error::<T>::DomainNotFound)?;
			ensure!(reservation.owner == who, Error::<T>::NotDomainOwner);
			Self::do_release(zone_id, label, reservation)
		}

		/// Proposes `new_owner` as the owner of a domain owned by the caller. Replaces any earlier
		/// proposal for that domain. Ownership moves only once `new_owner` calls `accept_transfer`.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::propose_transfer())]
		pub fn propose_transfer(
			origin: OriginFor<T>,
			zone_id: ZoneId,
			label: Label,
			new_owner: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who != new_owner, Error::<T>::TransferToSelf);
			ensure!(
				!DeniedDeployers::<T>::contains_key(&who)
					&& !DeniedDeployers::<T>::contains_key(&new_owner),
				Error::<T>::DeployerDenied
			);
			Reservations::<T>::try_mutate(zone_id, &label, |maybe_reservation| {
				let reservation = maybe_reservation.as_mut().ok_or(Error::<T>::DomainNotFound)?;
				ensure!(reservation.owner == who, Error::<T>::NotDomainOwner);
				let key = (zone_id, label.clone());
				if let Some(previous) = reservation.pending_owner.replace(new_owner.clone()) {
					ProposedTransfers::<T>::remove(previous, &key);
				}
				ProposedTransfers::<T>::insert(&new_owner, key, ());
				Ok::<_, Error<T>>(())
			})?;

			Self::deposit_event(Event::DomainTransferProposed {
				zone_id,
				label,
				from: who,
				to: new_owner,
			});
			Ok(())
		}

		/// Adds a zone under the next free id, emitted in `ZoneAdded`.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::add_zone())]
		pub fn add_zone(
			origin: OriginFor<T>,
			suffix: Suffix,
			deposit: BalanceFor<T>,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			ensure!(is_valid_host(&suffix), Error::<T>::InvalidSuffix);
			ensure!(!ZoneIdOf::<T>::contains_key(&suffix), Error::<T>::SuffixAlreadyRegistered);
			if let Some(dot) = suffix.iter().position(|c| *c == b'.') {
				let parent = Suffix::truncate_from(suffix[dot + 1..].to_vec());
				let first = Label::truncate_from(suffix[..dot].to_vec());
				ensure!(
					ZoneIdOf::<T>::get(parent).is_none_or(|parent_id| {
						!Reservations::<T>::contains_key(parent_id, first)
					}),
					Error::<T>::SuffixClaimed
				);
			}
			let zone_id = NextZoneId::<T>::get();
			NextZoneId::<T>::put(zone_id.checked_add(1).ok_or(Error::<T>::TooManyZones)?);

			ZoneIdOf::<T>::insert(&suffix, zone_id);
			Zones::<T>::insert(zone_id, Zone { suffix: suffix.clone(), deposit, active: true });

			Self::deposit_event(Event::ZoneAdded { zone_id, suffix, deposit });
			Ok(Pays::No.into())
		}

		/// Changes a zone's deposit or active flag. Existing claims keep their deposit.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::update_zone())]
		pub fn update_zone(
			origin: OriginFor<T>,
			zone_id: ZoneId,
			deposit: Option<BalanceFor<T>>,
			active: Option<bool>,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			let (deposit, active) = Zones::<T>::try_mutate(zone_id, |maybe_zone| {
				let zone = maybe_zone.as_mut().ok_or(Error::<T>::ZoneNotFound)?;
				if let Some(deposit) = deposit {
					zone.deposit = deposit;
				}
				if let Some(active) = active {
					zone.active = active;
				}
				Ok::<_, Error<T>>((zone.deposit, zone.active))
			})?;

			Self::deposit_event(Event::ZoneUpdated { zone_id, deposit, active });
			Ok(Pays::No.into())
		}

		/// Removes a zone that has no claims and no banned labels left.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::remove_zone())]
		pub fn remove_zone(origin: OriginFor<T>, zone_id: ZoneId) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			ensure!(
				Reservations::<T>::iter_key_prefix(zone_id).next().is_none()
					&& BannedLabels::<T>::iter_key_prefix(zone_id).next().is_none(),
				Error::<T>::ZoneNotEmpty
			);
			let zone = Zones::<T>::take(zone_id).ok_or(Error::<T>::ZoneNotFound)?;
			ZoneIdOf::<T>::remove(&zone.suffix);

			Self::deposit_event(Event::ZoneRemoved { zone_id });
			Ok(Pays::No.into())
		}

		/// Adds a label to or removes it from the global reserved list. Reserving blocks new claims only.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::update_reserved_labels())]
		pub fn update_reserved_labels(
			origin: OriginFor<T>,
			update: ListUpdate<Label>,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			match update.operation {
				ListUpdateOperation::Add => {
					ensure!(is_valid_label(&update.item), Error::<T>::InvalidLabel);
					ReservedLabels::<T>::insert(&update.item, ());
				},
				ListUpdateOperation::Remove => ReservedLabels::<T>::remove(&update.item),
			}

			Self::deposit_event(Event::ReservedLabelsUpdated);
			Ok(Pays::No.into())
		}

		/// Bans or unbans a label in `zone_id`. Banning force-releases an existing claim.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::update_banned_labels())]
		pub fn update_banned_labels(
			origin: OriginFor<T>,
			zone_id: ZoneId,
			update: ListUpdate<Label>,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			match update.operation {
				ListUpdateOperation::Add => {
					ensure!(is_valid_label(&update.item), Error::<T>::InvalidLabel);
					if let Some(reservation) = Reservations::<T>::take(zone_id, &update.item) {
						Self::do_release(zone_id, update.item.clone(), reservation)?;
					}
					BannedLabels::<T>::insert(zone_id, &update.item, ());
				},
				ListUpdateOperation::Remove => BannedLabels::<T>::remove(zone_id, &update.item),
			}

			Self::deposit_event(Event::BannedLabelsUpdated { zone_id });
			Ok(Pays::No.into())
		}

		/// Releases any domain and refunds its owner.
		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::force_release())]
		pub fn force_release(
			origin: OriginFor<T>,
			zone_id: ZoneId,
			label: Label,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			let reservation =
				Reservations::<T>::take(zone_id, &label).ok_or(Error::<T>::DomainNotFound)?;
			Self::do_release(zone_id, label, reservation)?;
			Ok(Pays::No.into())
		}

		/// Adds a host to or removes it from the relay deny list.
		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::update_denied_hosts())]
		pub fn update_denied_hosts(
			origin: OriginFor<T>,
			update: ListUpdate<Host>,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			match update.operation {
				ListUpdateOperation::Add => {
					ensure!(is_valid_host(&update.item), Error::<T>::InvalidHost);
					DeniedHosts::<T>::insert(&update.item, ());
				},
				ListUpdateOperation::Remove => DeniedHosts::<T>::remove(&update.item),
			}

			Self::deposit_event(Event::DeniedHostsUpdated);
			Ok(Pays::No.into())
		}

		/// Adds an account to or removes it from the deployer deny list.
		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::update_denied_deployers())]
		pub fn update_denied_deployers(
			origin: OriginFor<T>,
			update: ListUpdate<T::AccountId>,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			match update.operation {
				ListUpdateOperation::Add => DeniedDeployers::<T>::insert(&update.item, ()),
				ListUpdateOperation::Remove => DeniedDeployers::<T>::remove(&update.item),
			}

			Self::deposit_event(Event::DeniedDeployersUpdated);
			Ok(Pays::No.into())
		}

		/// Accepts a pending transfer to the caller, refunding the previous owner's deposit and
		/// holding the zone's current deposit from the caller.
		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::accept_transfer())]
		pub fn accept_transfer(
			origin: OriginFor<T>,
			zone_id: ZoneId,
			label: Label,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let zone = Zones::<T>::get(zone_id).ok_or(Error::<T>::ZoneNotFound)?;
			let from = Reservations::<T>::try_mutate(zone_id, &label, |maybe_reservation| {
				let reservation = maybe_reservation.as_mut().ok_or(Error::<T>::DomainNotFound)?;
				ensure!(
					reservation.pending_owner.take().as_ref() == Some(&who),
					Error::<T>::NoPendingTransfer
				);
				ensure!(
					!DeniedDeployers::<T>::contains_key(&reservation.owner)
						&& !DeniedDeployers::<T>::contains_key(&who),
					Error::<T>::DeployerDenied
				);
				T::Currency::release(
					&HoldReason::DomainDeposit.into(),
					&reservation.owner,
					reservation.deposit,
					Precision::BestEffort,
				)?;
				T::Currency::hold(&HoldReason::DomainDeposit.into(), &who, zone.deposit)?;
				reservation.deposit = zone.deposit;
				Ok::<_, DispatchError>(core::mem::replace(&mut reservation.owner, who.clone()))
			})?;
			let key = (zone_id, label.clone());
			ProposedTransfers::<T>::remove(&who, &key);
			Self::remove_domain_of(&from, &key);
			Self::add_domain_of(&who, key)?;

			Self::deposit_event(Event::DomainTransferred { zone_id, label, from, to: who });
			Ok(())
		}

		/// Drops a pending transfer. Callable by the proposed new owner or the current owner.
		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::reject_transfer())]
		pub fn reject_transfer(
			origin: OriginFor<T>,
			zone_id: ZoneId,
			label: Label,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let to = Reservations::<T>::try_mutate(zone_id, &label, |maybe_reservation| {
				let reservation = maybe_reservation.as_mut().ok_or(Error::<T>::DomainNotFound)?;
				let to = reservation.pending_owner.take().ok_or(Error::<T>::NoPendingTransfer)?;
				ensure!(to == who || reservation.owner == who, Error::<T>::NotTransferParty);
				Ok::<_, Error<T>>(to)
			})?;
			let key = (zone_id, label);
			ProposedTransfers::<T>::remove(&to, &key);

			Self::deposit_event(Event::DomainTransferRejected { zone_id, label: key.1, to });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Refunds the owner of a reservation already taken out of `Reservations` and cleans up.
		fn do_release(
			zone_id: ZoneId,
			label: Label,
			reservation: ReservationFor<T>,
		) -> DispatchResult {
			let owner = reservation.owner;
			T::Currency::release(
				&HoldReason::DomainDeposit.into(),
				&owner,
				reservation.deposit,
				Precision::BestEffort,
			)?;
			let key = (zone_id, label);
			if let Some(pending_owner) = reservation.pending_owner {
				ProposedTransfers::<T>::remove(pending_owner, &key);
			}
			Self::remove_domain_of(&owner, &key);

			Self::deposit_event(Event::DomainReleased { zone_id, label: key.1, owner });
			Ok(())
		}

		fn add_domain_of(owner: &T::AccountId, key: DomainKey) -> DispatchResult {
			DomainsOf::<T>::try_mutate(owner, |domains| {
				domains.try_push(key).map_err(|_| Error::<T>::TooManyDomains.into())
			})
		}

		fn remove_domain_of(owner: &T::AccountId, key: &DomainKey) {
			DomainsOf::<T>::mutate_exists(owner, |maybe_domains| {
				if let Some(domains) = maybe_domains {
					domains.retain(|d| d != key);
					if domains.is_empty() {
						*maybe_domains = None;
					}
				}
			});
		}
	}
}
