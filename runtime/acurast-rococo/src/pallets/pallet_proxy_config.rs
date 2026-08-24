use frame_support::{parameter_types, traits::InstanceFilter};
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use polkadot_core_primitives::BlakeTwo256;

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
	Debug,
	MaxEncodedLen,
	scale_info::TypeInfo,
	Default,
)]
pub enum ProxyType {
	/// Fully permissioned proxy. Can execute any call on behalf of _proxied_.
	#[default]
	Any,
	/// Can execute only allowlisted calls that do not transfer funds or assets.
	NonTransfer,
	/// Proxy for all Balances pallet calls.
	Balances,
	/// Proxy with the ability to reject time-delay proxy announcements.
	CancelProxy,
	/// Proxy for ProcessorManager pallet calls, excluding fund-moving `recover_funds`.
	ProcessorManager,
	/// Collator selection proxy. Can execute calls related to collator selection mechanism.
	Collator,
}

impl InstanceFilter<RuntimeCall> for ProxyType {
	fn filter(&self, c: &RuntimeCall) -> bool {
		match self {
			ProxyType::Any => true,
			// Explicit allowlist of calls that cannot move funds or assets, modelled on
			// the upstream Polkadot/Kusama runtimes. `Utility` and `Multisig` are
			// deliberately excluded since they could smuggle a fund-moving call past
			// the filter. `Scheduler` dispatches with `EnsureCouncilOrRoot`, so a
			// signed proxy cannot use it to escalate.
			ProxyType::NonTransfer => {
				matches!(
					c,
					RuntimeCall::System { .. }
						| RuntimeCall::Scheduler { .. }
						| RuntimeCall::Preimage { .. }
						| RuntimeCall::Timestamp { .. }
						| RuntimeCall::Session { .. }
				) || ProxyType::ProcessorManager.filter(c)
			},
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
			// `recover_funds` sweeps a managed processor's entire usable balance to a
			// caller-chosen destination and must never be delegable via proxy.
			ProxyType::ProcessorManager => {
				matches!(c, RuntimeCall::AcurastProcessorManager { .. })
					&& !matches!(
						c,
						RuntimeCall::AcurastProcessorManager(
							pallet_acurast_processor_manager::Call::recover_funds { .. }
						)
					)
			},
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
			_ => false,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_runtime::MultiAddress;

	fn account() -> MultiAddress<crate::AccountId, ()> {
		MultiAddress::Id(sp_runtime::AccountId32::new([0u8; 32]))
	}

	fn recover_funds_call() -> RuntimeCall {
		RuntimeCall::AcurastProcessorManager(
			pallet_acurast_processor_manager::Call::recover_funds {
				processor: account(),
				destination: account(),
			},
		)
	}

	#[test]
	fn non_transfer_rejects_fund_moving_calls() {
		let calls = [
			RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
				dest: account(),
				value: 1,
			}),
			// Sweeps a managed processor's balance to a caller-chosen destination.
			recover_funds_call(),
			// `Utility::batch` could smuggle a fund-moving call past the filter.
			RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![] }),
		];
		for call in calls {
			assert!(!ProxyType::NonTransfer.filter(&call), "{call:?}");
		}
	}

	#[test]
	fn non_transfer_allows_allowlisted_calls() {
		let calls = [
			RuntimeCall::System(frame_system::Call::remark { remark: vec![] }),
			RuntimeCall::AcurastProcessorManager(
				pallet_acurast_processor_manager::Call::heartbeat {},
			),
		];
		for call in calls {
			assert!(ProxyType::NonTransfer.filter(&call), "{call:?}");
		}
	}

	#[test]
	fn processor_manager_excludes_recover_funds() {
		assert!(ProxyType::ProcessorManager.filter(&RuntimeCall::AcurastProcessorManager(
			pallet_acurast_processor_manager::Call::heartbeat {},
		)));
		assert!(!ProxyType::ProcessorManager.filter(&recover_funds_call()));
	}

	#[test]
	fn superset_reflects_filtered_calls() {
		assert!(ProxyType::Any.is_superset(&ProxyType::NonTransfer));
		assert!(ProxyType::NonTransfer.is_superset(&ProxyType::ProcessorManager));
		// The allowlisted `NonTransfer` no longer covers `Utility`/`Multisig`-based classes.
		assert!(!ProxyType::NonTransfer.is_superset(&ProxyType::Collator));
		assert!(!ProxyType::NonTransfer.is_superset(&ProxyType::CancelProxy));
		assert!(!ProxyType::NonTransfer.is_superset(&ProxyType::Balances));
		assert!(!ProxyType::NonTransfer.is_superset(&ProxyType::Any));
	}
}
