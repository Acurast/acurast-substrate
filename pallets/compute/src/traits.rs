use frame_support::pallet_prelude::Weight;

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
	fn commit_compute() -> Weight;
	fn stake_more() -> Weight;
	fn cooldown_compute_commitment() -> Weight;
	fn end_compute_commitment() -> Weight;
	fn reward() -> Weight;
	fn slash() -> Weight;
	fn force_end_commitment() -> Weight;
	fn force_clear_staking_pools() -> Weight;
	fn withdraw_delegation() -> Weight;
	fn withdraw_commitment() -> Weight;
	fn delegate_more() -> Weight;
	fn compound_delegation() -> Weight;
	fn compound_stake() -> Weight;
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

	fn commit_compute() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn stake_more() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn cooldown_compute_commitment() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn end_compute_commitment() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn reward() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn slash() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn force_end_commitment() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn force_clear_staking_pools() -> Weight {
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
}
