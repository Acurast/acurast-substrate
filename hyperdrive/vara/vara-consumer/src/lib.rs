#![no_std]

use gstd::{exec, msg, prelude::*};
use hex_literal::hex;

static mut MESSAGE_LOG: Vec<String> = vec![];

#[no_mangle]
extern "C" fn handle() {
	let new_msg: String = msg::load().expect("Unable to create string");

	// check source is devnet proxy contract
	// use this to check for canary proxy instead: 8d589e54da57f66fee61d3bc618ffa25d661d1306f4465da47591a907c7d616b
	if msg::source() !=
		hex!("008c7b8e8af22f221bf9872c47a749aac51dc3c374b1ce384f4a43f6a2883afb").into()
	{
		panic!("message source is not acurast proxy");
	}

	unsafe {
		MESSAGE_LOG.push(new_msg);
	}
}

#[no_mangle]
extern "C" fn state() {
	msg::reply(unsafe { MESSAGE_LOG.clone() }, 0)
		.expect("Failed to encode or reply with `<AppMetadata as Metadata>::State` from `state()`");
}

#[cfg(test)]
mod tests {
	extern crate std;

	use gstd::{Encode, String};
	use gtest::{Program, System};

	#[test]
	fn it_works() {
		let system = System::new();
		system.init_logger();
		system.mint_to(42, 100_000_000_000_000);

		let program = Program::current_opt(&system);

		let mid = program.send_bytes(42, "INIT");
		let res = system.run_next_block();
		assert!(res.succeed.contains(&mid));

		let mid = program.send_bytes(42, String::from("PING").encode());
		let res = system.run_next_block();
		assert!(res.succeed.contains(&mid));
		let log = &res.log[0];
		assert_eq!(log.source(), 1.into());
		assert_eq!(log.destination(), 42.into());
		assert_eq!(log.payload(), "PONG".as_bytes());
	}
}
