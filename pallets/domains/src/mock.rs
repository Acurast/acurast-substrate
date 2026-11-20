use frame_support::{derive_impl, parameter_types, traits::ConstU32};
use frame_system::EnsureRoot;
use sp_runtime::{traits::IdentityLookup, AccountId32, BuildStorage};

pub type AccountId = AccountId32;
pub type Balance = u128;
type Block = frame_system::mocking::MockBlock<Test>;

pub const UNIT: Balance = 1_000_000;
pub const EXISTENTIAL_DEPOSIT: Balance = UNIT / 1_000;
pub const DEPOSIT: Balance = 10 * UNIT;
pub const INITIAL_BALANCE: Balance = 100 * UNIT;

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const CHARLIE: AccountId = AccountId32::new([3u8; 32]);
pub const POOR: AccountId = AccountId32::new([4u8; 32]);

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		AcurastDomains: crate,
	}
);

parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const MaxDomainsPerAccount: u32 = 3;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type Balance = Balance;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MaxFreezes = ConstU32<1>;
}

impl crate::Config for Test {
	type RuntimeHoldReason = RuntimeHoldReason;
	type Currency = Balances;
	type UpdateOrigin = EnsureRoot<AccountId>;
	type MaxDomainsPerAccount = MaxDomainsPerAccount;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(ALICE, INITIAL_BALANCE),
			(BOB, INITIAL_BALANCE),
			(CHARLIE, INITIAL_BALANCE),
			(POOR, DEPOSIT / 2),
		],
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
