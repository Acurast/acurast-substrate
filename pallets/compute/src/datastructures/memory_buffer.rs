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
pub struct MemoryBuffer<Timestamp: Ord, Value> {
	/// The past value and the timestamp range when it was valid as `(start_time, value)`.
	/// `end_time` is the same as `current.0` (when current value became valid).
	past: Option<(Timestamp, Value)>,
	/// The current value and the timestamp when it became valid.
	current: (Timestamp, Value),
}

impl<Timestamp: Ord + Copy + Debug, Value: Copy + Default + Debug> MemoryBuffer<Timestamp, Value> {
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
	pub fn mutate<F>(&mut self, t: Timestamp, f: F, retain: bool) -> Result<(), ()>
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
				if !retain {
					self.current.1 = Default::default();
				}
				self.current.0 = t;
				f(&mut self.current.1);
				Ok(())
			},
		}
	}

	/// Returns the value valid only if set exactly at the specified timestamp.
	///
	/// Returns default if the timestamp is neither at the set time of past nor current.
	pub fn get(&self, t: Timestamp) -> Value {
		if t == self.current.0 {
			// Timestamp is the current value's set time
			return self.current.1;
		}

		if let Some((past_start, past_value)) = &self.past {
			if t == *past_start {
				// Timestamp is the current value's set time
				return *past_value;
			}
		}

		// Timestamp is outside the remembered range
		Default::default()
	}

	/// Returns the value valid at the specified timestamp, but maybe set before that.
	///
	/// Returns default if the timestamp is before the past value's set time
	/// or before the current value's set time when there's no past value.
	pub fn get_latest(&self, t: Timestamp) -> Value {
		if t >= self.current.0 {
			// Timestamp is in current value's validity range
			return self.current.1;
		}

		if let Some((past_start, past_value)) = &self.past {
			if t >= *past_start && t < self.current.0 {
				// Timestamp is in past value's validity range
				return *past_value;
			}
		}

		// Timestamp is outside the remembered range
		Default::default()
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
		assert_eq!(b.get_latest(0), 100);
		assert_eq!(b.get_latest(5), 100);
		assert_eq!(b.get_latest(-1), 0); // Before current start - returns default
	}

	#[test]
	fn test_set_at_current_timestamp() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_ok!(b.set(0, 200));
		assert_eq!(b.get_latest(0), 200);
		assert_eq!(b.get_latest(5), 200);
	}

	#[test]
	fn test_set_new_value_creates_past() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_ok!(b.set(10, 200));

		// Past value should be accessible in [0, 10)
		assert_eq!(b.get_latest(0), 100);
		assert_eq!(b.get_latest(5), 100);
		assert_eq!(b.get_latest(9), 100);

		// Current value should be accessible from [10, ∞)
		assert_eq!(b.get_latest(10), 200);
		assert_eq!(b.get_latest(15), 200);
		assert_eq!(b.get_latest(100), 200);
	}

	#[test]
	fn test_set_overwrites_past() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_ok!(b.set(10, 200));
		assert_ok!(b.set(20, 300));

		// Old past (0-10) is no longer accessible - returns default
		assert_eq!(b.get_latest(0), 0);
		assert_eq!(b.get_latest(5), 0);

		// New past (10-20) is accessible
		assert_eq!(b.get_latest(10), 200);
		assert_eq!(b.get_latest(15), 200);
		assert_eq!(b.get_latest(19), 200);

		// Current (20+) is accessible
		assert_eq!(b.get_latest(20), 300);
		assert_eq!(b.get_latest(25), 300);
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
	fn test_outside_range_returns_default() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(10, 100);

		// Before current start - returns default
		assert_eq!(b.get_latest(0), 0);
		assert_eq!(b.get_latest(5), 0);
		assert_eq!(b.get_latest(9), 0);

		assert_ok!(b.set(20, 200));

		// Before past start - returns default
		assert_eq!(b.get_latest(0), 0);
		assert_eq!(b.get_latest(5), 0);
		assert_eq!(b.get_latest(9), 0);

		// In past range
		assert_eq!(b.get_latest(10), 100);
		assert_eq!(b.get_latest(15), 100);

		// In current range
		assert_eq!(b.get_latest(20), 200);
	}

	#[test]
	fn test_mutate_at_current_timestamp() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_ok!(b.mutate(0, |v| *v += 50, false));
		assert_eq!(b.get_latest(0), 150);
		assert_eq!(b.get_latest(5), 150);
	}

	#[test]
	fn test_mutate_new_timestamp_creates_past() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);
		assert_ok!(b.mutate(10, |v| *v += 50, false));

		// Past value should be accessible in [0, 10)
		assert_eq!(b.get_latest(0), 100);
		assert_eq!(b.get_latest(5), 100);
		assert_eq!(b.get_latest(9), 100);

		// Current value should be accessible from [10, ∞) - starts from default (0) + 50
		assert_eq!(b.get_latest(10), 50);
		assert_eq!(b.get_latest(15), 50);
	}

	#[test]
	fn test_mutate_in_past_fails() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(10, 100);
		assert_err!(b.mutate(5, |v| *v = 200, false), ());
	}

	#[test]
	fn test_mutate_multiple_times() {
		let mut b: MemoryBuffer<i32, i32> = MemoryBuffer::new_with(0, 100);

		// Mutate at same timestamp multiple times
		assert_ok!(b.mutate(0, |v| *v += 10, false));
		assert_eq!(b.get_latest(0), 110);

		assert_ok!(b.mutate(0, |v| *v += 20, false));
		assert_eq!(b.get_latest(0), 130);

		// Move to new timestamp and mutate with retain=true
		assert_ok!(b.mutate(10, |v| *v += 50, true));
		assert_eq!(b.get_latest(0), 130);
		assert_eq!(b.get_latest(10), 180);

		// Mutate at new timestamp
		assert_ok!(b.mutate(10, |v| *v += 25, false));
		assert_eq!(b.get_latest(10), 205);
	}
}
