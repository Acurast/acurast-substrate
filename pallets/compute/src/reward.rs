use core::ops::Add;
use frame_support::traits::tokens::Balance;
use sp_runtime::traits::Zero;

/// A reward that must be handled explicitly to prevent accidental loss.
///
/// Similar to the [`frame_support::traits::Currency::NegativeImbalance`] pattern used in [`frame_support::traits::Currency`] traits.
#[must_use]
pub struct PendingReward<T: Balance> {
	amount: T,
}

impl<T: Balance> PendingReward<T> {
	/// Create a new pending reward
	pub(crate) fn new(amount: T) -> Self {
		Self { amount }
	}

	/// Get the reward amount without consuming the wrapper
	pub fn peek(&self) -> T {
		self.amount
	}

	/// Consume the reward, returning the amount.
	/// This is the primary way to "use" the reward.
	pub fn consume(self) -> T {
		self.amount
	}

	/// Split the reward into two parts
	pub fn split(self, amount: T) -> Result<(Self, Self), Self> {
		if amount > self.amount {
			Err(self)
		} else {
			Ok((Self::new(amount), Self::new(self.amount - amount)))
		}
	}

	/// Add another pending reward to this one
	pub fn merge(self, other: Self) -> Self {
		Self::new(self.amount + other.amount)
	}

	/// Convert to a different balance type (if needed)
	pub fn convert<U: Balance + From<T>>(self) -> PendingReward<U> {
		PendingReward::new(self.amount.into())
	}

	/// Explicitly drop the reward (useful for testing or error cases)
	/// Forces the caller to acknowledge they're discarding the reward
	pub fn drop_reward(self) {
		// Reward is dropped here intentionally
	}
}

impl<T: Balance> Add for PendingReward<T> {
	type Output = Self;

	fn add(self, other: Self) -> Self {
		Self::new(self.amount + other.amount)
	}
}

impl<T: Balance> Zero for PendingReward<T> {
	fn zero() -> Self {
		Self::new(T::zero())
	}

	fn is_zero(&self) -> bool {
		self.amount.is_zero()
	}
}

// Optional: Implement Drop with a panic for debugging (remove in production)
#[cfg(debug_assertions)]
impl<T: Balance> Drop for PendingReward<T> {
	fn drop(&mut self) {
		if !self.amount.is_zero() {
			// This will only panic in debug builds if reward is dropped without being used
			panic!("PendingReward dropped without being consumed! Amount: {:?}", self.amount);
		}
	}
}
