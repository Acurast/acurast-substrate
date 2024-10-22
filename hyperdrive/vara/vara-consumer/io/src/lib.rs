#![no_std]

use gmeta::{InOut, Metadata, Out};
use gstd::prelude::*;

pub struct VaraConsumerMetadata;

impl Metadata for VaraConsumerMetadata {
	type Init = ();
	type Handle = InOut<String, String>;
	type Others = ();
	type Reply = ();
	type Signal = ();
	type State = Out<Vec<String>>;
}
