#[cfg(test)]
pub mod relay_chain;

pub use acurast_rococo_runtime as acurast_runtime;

pub use xcm_simulator;

extern crate core;

#[cfg(test)]
mod tests {

	use super::*;
	use hex_literal::hex;
	use sp_runtime::{
		traits::{AccountIdConversion, ConstU32},
		AccountId32, BoundedVec, BuildStorage,
	};
	use xcm_simulator::{decl_test_parachain, ParaId, TestExt};

	use crate::acurast_runtime::{
		pallet_acurast_marketplace::{Advertisement, FeeManager, Pricing, SchedulingWindow},
		AccountId, Balance, FeeManagement,
	};
	// parent re-exports
	use crate::acurast_runtime::{
		pallet_acurast, pallet_acurast_marketplace,
		pallet_acurast_marketplace::ExecutionOperationHash,
	};
	use acurast_rococo_runtime::pallet_acurast::{
		Attestation, AttestationSecurityLevel, AttestationValidity,
		BoundedAttestationApplicationId, BoundedAttestationPackageInfo, BoundedAuthorizationList,
		BoundedKeyDescription, BoundedRootOfTrust, PackageInfoSet, VerifiedBootState,
	};
	use xcm_simulator::{self, decl_test_network, decl_test_relay_chain};

	mod jobs;

	decl_test_relay_chain! {
		pub struct Relay {
			Runtime = crate::relay_chain::Runtime,
			RuntimeCall = crate::relay_chain::RuntimeCall,
			RuntimeEvent = crate::relay_chain::RuntimeEvent,
			XcmConfig = crate::relay_chain::xcm_config::XcmConfig,
			MessageQueue = crate::relay_chain::MessageQueue,
			System = crate::relay_chain::System,
			new_ext = polkadot_ext(),
		}
	}

	decl_test_parachain! {
		pub struct AcurastParachain {
			Runtime = acurast_runtime::Runtime,
			XcmpMessageHandler = acurast_runtime::XcmpQueue,
			DmpMessageHandler = acurast_runtime::DmpQueue,
			new_ext = acurast_ext(ACURAST_CHAIN_ID),
		}
	}

	decl_test_network! {
		pub struct Network {
			relay_chain = Relay,
			parachains = vec![
				(2001, AcurastParachain),
			],
		}
	}

	// make this match parachains in decl_test_network!
	pub const ACURAST_CHAIN_ID: u32 = 2001;

	pub const ALICE: AccountId32 = AccountId32::new([4u8; 32]);
	pub const BOB: AccountId32 = AccountId32::new([8u8; 32]);
	pub const FERDIE: AccountId32 = AccountId32::new([5u8; 32]);

	pub const INITIAL_BALANCE: u128 = 1_000_000_000_000_000;

