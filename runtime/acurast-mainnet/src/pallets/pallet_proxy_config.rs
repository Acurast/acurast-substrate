use frame_support::{parameter_types, traits::InstanceFilter};
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use polkadot_core_primitives::BlakeTwo256;
use sp_core::RuntimeDebug;

use acurast_runtime_common::types::Balance;

use crate::{deposit, Balances, Runtime, RuntimeCall, RuntimeEvent, System};

parameter_types! {
	// One storage item; key size 32, value size 8.
	pub const ProxyDepositBase: Balance = deposit(1, 40);
	// Additional storage item size of 33 bytes.
	pub const ProxyDepositFactor: Balance = deposit(0, 33);
	pub const MaxProxies: u16 = 32;
	// One storage item; key size 32, value size 16.
	pub const AnnouncementDepositBase: Balance = deposit(1, 48);
	pub const AnnouncementDepositFactor: Balance = deposit(0, 66);
	pub const MaxPending: u16 = 32;
}

impl pallet_proxy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type MaxPending = MaxPending;
	type CallHasher = BlakeTwo256;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
	type BlockNumberProvider = System;
	type WeightInfo = pallet_proxy::weights::SubstrateWeight<Self>;
}

/// The type used to represent the kinds of proxying allowed.
#[derive(
	Copy,
	Clone,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	Encode,
	Decode,
	DecodeWithMemTracking,
	RuntimeDebug,
	MaxEncodedLen,
	scale_info::TypeInfo,
)]
pub enum ProxyType {
	/// Fully permissioned proxy. Can execute any call on behalf of _proxied_.
	Any,
	/// Can execute any call that does not transfer funds or assets.
	NonTransfer,
	/// Proxy for all Balances pallet calls.
	Balances,
	/// Proxy with the ability to reject time-delay proxy announcements.
	CancelProxy,
	/// Proxy for all ProcessorManager pallet calls.
	ProcessorManager,
	/// Collator selection proxy. Can execute calls related to collator selection mechanism.
	Collator,
}

impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}

impl InstanceFilter<RuntimeCall> for ProxyType {
	fn filter(&self, c: &RuntimeCall) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::NonTransfer => !matches!(c, RuntimeCall::Balances { .. }),
			ProxyType::Balances => {
				matches!(
					c,
					RuntimeCall::Balances { .. }
						| RuntimeCall::Utility { .. }
						| RuntimeCall::Multisig { .. }
				)
			},
			ProxyType::CancelProxy => matches!(
				c,
				RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement { .. })
					| RuntimeCall::Utility { .. }
					| RuntimeCall::Multisig { .. }
			),
			ProxyType::ProcessorManager => matches!(c, RuntimeCall::AcurastProcessorManager { .. }),
			ProxyType::Collator => matches!(
				c,
				RuntimeCall::CollatorSelection { .. }
					| RuntimeCall::Utility { .. }
					| RuntimeCall::Multisig { .. }
			),
		}
	}

	fn is_superset(&self, o: &Self) -> bool {
		match (self, o) {
			(x, y) if x == y => true,
			(ProxyType::Any, _) => true,
			(_, ProxyType::Any) => false,
			(ProxyType::NonTransfer, ProxyType::ProcessorManager) => true,
			(ProxyType::NonTransfer, ProxyType::Collator) => true,
			(ProxyType::NonTransfer, ProxyType::CancelProxy) => true,
			_ => false,
		}
	}
}
