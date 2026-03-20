use core::cmp::Ordering;

use frame_support::pallet_prelude::*;
use sp_runtime::traits::Debug;

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	PartialEq,
	Eq,
	Default,
)]
pub struct MemoryBuffer<Timestamp, Value> {
	/// The past value and the timestamp range when it was valid as `(start_time, value)`.
	/// `end_time` is the same as `current.0` (when current value became valid).
	past: Option<(Timestamp, Value)>,
	/// The current value and the timestamp when it became valid.
	current: (Timestamp, Value),
}

impl<Timestamp: Ord + Copy + Debug, Value: Copy + Debug> MemoryBuffer<Timestamp, Value> {
	pub fn new_with(timestamp: Timestamp, value: Value) -> Self {
		Self { current: (timestamp, value), past: None }
	}

	#[cfg(test)]
	pub fn from_inner(current: (Timestamp, Value), past: Option<(Timestamp, Value)>) -> Self {
		Self { current, past }
	}

	/// Sets a new value at the specified current_timestamp.
	///
	/// - If current_timestamp < current start time: returns error
	/// - If current_timestamp == current start time: updates current value
	/// - If current_timestamp > current start time: moves current to past, sets new current
	#[allow(clippy::result_unit_err)]
	pub fn set(&mut self, current_timestamp: Timestamp, value: Value) -> Result<(), ()> {
		match current_timestamp.cmp(&self.current.0) {
			Ordering::Less => {
				// Setting in the past is not allowed
				Err(())
			},
			Ordering::Equal => {
				// Replace current value at the same timestamp
				self.current.1 = value;
				Ok(())
			},
			Ordering::Greater => {
				// Move current to past, set new current
				self.past = Some((self.current.0, self.current.1));
				self.current = (current_timestamp, value);
				Ok(())
			},
		}
	}

	/// Mutates the value at the specified current_timestamp in place.
	///
	/// - If current_timestamp < current start time: returns error
	/// - If current_timestamp == current start time: mutates current value in place
	/// - If current_timestamp > current start time: copies current to past, mutates same current value in place
	#[allow(clippy::result_unit_err)]
	pub fn mutate<F>(&mut self, t: Timestamp, f: F) -> Result<(), ()>
	where
		F: FnOnce(&mut Value),
	{
		match t.cmp(&self.current.0) {
			Ordering::Less => {
				// Mutating in the past is not allowed
				Err(())
			},
			Ordering::Equal => {
				// Mutate current value at the same timestamp
				f(&mut self.current.1);
				Ok(())
			},
			Ordering::Greater => {
				self.past = Some((self.current.0, self.current.1));
				self.current.0 = t;
				f(&mut self.current.1);
				Ok(())
			},
		}
	}

	/// Returns the value valid only if set exactly at the specified timestamp.
	///
	/// Returns `None` if the timestamp is neither at the set time of past nor current.
	pub fn get(&self, t: Timestamp) -> Option<Value> {
		if t == self.current.0 {
			// Timestamp is the current value's set time
			return Some(self.current.1);
		}

		if let Some((past_start, past_value)) = &self.past {
			if t == *past_start {
				// Timestamp is the current value's set time
				return Some(*past_value);
			}
		}

		// Timestamp is outside the remembered range
		None
	}

	//// Returns the value valid at the specified timestamp if within the remembered range.
	///
	/// Returns `None` if the timestamp is before the past value's set time
	/// or before the current value's set time when there's no past value.
	/// This allows callers to distinguish between "no data" and "data is default".
	pub fn get_latest(&self, t: Timestamp) -> Option<Value> {
		if t >= self.current.0 {
			// Timestamp is in current value's validity range
			return Some(self.current.1);
		}

		if let Some((past_start, past_value)) = &self.past {
			if t >= *past_start && t < self.current.0 {
				// Timestamp is in past value's validity range
				return Some(*past_value);
			}
		}

		// Timestamp is outside the remembered range
		None
	}

	/// Returns the current value and its start timestamp.
	pub fn get_current(&self) -> (Timestamp, Value) {
		self.current
	}

	/// Returns the past value and its validity period `(start_time, end_time)` if present.
	/// `end_time` is when the current value became valid.
	pub fn get_past(&self) -> Option<(Timestamp, Timestamp, Value)> {
		self.past.map(|(start, value)| (start, self.current.0, value))
	}
}

#[cfg(test)]
mod tests {
	use frame_support::{assert_err, assert_ok};

	use super::*;

	#[test]
	fn test_new_buffer() {
		let b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_eq!(b.get_latest(0), Some(100));
		assert_eq!(b.get_latest(5), Some(100));
		assert_eq!(b.get_latest(-1), None); // Before current start - returns None
	}

