use app::Hyperdrive;
use sails_idl_gen::program;
use std::{env, path::PathBuf};

fn main() {
	gear_wasm_builder::build();

	program::generate_idl_to_file::<Hyperdrive>(
		PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("app.idl"),
	)
	.unwrap();
}
