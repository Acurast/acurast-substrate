use std::marker::PhantomData;

use frame_support::{
	derive_impl, parameter_types,
	sp_runtime::{
		traits::{ConstU16, ConstU32, ConstU64, IdentityLookup},
		BuildStorage,
	},
};
use sp_arithmetic::Perbill;
use sp_core::H256;
use sp_runtime::DispatchError;
use sp_std::prelude::*;

use crate::{stub::*, *};

type Block = frame_system::mocking::MockBlock<Test>;

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {}
	}
}

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>} = 0,
		AcurastVesting: crate::{Pallet, Call, Storage, Event<T>},
		MockPallet: mock_pallet::{Pallet, Event<T>}
	}
);

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Nonce = u64;
	type Hash = H256;
	type Block = Block;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_types! {
	pub const DivestTolerance: BlockNumber = 2;
	pub const MaximumLockingPeriod: BlockNumber = 100;
	pub const BalanceUnit: u128 = UNIT;
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type DivestTolerance = DivestTolerance;
	type MaximumLockingPeriod = MaximumLockingPeriod;
	type Balance = Balance;
	type BalanceUnit = BalanceUnit;
	type BlockNumber = BlockNumber;
	type VestingBalance = MockVestingBalance<Self>;
	type WeightInfo = ();
}

impl mock_pallet::Config for Test {
	type RuntimeEvent = RuntimeEvent;
}

#[frame_support::pallet]
pub mod mock_pallet {
	use frame_support::pallet_prelude::*;
	use sp_arithmetic::Perbill;

	#[pallet::config]
	pub trait Config: frame_system::Config + crate::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		LockStake(T::AccountId, T::Balance),
		PayAccrued(T::AccountId, T::Balance),
		PayKicker(T::AccountId, T::Balance),
		UnlockStake(T::AccountId, T::Balance),
		PowerDecreased(T::AccountId, Perbill),
		PowerIncreased(T::AccountId, Perbill),
	}
}

pub struct MockVestingBalance<T>(PhantomData<T>);

impl<T: Config + mock_pallet::Config> VestingBalance<T::AccountId, T::Balance>
	for MockVestingBalance<T>
{
	fn lock_stake(
		target: &T::AccountId,
		stake: <T as Config>::Balance,
	) -> Result<(), DispatchError> {
		mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::LockStake(
			target.clone(),
			stake,
		));
		Ok(())
	}

	fn adjust_lock(_acc: &T::AccountId, _stake: T::Balance) {}

	fn pay_accrued(
		target: &T::AccountId,
		accrued: <T as Config>::Balance,
	) -> Result<(), DispatchError> {
		mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::PayAccrued(
			target.clone(),
			accrued,
		));
		Ok(())
	}

	fn pay_kicker(
		target: &T::AccountId,
		accrued: <T as Config>::Balance,
	) -> Result<(), DispatchError> {
		mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::PayKicker(
			target.clone(),
			accrued,
		));
		Ok(())
	}

	fn unlock_stake(
		target: &T::AccountId,
		stake: <T as Config>::Balance,
	) -> Result<(), DispatchError> {
		mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::UnlockStake(
			target.clone(),
			stake,
		));
		Ok(())
	}

	fn power_decreased(target: &T::AccountId, perbill: Perbill) -> Result<(), DispatchError> {
		mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::PowerDecreased(
			target.clone(),
			perbill,
		));
		Ok(())
	}

	fn power_increased(
		target: &T::AccountId,
		reciprocal_perbill: Perbill,
	) -> Result<(), DispatchError> {
		mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::PowerIncreased(
			target.clone(),
			reciprocal_perbill,
		));
		Ok(())
	}
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}
