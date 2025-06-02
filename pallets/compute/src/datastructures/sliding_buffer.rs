use core::ops::Add;

use frame_support::pallet_prelude::*;
use sp_runtime::traits::{Debug, One};

/// A length-2 sliding buffer storing two values `(previous, current)` that keeps (modifiable) values for two **subsequent** epochs.
///
/// This is useful to memorize the metric totals when some processors' metrics already add towards subsequent epoch's total.
/// Instead of storing an array of values by epoch, it achieves this by only two values stored.
///
/// Two stored, non-default values `(previous, current)` are always for adjacent epochs `(epoch - 1, epoch)`.
///
/// * If you write to `epoch + 1`, slot for `epoch - 1` gets lost.
/// * If you write to `epoch + x` for `x > 1`, both slots get lost.
///
/// Whenever a slot is reset, the default of `Value` is used. You can provide `Option<V>` as `Value` if you like to distinguish between `0` and no value.
/// ```
#[derive(
	RuntimeDebugNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	PartialEq,
	Eq,
)]
pub struct SlidingBuffer<
	Epoch: Copy + Ord + One + Add<Output = Epoch> + Debug,
	Value: Copy + Default + Debug,
> {
	epoch: Epoch,
	prev: Value,
	cur: Value,
}

impl<Epoch: Copy + Ord + One + Add<Output = Epoch> + Debug, Value: Copy + Default + Debug>
	SlidingBuffer<Epoch, Value>
{
	pub fn new(epoch: Epoch) -> Self {
		Self { epoch, prev: Default::default(), cur: Default::default() }
	}

	#[cfg(test)]
	pub fn from_inner(epoch: Epoch, prev: Value, cur: Value) -> Self {
		Self { epoch, prev, cur }
	}

	/// Sets the value for a specific epoch.
	///
	/// It either updates one of the two buffered values if `epoch` denotes one of them or
	/// it "rotates" the buffer if `epoch` is subsequent to [`Self::epoch`]. In all other cases it clears the two buffered values and stores the new `value`.
	pub fn mutate<F>(&mut self, epoch: Epoch, f: F)
	where
		F: FnOnce(&mut Value),
	{
		if epoch + One::one() == self.epoch {
			f(&mut self.prev);
		} else if epoch == self.epoch {
			f(&mut self.cur);
		} else if self.epoch + One::one() == epoch {
			// shift since we are updating the subsequent value
			self.prev = self.cur;
			self.cur = Default::default();
			self.epoch = epoch;
			f(&mut self.cur);
		} else {
			self.prev = Default::default();
			self.cur = Default::default();
			self.epoch = epoch;
			f(&mut self.cur);
		}
	}

	/// Returns some value if it has been memorized, otherwise the default value.
	pub fn get(&self, epoch: Epoch) -> Value {
		if epoch + One::one() == self.epoch {
			self.prev
		} else if epoch == self.epoch {
			self.cur
		} else {
			Default::default()
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_subsequent() {
		// To distinguish 0 from None we use an optional `Value` type.
		let mut b: SlidingBuffer<i32, Option<i32>> = SlidingBuffer::new(0);
		assert_eq!(b.get(0), None);

		b.mutate(1, |v| {
			*v = Some(1);
		});
		assert_eq!(b.get(1), Some(1));
		assert_eq!(b.get(2), None);

		b.mutate(2, |v| {
			*v = Some(2);
		});
		assert_eq!(b.get(1), Some(1));
		assert_eq!(b.get(2), Some(2));
		assert_eq!(b.get(3), None);

		b.mutate(3, |v| {
			*v = Some(3);
		});
		assert_eq!(b.get(1), None);
		assert_eq!(b.get(2), Some(2));
		assert_eq!(b.get(3), Some(3));
		assert_eq!(b.get(4), None);
	}

	#[test]
	fn test_gap() {
		// To distinguish 0 from None we use an optional `Value` type.
		let mut b: SlidingBuffer<i32, Option<i32>> = SlidingBuffer::new(0);
		assert_eq!(b.get(0), None);

		b.mutate(1, |v| {
			*v = Some(1);
		});
		assert_eq!(b.get(1), Some(1));
		assert_eq!(b.get(2), None);

		b.mutate(4, |v| {
			*v = Some(4);
		});
		assert_eq!(b.get(1), None);
		assert_eq!(b.get(2), None);
		assert_eq!(b.get(4), Some(4));
		assert_eq!(b.get(5), None);
	}

	#[test]
	fn test_update_previsous_adjacent() {
		// To distinguish 0 from None we use an optional `Value` type.
		let mut b: SlidingBuffer<i32, Option<i32>> = SlidingBuffer::new(0);
		assert_eq!(b.get(0), None);

		b.mutate(1, |v| {
			*v = Some(1);
		});
		assert_eq!(b.get(1), Some(1));
		assert_eq!(b.get(2), None);

		b.mutate(2, |v| {
			*v = Some(2);
		});
		assert_eq!(b.get(1), Some(1));
		assert_eq!(b.get(2), Some(2));
		assert_eq!(b.get(3), None);

		// here we update previous value, adjacent to current
		b.mutate(1, |v| {
			*v = Some(11);
		});
		// ...and we expect nothing lost but previous updated
		assert_eq!(b.get(1), Some(11));
		assert_eq!(b.get(2), Some(2));
		assert_eq!(b.get(3), None);
	}

	#[test]
	fn test_update_previsous_gap() {
		// To distinguish 0 from None we use an optional `Value` type.
		let mut b: SlidingBuffer<i32, Option<i32>> = SlidingBuffer::new(0);
		assert_eq!(b.get(0), None);

		b.mutate(1, |v| {
			*v = Some(1);
		});
		assert_eq!(b.get(1), Some(1));
		assert_eq!(b.get(2), None);

		b.mutate(2, |v| {
			*v = Some(2);
		});
		assert_eq!(b.get(1), Some(1));
		assert_eq!(b.get(2), Some(2));
		assert_eq!(b.get(3), None);

		// here we update previous value, but more than one in the past
		b.mutate(0, |v| {
			*v = Some(10);
		});
		// ...and we expect nothing lost but previous updated
		assert_eq!(b.get(0), Some(10));
		assert_eq!(b.get(1), None);
		assert_eq!(b.get(2), None);
		assert_eq!(b.get(3), None);
	}
}
