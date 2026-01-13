use frame_support::pallet_prelude::Weight;

pub trait BlockAuthorProvider<AccountId> {
	fn author() -> Option<AccountId>;
}

pub trait WeightInfo {
	fn create_pool(x: u32) -> Weight;
	fn modify_pool_same_config() -> Weight;
	fn modify_pool_replace_config(x: u32) -> Weight;
	fn modify_pool_update_config(x: u32) -> Weight;
	fn offer_backing() -> Weight;
	fn withdraw_backing_offer() -> Weight;
	fn accept_backing_offer() -> Weight;
	fn delegate() -> Weight;
	fn cooldown_delegation() -> Weight;
	fn redelegate() -> Weight;
	fn end_delegation() -> Weight;
	fn commit_compute(x: u32) -> Weight;
	fn stake_more(x: u32) -> Weight;
	fn cooldown_compute_commitment() -> Weight;
	fn end_compute_commitment() -> Weight;
	fn kick_out() -> Weight;
	fn slash() -> Weight;
	fn withdraw_delegation() -> Weight;
	fn withdraw_commitment() -> Weight;
	fn delegate_more() -> Weight;
	fn compound_delegation() -> Weight;
	fn compound_stake() -> Weight;
	fn enable_inflation() -> Weight;
}

impl WeightInfo for () {
	fn create_pool(_x: u32) -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn modify_pool_same_config() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn modify_pool_replace_config(_x: u32) -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn modify_pool_update_config(_x: u32) -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn offer_backing() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn withdraw_backing_offer() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn accept_backing_offer() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn delegate() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn cooldown_delegation() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn redelegate() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn end_delegation() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn commit_compute(_x: u32) -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn stake_more(_x: u32) -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn cooldown_compute_commitment() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn end_compute_commitment() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn kick_out() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn slash() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn withdraw_delegation() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn withdraw_commitment() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn delegate_more() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn compound_delegation() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn compound_stake() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn enable_inflation() -> Weight {
		Weight::from_parts(10_000, 0)
	}
}
