// Copyright 2019-2022 PureStake Inc.
// Copyright 2023 Papers AG

/* TODO: use orml_utilities::OrderedSet without leaking substrate v2.0 dependencies*/
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

/// An ordered set backed by `Vec`
#[derive(
    RuntimeDebug, PartialEq, Eq, Encode, Decode, Default, Clone, TypeInfo, Serialize, Deserialize,
)]
pub struct OrderedSet<T>(pub Vec<T>);

impl<T: Ord> OrderedSet<T> {
    /// Create a new empty set
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create a set from a `Vec`.
    /// `v` will be sorted and dedup first.
    pub fn from(mut v: Vec<T>) -> Self {
        v.sort();
        v.dedup();
        Self::from_sorted_set(v)
    }

    /// Create a set from a `Vec`.
    /// Assume `v` is sorted and contain unique elements.
    pub fn from_sorted_set(v: Vec<T>) -> Self {
        Self(v)
    }

    /// Insert an element.
    /// Return true if insertion happened.
    pub fn insert(&mut self, value: T) -> bool {
        match self.0.binary_search(&value) {
            Ok(_) => false,
            Err(loc) => {
                self.0.insert(loc, value);
                true
            }
        }
    }

    /// Remove an element.
    /// Return true if removal happened.
    pub fn remove(&mut self, value: &T) -> bool {
        match self.0.binary_search(value) {
            Ok(loc) => {
                self.0.remove(loc);
                true
            }
            Err(_) => false,
        }
    }

    /// Return if the set contains `value`
    pub fn contains(&self, value: &T) -> bool {
        self.0.binary_search(value).is_ok()
    }

    /// Clear the set
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl<T: Ord> From<Vec<T>> for OrderedSet<T> {
    fn from(v: Vec<T>) -> Self {
        Self::from(v)
    }
}
