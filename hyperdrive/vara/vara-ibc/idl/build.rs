use std::path::PathBuf;

use vara_ibc::VaraIbcProgram as ProgramType;

fn main() {
	let idl_file_path = PathBuf::from("vara_ibc.idl");
	// Generate IDL file for the program
	sails_idl_gen::generate_idl_to_file::<ProgramType>(&idl_file_path).unwrap();
}
