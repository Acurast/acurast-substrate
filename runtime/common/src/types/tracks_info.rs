use alloc::borrow::Cow;
use core::marker::PhantomData;

use frame_support::traits::{CallerTrait, OriginTrait};
use sp_core::Get;

use pallet_referenda::Track;

use crate::types::{Balance, BlockNumber};

pub struct TracksInfo<T, TR>(PhantomData<(T, TR)>);
impl<T, TR> pallet_referenda::TracksInfo<Balance, BlockNumber> for TracksInfo<T, TR>
where
	T: frame_system::Config,
	TR: Get<[Track<u16, Balance, BlockNumber>; 1]>,
{
	type Id = u16;
	type RuntimeOrigin = <T::RuntimeOrigin as OriginTrait>::PalletsOrigin;

	fn tracks() -> impl Iterator<Item = Cow<'static, Track<Self::Id, Balance, BlockNumber>>> {
		TR::get().into_iter().map(Cow::Owned)
	}

	fn track_for(origin: &Self::RuntimeOrigin) -> Result<Self::Id, ()> {
		let Some(origin) = origin.as_system_ref() else {
			return Err(());
		};
		match origin {
			frame_system::RawOrigin::Root => Ok(0),
			_ => Err(()),
		}
	}
}
