use crate::{Config, Error};

impl<T> From<pallet_acurast::Error<T>> for Error<T> {
	fn from(e: pallet_acurast::Error<T>) -> Self {
		Error::<T>::PalletAcurast(e)
	}
}

impl<T: Config> From<Error<T>> for pallet_acurast::Error<T> {
	fn from(_: Error<T>) -> Self {
		Self::JobHookFailed
	}
}
