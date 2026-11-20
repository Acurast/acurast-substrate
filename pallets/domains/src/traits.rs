use frame_support::weights::Weight;

pub trait WeightInfo {
	fn claim() -> Weight;
	fn release() -> Weight;
	fn propose_transfer() -> Weight;
	fn accept_transfer() -> Weight;
	fn reject_transfer() -> Weight;
	fn add_zone() -> Weight;
	fn update_zone() -> Weight;
	fn remove_zone() -> Weight;
	fn update_reserved_labels() -> Weight;
	fn update_banned_labels() -> Weight;
	fn force_release() -> Weight;
	fn update_denied_hosts() -> Weight;
	fn update_denied_deployers() -> Weight;
}

impl WeightInfo for () {
	fn claim() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn release() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn propose_transfer() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn accept_transfer() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn reject_transfer() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn add_zone() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn update_zone() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn remove_zone() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn update_reserved_labels() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn update_banned_labels() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn force_release() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn update_denied_hosts() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn update_denied_deployers() -> Weight {
		Weight::from_parts(10_000, 0)
	}
}
