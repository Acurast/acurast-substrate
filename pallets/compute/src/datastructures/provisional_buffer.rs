use core::cmp::Ordering;

use frame_support::pallet_prelude::*;
use sp_runtime::traits::Debug;

#[derive(
	RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Default,
)]
pub struct ProvisionalBuffer<Epoch: Ord + Debug, Value: Copy + Default + Debug> {
	current: Value,
	/// The next value and the epoch when the next value will be applied.
	next: Option<(Epoch, Value)>,
}

impl<Epoch: Ord + Debug, Value: Copy + Default + Debug> ProvisionalBuffer<Epoch, Value> {
	pub fn new(value: Value) -> Self {
		Self { current: value, next: None }
	}

	#[cfg(test)]
	pub fn from_inner(current: Value, next: Option<(Epoch, Value)>) -> Self {
		Self { current, next }
	}

	/// Sets the value for a specific epoch and subsequent epochs.
	pub fn set(&mut self, current_epoch: Epoch, epoch: Epoch, value: Value) -> Result<(), ()> {
		if current_epoch >= epoch {
			self.current = value;
			self.next = None;
			return Ok(());
		}
		if let Some((next_epoch, next_value)) = self.next.as_mut() {
			match epoch.cmp(&next_epoch) {
				Ordering::Less => {
					// new value is in nearer future, so update next
					*next_epoch = epoch;
					*next_value = value;
				},
				Ordering::Equal => {
					// replace
					*next_epoch = epoch;
					*next_value = value;
				},
				Ordering::Greater => {
					if current_epoch < *next_epoch {
						// illegal since we would loose current value while still needed
						return Err(());
					}
					// move next into current, insert as next value
					self.current = *next_value;
					*next_epoch = epoch;
					*next_value = value;
				},
			}
		} else {
			self.next = Some((epoch, value));
		}

		Ok(())
	}

	/// Returns the current value or the provisional value if present and epoch is >= the provisional value application.
	pub fn get(&self, epoch: Epoch) -> Value {
		if let Some((e, v)) = &self.next {
			if e <= &epoch {
				return *v;
			}
		}

		self.current
	}
}

#[cfg(test)]
mod tests {
	use frame_support::{assert_err, assert_ok};

	use super::*;

	#[test]
	fn test_set_next() {
		let mut b: ProvisionalBuffer<i32, i32> = ProvisionalBuffer::new(-1);
		assert_eq!(b.get(0), -1);
		assert_eq!(b.get(5), -1);

		assert_ok!(b.set(5, 9, -9));
		assert_eq!(b.get(5), -1);
		assert_eq!(b.get(8), -1);
		assert_eq!(b.get(9), -9);
		assert_eq!(b.get(10), -9);
	}

	#[test]
	fn test_set_before_next() {
		let mut b: ProvisionalBuffer<i32, i32> = ProvisionalBuffer::new(-1);
		assert_eq!(b.get(0), -1);
		assert_eq!(b.get(5), -1);

		assert_ok!(b.set(5, 10, -10));
		assert_ok!(b.set(5, 9, -9));
		assert_eq!(b.get(8), -1);
		assert_eq!(b.get(9), -9);
		assert_eq!(b.get(10), -9);
	}

	#[test]
	fn test_set_at_next() {
		let mut b: ProvisionalBuffer<i32, i32> = ProvisionalBuffer::new(-1);
		assert_eq!(b.get(0), -1);
		assert_eq!(b.get(5), -1);

		assert_ok!(b.set(5, 10, -10));
		assert_ok!(b.set(5, 10, -10));
		assert_eq!(b.get(9), -1);
		assert_eq!(b.get(10), -10);
		assert_eq!(b.get(11), -10);
	}

	#[test]
	fn test_set_after_next() {
		let mut b: ProvisionalBuffer<i32, i32> = ProvisionalBuffer::new(-1);
		assert_eq!(b.get(0), -1);
		assert_eq!(b.get(5), -1);

		assert_ok!(b.set(5, 10, -10));
		assert_err!(b.set(5, 11, -11), ());

		assert_ok!(b.set(11, 11, -11));
		assert_eq!(b.get(10), -11);
		assert_eq!(b.get(11), -11);
		assert_eq!(b.get(12), -11);
	}
}
