use gstd::ActorId;
use sails_rs::{calls::*, gtest::calls::*};

use vara_ibc_client::traits::*;

const ACTOR_ID: u64 = 42;

#[tokio::test]
async fn do_something_works() {
	let remoting = GTestRemoting::new(ACTOR_ID.into());
	remoting.system().init_logger();

	// Submit program code into the system
	let program_code_id = remoting.system().submit_code(vara_ibc::WASM_BINARY);

	let program_factory = vara_ibc_client::VaraIbcFactory::new(remoting.clone());

	let program_id = program_factory
		.new(ActorId::from([
			24, 90, 139, 95, 146, 236, 211, 72, 237, 155, 18, 160, 71, 202, 43, 40, 72, 139, 19,
			152, 6, 90, 141, 255, 141, 207, 136, 98, 69, 249, 40, 11,
		])) // Call program's constructor (see src/lib.rs:27)
		.send_recv(program_code_id, b"salt")
		.await
		.unwrap();

	let mut service_client = vara_ibc_client::VaraIbc::new(remoting.clone());

	//let result = service_client
	//	.config() // Call service's method (see src/lib.rs:17)
	//	.send_recv(program_id)
	//	.await
	//	.unwrap();

	//assert_eq!(result, "Hello from VaraIbc!".to_string());
}