	#[test]
	fn test_set_at_current_timestamp() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_ok!(b.set(0, 200));
		assert_eq!(b.get_latest(0), Some(200));
		assert_eq!(b.get_latest(5), Some(200));
	}

	#[test]
	fn test_set_new_value_creates_past() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_ok!(b.set(10, 200));

		// Past value should be accessible in [0, 10)
		assert_eq!(b.get_latest(0), Some(100));
		assert_eq!(b.get_latest(5), Some(100));
		assert_eq!(b.get_latest(9), Some(100));

		// Current value should be accessible from [10, ∞)
		assert_eq!(b.get_latest(10), Some(200));
		assert_eq!(b.get_latest(15), Some(200));
		assert_eq!(b.get_latest(100), Some(200));
	}

	#[test]
	fn test_set_overwrites_past() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_ok!(b.set(10, 200));
		assert_ok!(b.set(20, 300));

		// Old past (0-10) is no longer accessible - returns None
		assert_eq!(b.get_latest(0), None);
		assert_eq!(b.get_latest(5), None);

		// New past (10-20) is accessible
		assert_eq!(b.get_latest(10), Some(200));
		assert_eq!(b.get_latest(15), Some(200));
		assert_eq!(b.get_latest(19), Some(200));

		// Current (20+) is accessible
		assert_eq!(b.get_latest(20), Some(300));
		assert_eq!(b.get_latest(25), Some(300));
	}

	#[test]
	fn test_set_in_past_fails() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(10, 100);
		assert_err!(b.set(5, 200), ());
	}

	#[test]
	fn test_get_current() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_eq!(b.get_current(), (0, 100));

		assert_ok!(b.set(10, 200));
		assert_eq!(b.get_current(), (10, 200));
	}

	#[test]
	fn test_get_past() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_eq!(b.get_past(), None);

		assert_ok!(b.set(10, 200));
		assert_eq!(b.get_past(), Some((0, 10, 100)));

		assert_ok!(b.set(20, 300));
		assert_eq!(b.get_past(), Some((10, 20, 200)));
	}

	#[test]
	fn test_outside_range_returns_none() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(10, 100);

		// Before current start - returns None
		assert_eq!(b.get_latest(0), None);
		assert_eq!(b.get_latest(5), None);
		assert_eq!(b.get_latest(9), None);

		assert_ok!(b.set(20, 200));

		// Before past start - returns None
		assert_eq!(b.get_latest(0), None);
		assert_eq!(b.get_latest(5), None);
		assert_eq!(b.get_latest(9), None);

		// In past range
		assert_eq!(b.get_latest(10), Some(100));
		assert_eq!(b.get_latest(15), Some(100));

		// In current range
		assert_eq!(b.get_latest(20), Some(200));
	}

	#[test]
	fn test_mutate_at_current_timestamp() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_ok!(b.mutate(0, |v| *v += 50));
		assert_eq!(b.get_latest(0), Some(150));
		assert_eq!(b.get_latest(5), Some(150));
	}

	#[test]
	fn test_mutate_new_timestamp_creates_past() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_ok!(b.mutate(10, |v| *v += 50));

		// Past value should be accessible in [0, 10)
		assert_eq!(b.get_latest(0), Some(100));
		assert_eq!(b.get_latest(5), Some(100));
		assert_eq!(b.get_latest(9), Some(100));

		// Current value should be accessible from [10, ∞) - starts from default (0) + 50
		assert_eq!(b.get_latest(10), Some(150));
		assert_eq!(b.get_latest(15), Some(150));
	}

	#[test]
	fn test_mutate_in_past_fails() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(10, 100);
		assert_err!(b.mutate(5, |v| *v = 200), ());
	}

	#[test]
	fn test_get_latest() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(10, 100);

		// Within current range - returns Some
		assert_eq!(b.get_latest(10), Some(100));
		assert_eq!(b.get_latest(15), Some(100));
		assert_eq!(b.get_latest(100), Some(100));

		// Before current start, no past - returns None
		assert_eq!(b.get_latest(0), None);
		assert_eq!(b.get_latest(5), None);
		assert_eq!(b.get_latest(9), None);

		// Add a past value by setting a new current
		assert_ok!(b.set(20, 200));

		// Within current range
		assert_eq!(b.get_latest(20), Some(200));
		assert_eq!(b.get_latest(25), Some(200));

		// Within past range
		assert_eq!(b.get_latest(10), Some(100));
		assert_eq!(b.get_latest(15), Some(100));
		assert_eq!(b.get_latest(19), Some(100));

		// Before past start - returns None
		assert_eq!(b.get_latest(0), None);
		assert_eq!(b.get_latest(5), None);
		assert_eq!(b.get_latest(9), None);

		// After another set, the old past is lost
		assert_ok!(b.set(30, 300));

		// Within current range
		assert_eq!(b.get_latest(30), Some(300));

		// Within new past range (was current before)
		assert_eq!(b.get_latest(20), Some(200));
		assert_eq!(b.get_latest(25), Some(200));

		// Old past (10-19) is now gone - returns None
		assert_eq!(b.get_latest(10), None);
		assert_eq!(b.get_latest(15), None);
		assert_eq!(b.get_latest(19), None);
	}
}
