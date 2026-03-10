use sp_runtime::Weight;

pub trait WeightInfo {
	fn claim() -> Weight;
	fn vest() -> Weight;
	fn create_claim_type() -> Weight;
	fn update_claim_type() -> Weight;
	fn remove_claim_type() -> Weight;
	fn multi_claim() -> Weight;
	fn multi_vest() -> Weight;
}