	pub fn acurast_ext(para_id: u32) -> sp_io::TestExternalities {
		use crate::acurast_runtime::{Runtime, System};

		let mut t = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();

		parachain_info::GenesisConfig::<Runtime> {
			parachain_id: ParaId::from(para_id),
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let pallet_acurast_account: <Runtime as frame_system::Config>::AccountId =
			<Runtime as acurast_runtime::pallet_acurast::Config>::PalletId::get()
				.into_account_truncating();

		let fee_manager_account: <Runtime as frame_system::Config>::AccountId =
			acurast_runtime::FeeManagerPalletId::get().into_account_truncating();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: vec![
				(ALICE, INITIAL_BALANCE),
				(BOB, INITIAL_BALANCE),
				(FERDIE, INITIAL_BALANCE),
				(pallet_acurast_account, INITIAL_BALANCE),
				(fee_manager_account, INITIAL_BALANCE),
			],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_xcm::GenesisConfig::<Runtime>::default().build_storage().unwrap();
		acurast_runtime::pallet_acurast::GenesisConfig::<Runtime> {
			attestations: vec![(BOB, Some(Attestation {
				cert_ids: vec![
					(
						hex!("301b311930170603550405131066393230303965383533623662303435").to_vec().try_into().unwrap(),
						hex!("00e8fa196314d2fa18").to_vec().try_into().unwrap()
					),
					(
						hex!("301b311930170603550405131066393230303965383533623662303435").to_vec().try_into().unwrap(),
						hex!("038826676065899685f5").to_vec().try_into().unwrap()
					),
					(
						hex!("302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f78").to_vec().try_into().unwrap(),
						hex!("15905857467176635834").to_vec().try_into().unwrap()
					),
					(
						hex!("302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f78").to_vec().try_into().unwrap(),
						hex!("01").to_vec().try_into().unwrap()
					),
				].try_into().unwrap(),
				key_description: BoundedKeyDescription {
					attestation_security_level: AttestationSecurityLevel::StrongBox,
					key_mint_security_level: AttestationSecurityLevel::StrongBox,
					software_enforced: BoundedAuthorizationList {
						purpose: None,
						algorithm: None,
						key_size: None,
						digest: None,
						padding: None,
						ec_curve: None,
						rsa_public_exponent: None,
						mgf_digest: None,
						rollback_resistance: None,
						early_boot_only: None,
						active_date_time: None,
						origination_expire_date_time: None,
						usage_expire_date_time: None,
						usage_count_limit: None,
						no_auth_required: false,
						user_auth_type: None,
						auth_timeout: None,
						allow_while_on_body: false,
						trusted_user_presence_required: None,
						trusted_confirmation_required: None,
						unlocked_device_required: None,
						all_applications: None,
						application_id: None,
						creation_date_time: Some(1701938381112),
						origin: None,
						root_of_trust: None,
						os_version: None,
						os_patch_level: None,
						attestation_application_id: Some(BoundedAttestationApplicationId {
							package_infos: vec![
								BoundedAttestationPackageInfo {
									package_name: b"com.acurast.attested.executor.testnet".to_vec().try_into().unwrap(),
									version: 26,
								}
							].try_into().unwrap(),
							signature_digests: vec![
								hex!("ec70c2a4e072a0f586552a68357b23697c9d45f1e1257a8c4d29a25ac4982433").to_vec().try_into().unwrap()
							].try_into().unwrap(),
						}),
						attestation_id_brand: None,
						attestation_id_device: None,
						attestation_id_product: None,
						attestation_id_serial: None,
						attestation_id_imei: None,
						attestation_id_meid: None,
						attestation_id_manufacturer: None,
						attestation_id_model: None,
						vendor_patch_level: None,
						boot_patch_level: None,
						device_unique_attestation: None,
					},
					tee_enforced: BoundedAuthorizationList {
						purpose: Some(hex!("0203").to_vec().try_into().unwrap()),
						algorithm: Some(3),
						key_size: Some(256),
						digest: None,
						padding: None,
						ec_curve: None,
						rsa_public_exponent: None,
						mgf_digest: None,
						rollback_resistance: None,
						early_boot_only: None,
						active_date_time: None,
						origination_expire_date_time: None,
						usage_expire_date_time: None,
						usage_count_limit: None,
						no_auth_required: false,
						user_auth_type: None,
						auth_timeout: None,
						allow_while_on_body: false,
						trusted_user_presence_required: None,
						trusted_confirmation_required: None,
						unlocked_device_required: None,
						all_applications: None,
						application_id: None,
						creation_date_time: None,
						origin: None,
						root_of_trust: Some(BoundedRootOfTrust {
							verified_boot_key: hex!("879cd3f18ea76e244d4d4ac3bcb9c337c13b4667190b19035afe2536550050f1").to_vec().try_into().unwrap(),
							device_locked: true,
							verified_boot_state: VerifiedBootState::Verified,
							verified_boot_hash: Some(hex!("63293a162d3058e555ac5bf910164b8dce7b62e1aa924a58f33aacece4be3ca4").to_vec().try_into().unwrap()),
						}),
						os_version: None,
						os_patch_level: None,
						attestation_application_id: None,
						attestation_id_brand: None,
						attestation_id_device: None,
						attestation_id_product: None,
						attestation_id_serial: None,
						attestation_id_imei: None,
						attestation_id_meid: None,
						attestation_id_manufacturer: None,
						attestation_id_model: None,
						vendor_patch_level: None,
						boot_patch_level: None,
						device_unique_attestation: None,
					},
				},
				validity: AttestationValidity {
					not_before: 0,
					not_after: 1842739199000,
				},
			}))],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		acurast_runtime::pallet_acurast_processor_manager::GenesisConfig::<Runtime> {
			managers: vec![(ALICE, vec![BOB])],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}

	type Acurast = pallet_acurast::Pallet<acurast_runtime::Runtime>;
	type AcurastMarketplace = pallet_acurast_marketplace::Pallet<acurast_runtime::Runtime>;
	type AcurastBalances = pallet_balances::Pallet<acurast_runtime::Runtime>;

	/// Type representing the utf8 bytes of a string containing the value of an ipfs url.
	/// The ipfs url is expected to point to a script.
	pub type Script = BoundedVec<u8, ConstU32<53>>;

	const SCRIPT_BYTES: [u8; 53] = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");
	pub const OPERATION_HASH: [u8; 32] =
		hex!("a3f18e4c6f0cdd0d8666f407610351cacb9a263678cf058294be9977b69f2cb3");

	pub fn script() -> Script {
		SCRIPT_BYTES.to_vec().try_into().unwrap()
	}

	pub fn operation_hash() -> ExecutionOperationHash {
		OPERATION_HASH.to_vec().try_into().unwrap()
	}

	pub fn advertisement(
		fee_per_millisecond: u128,
		fee_per_storage_byte: u128,
		storage_capacity: u32,
		max_memory: u32,
		network_request_quota: u8,
		scheduling_window: SchedulingWindow,
	) -> Advertisement<AccountId, Balance, pallet_acurast::CU32<100>> {
		Advertisement {
			pricing: Pricing {
				fee_per_millisecond,
				fee_per_storage_byte,
				base_fee_per_execution: 0,
				scheduling_window,
			},
			allowed_consumers: None,
			storage_capacity,
			max_memory,
			network_request_quota,
			available_modules: vec![].try_into().unwrap(),
		}
	}

	// add arg paras: Vec<u32>
	pub fn polkadot_ext() -> sp_io::TestExternalities {
		use crate::relay_chain::{Runtime, System};

		let t = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			System::set_block_number(1);
		});
		ext
	}
}
