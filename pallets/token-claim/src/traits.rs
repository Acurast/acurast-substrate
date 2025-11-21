use sp_runtime::Weight;

pub trait WeightInfo {
	fn claim() -> Weight;
}
