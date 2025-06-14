// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::helper;
use quote::ToTokens;
use syn::spanned::Spanned;

/// List of additional token to be used for parsing.
mod keyword {
	syn::custom_keyword!(pallet);
	syn::custom_keyword!(Pallet);
	syn::custom_keyword!(without_storage_info);
	syn::custom_keyword!(storage_version);
}

/// Definition of the pallet pallet.
pub struct PalletStructDef {
	/// The index of item in pallet pallet.
	pub index: usize,
	/// A set of usage of instance, must be check for consistency with config trait.
	pub instances: Vec<helper::InstanceUsage>,
	/// The keyword Pallet used (contains span).
	pub pallet: keyword::Pallet,
	/// The span of the pallet::pallet attribute.
	pub attr_span: proc_macro2::Span,
	/// Whether to specify the storages max encoded len when implementing `StorageInfoTrait`.
	/// Contains the span of the attribute.
	pub without_storage_info: Option<proc_macro2::Span>,
	/// The in-code storage version of the pallet.
	pub storage_version: Option<syn::Path>,
}

/// Parse for one variant of:
/// * `#[pallet::without_storage_info]`
/// * `#[pallet::storage_version(STORAGE_VERSION)]`
pub enum PalletStructAttr {
	WithoutStorageInfoTrait(proc_macro2::Span),
	StorageVersion { storage_version: syn::Path, span: proc_macro2::Span },
}

impl PalletStructAttr {
	fn span(&self) -> proc_macro2::Span {
		match self {
			Self::WithoutStorageInfoTrait(span) | Self::StorageVersion { span, .. } => *span,
		}
	}
}

impl syn::parse::Parse for PalletStructAttr {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		input.parse::<syn::Token![#]>()?;
		let content;
		syn::bracketed!(content in input);
		content.parse::<keyword::pallet>()?;
		content.parse::<syn::Token![::]>()?;

		let lookahead = content.lookahead1();
		if lookahead.peek(keyword::without_storage_info) {
			let span = content.parse::<keyword::without_storage_info>()?.span();
			Ok(Self::WithoutStorageInfoTrait(span))
		} else if lookahead.peek(keyword::storage_version) {
			let span = content.parse::<keyword::storage_version>()?.span();

			let version_content;
			syn::parenthesized!(version_content in content);
			let storage_version = version_content.parse::<syn::Path>()?;

			Ok(Self::StorageVersion { storage_version, span })
		} else {
			Err(lookahead.error())
		}
	}
}

impl PalletStructDef {
	pub fn try_from(
		attr_span: proc_macro2::Span,
		index: usize,
		item: &mut syn::Item,
	) -> syn::Result<Self> {
		let item = if let syn::Item::Struct(item) = item {
			item
		} else {
			let msg = "Invalid pallet::pallet, expected struct definition";
			return Err(syn::Error::new(item.span(), msg));
		};

		let mut without_storage_info = None;
		let mut storage_version_found = None;

		let struct_attrs: Vec<PalletStructAttr> = helper::take_item_pallet_attrs(&mut item.attrs)?;
		for attr in struct_attrs {
			match attr {
				PalletStructAttr::WithoutStorageInfoTrait(span)
					if without_storage_info.is_none() =>
				{
					without_storage_info = Some(span);
				},
				PalletStructAttr::StorageVersion { storage_version, .. }
					if storage_version_found.is_none() =>
				{
					storage_version_found = Some(storage_version);
				},
				attr => {
					let msg = "Unexpected duplicated attribute";
					return Err(syn::Error::new(attr.span(), msg));
				},
			}
		}

		let pallet = syn::parse2::<keyword::Pallet>(item.ident.to_token_stream())?;

		if !matches!(item.vis, syn::Visibility::Public(_)) {
			let msg = "Invalid pallet::pallet, Pallet must be public";
			return Err(syn::Error::new(item.span(), msg));
		}

		if item.generics.where_clause.is_some() {
			let msg = "Invalid pallet::pallet, where clause not supported on Pallet declaration";
			return Err(syn::Error::new(item.generics.where_clause.span(), msg));
		}

		let instances =
			vec![helper::check_type_def_gen_no_bounds(&item.generics, item.ident.span())?];

		Ok(Self {
			index,
			instances,
			pallet,
			attr_span,
			without_storage_info,
			storage_version: storage_version_found,
		})
	}
}
