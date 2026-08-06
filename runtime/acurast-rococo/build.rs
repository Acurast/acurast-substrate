use substrate_wasm_builder::WasmBuilder;

fn main() {
	WasmBuilder::new()
		.with_current_project()
		.export_heap_base()
		.import_memory()
		// Generate the metadata hash (RFC-0078) so Ledger and other offline signers can verify
		// and display transactions. Must match the chain spec's token properties.
		.enable_metadata_hash("tACU", 12)
		.build()
}
