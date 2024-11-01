use core::marker::PhantomData;

use acurast_runtime_common::{weight, Balance};
use frame_support::{instances::Instance1, pallet_prelude::DispatchResultWithPostInfo};
use pallet_acurast_hyperdrive::ParsedAction;

use crate::{
	Acurast, AcurastAccountId, AcurastMarketplace, AcurastPalletAccount, AlephZeroContract,
	AlephZeroContractSelector, Runtime, RuntimeEvent, VaraContract,
};

impl pallet_acurast_hyperdrive::Config<Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ActionExecutor = AcurastActionExecutor<Runtime>;
	type Sender = AcurastPalletAccount;
	type ParsableAccountId = AcurastAccountId;
	type AlephZeroContract = AlephZeroContract;
	type AlephZeroContractSelector = AlephZeroContractSelector;
	type VaraContract = VaraContract;
	type Balance = Balance;
	type WeightInfo = weight::pallet_acurast_hyperdrive::WeightInfo<Runtime>;
}

pub struct AcurastActionExecutor<T: pallet_acurast::Config>(PhantomData<T>);
impl pallet_acurast_hyperdrive::ActionExecutor<Runtime> for AcurastActionExecutor<Runtime> {
	fn execute(action: ParsedAction<Runtime>) -> DispatchResultWithPostInfo {
		match action {
			ParsedAction::RegisterJob(job_id, registration) =>
				Acurast::register_for(job_id, registration.into()),
			ParsedAction::DeregisterJob(job_id) => Acurast::deregister_for(job_id).into(),
			ParsedAction::FinalizeJob(job_ids) =>
				AcurastMarketplace::finalize_jobs_for(job_ids.into_iter()),
			ParsedAction::SetJobEnvironment(job_id, environments) => {
				Acurast::set_environment_for(job_id, environments)?;
				Ok(().into())
			},
			ParsedAction::Noop => {
				// Intentionally, just logging it
				log::debug!("Received NOOP operation from hyperdrive");

				Ok(().into())
			},
		}
	}
}
