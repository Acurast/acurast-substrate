use sails_client_gen::ClientGenerator;
use std::{env, path::PathBuf};

use vara_ibc::VaraIbcProgram as ProgramType;

fn main() {
	let idl_file_path = PathBuf::from("vara_ibc.idl");
	// Generate IDL file for the program
	sails_idl_gen::generate_idl_to_file::<ProgramType>(&idl_file_path).unwrap();
	// Generate client code from IDL file
	ClientGenerator::from_idl_path(&idl_file_path)
		.with_mocks("mocks")
		.generate_to(PathBuf::from(env::var("OUT_DIR").unwrap()).join("vara_ibc_client.rs"))
		.unwrap();
}
