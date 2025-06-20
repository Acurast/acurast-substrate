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

//! Implementation of the `derive_impl` attribute macro.

use macro_magic::mm_core::ForeignPath;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use std::collections::HashSet;
use syn::{
	parse2, parse_quote, spanned::Spanned, token, AngleBracketedGenericArguments, Ident, ImplItem,
	ItemImpl, Path, PathArguments, PathSegment, Result, Token,
};

mod keyword {
	syn::custom_keyword!(inject_runtime_type);
	syn::custom_keyword!(no_aggregated_types);
}

#[derive(derive_syn_parse::Parse, PartialEq, Eq)]
pub enum PalletAttrType {
	#[peek(keyword::inject_runtime_type, name = "inject_runtime_type")]
	RuntimeType(keyword::inject_runtime_type),
}

#[derive(derive_syn_parse::Parse)]
pub struct PalletAttr {
	_pound: Token![#],
	#[bracket]
	_bracket: token::Bracket,
	#[inside(_bracket)]
	typ: PalletAttrType,
}

fn is_runtime_type(item: &syn::ImplItemType) -> bool {
	item.attrs.iter().any(|attr| {
		if let Ok(PalletAttr { typ: PalletAttrType::RuntimeType(_), .. }) =
			parse2::<PalletAttr>(attr.into_token_stream())
		{
			return true;
		}
		false
	})
}
pub struct DeriveImplAttrArgs {
	pub default_impl_path: Path,
	pub generics: Option<AngleBracketedGenericArguments>,
	_as: Option<Token![as]>,
	pub disambiguation_path: Option<Path>,
	_comma: Option<Token![,]>,
	pub no_aggregated_types: Option<keyword::no_aggregated_types>,
}

impl syn::parse::Parse for DeriveImplAttrArgs {
	fn parse(input: syn::parse::ParseStream) -> Result<Self> {
		let mut default_impl_path: Path = input.parse()?;
		// Extract the generics if any
		let (default_impl_path, generics) = match default_impl_path.clone().segments.last() {
			Some(PathSegment { ident, arguments: PathArguments::AngleBracketed(args) }) => {
				default_impl_path.segments.pop();
				default_impl_path
					.segments
					.push(PathSegment { ident: ident.clone(), arguments: PathArguments::None });
				(default_impl_path, Some(args.clone()))
			},
			Some(PathSegment { arguments: PathArguments::None, .. }) => (default_impl_path, None),
			_ => {
				return Err(syn::Error::new(default_impl_path.span(), "Invalid default impl path"))
			},
		};

		let lookahead = input.lookahead1();
		let (_as, disambiguation_path) = if lookahead.peek(Token![as]) {
			let _as: Token![as] = input.parse()?;
			let disambiguation_path: Path = input.parse()?;
			(Some(_as), Some(disambiguation_path))
		} else {
			(None, None)
		};

		let lookahead = input.lookahead1();
		let (_comma, no_aggregated_types) = if lookahead.peek(Token![,]) {
			let _comma: Token![,] = input.parse()?;
			let no_aggregated_types: keyword::no_aggregated_types = input.parse()?;
			(Some(_comma), Some(no_aggregated_types))
		} else {
			(None, None)
		};

		Ok(DeriveImplAttrArgs {
			default_impl_path,
			generics,
			_as,
			disambiguation_path,
			_comma,
			no_aggregated_types,
		})
	}
}

impl ForeignPath for DeriveImplAttrArgs {
	fn foreign_path(&self) -> &Path {
		&self.default_impl_path
	}
}

impl ToTokens for DeriveImplAttrArgs {
	fn to_tokens(&self, tokens: &mut TokenStream2) {
		tokens.extend(self.default_impl_path.to_token_stream());
		tokens.extend(self.generics.to_token_stream());
		tokens.extend(self._as.to_token_stream());
		tokens.extend(self.disambiguation_path.to_token_stream());
		tokens.extend(self._comma.to_token_stream());
		tokens.extend(self.no_aggregated_types.to_token_stream());
	}
}

/// Gets the [`Ident`] representation of the given [`ImplItem`], if one exists. Otherwise
/// returns [`None`].
///
/// Used by [`combine_impls`] to determine whether we can compare [`ImplItem`]s by [`Ident`]
/// or not.
fn impl_item_ident(impl_item: &ImplItem) -> Option<&Ident> {
	match impl_item {
		ImplItem::Const(item) => Some(&item.ident),
		ImplItem::Fn(item) => Some(&item.sig.ident),
		ImplItem::Type(item) => Some(&item.ident),
		ImplItem::Macro(item) => item.mac.path.get_ident(),
		_ => None,
	}
}

/// The real meat behind `derive_impl`. Takes in a `local_impl`, which is the impl for which we
/// want to implement defaults (i.e. the one the attribute macro is attached to), and a
/// `foreign_impl`, which is the impl containing the defaults we want to use, and returns an
/// [`ItemImpl`] containing the final generated impl.
///
/// This process has the following caveats:
/// * Colliding items that have an ident are not copied into `local_impl`
/// * Uncolliding items that have an ident are copied into `local_impl` but are qualified as `type
///   #ident = <#default_impl_path as #disambiguation_path>::#ident;`
/// * Items that lack an ident are de-duplicated so only unique items that lack an ident are copied
///   into `local_impl`. Items that lack an ident and also exist verbatim in `local_impl` are not
///   copied over.
fn combine_impls(
	local_impl: ItemImpl,
	foreign_impl: ItemImpl,
	default_impl_path: Path,
	disambiguation_path: Path,
	inject_runtime_types: bool,
	generics: Option<AngleBracketedGenericArguments>,
) -> ItemImpl {
	let (existing_local_keys, existing_unsupported_items): (HashSet<ImplItem>, HashSet<ImplItem>) =
		local_impl
			.items
			.iter()
			.cloned()
			.partition(|impl_item| impl_item_ident(impl_item).is_some());
	let existing_local_keys: HashSet<Ident> = existing_local_keys
		.into_iter()
		.filter_map(|item| impl_item_ident(&item).cloned())
		.collect();
	let mut final_impl = local_impl;
	let extended_items = foreign_impl.items.into_iter().filter_map(|item| {
		if let Some(ident) = impl_item_ident(&item) {
			if existing_local_keys.contains(&ident) {
				// do not copy colliding items that have an ident
				return None;
			}
			if let ImplItem::Type(typ) = item.clone() {
				let cfg_attrs = typ
					.attrs
					.iter()
					.filter(|attr| attr.path().get_ident().map_or(false, |ident| ident == "cfg"))
					.map(|attr| attr.to_token_stream());
				if is_runtime_type(&typ) {
					let item: ImplItem = if inject_runtime_types {
						parse_quote! {
							#( #cfg_attrs )*
							type #ident = #ident;
						}
					} else {
						item
					};
					return Some(item);
				}
				// modify and insert uncolliding type items
				let modified_item: ImplItem = parse_quote! {
					#( #cfg_attrs )*
					type #ident = <#default_impl_path #generics as #disambiguation_path>::#ident;
				};
				return Some(modified_item);
			}
			// copy uncolliding non-type items that have an ident
			Some(item)
		} else {
			// do not copy colliding items that lack an ident
			(!existing_unsupported_items.contains(&item))
				// copy uncolliding items without an ident verbatim
				.then_some(item)
		}
	});
	final_impl.items.extend(extended_items);
	final_impl
}

/// Computes the disambiguation path for the `derive_impl` attribute macro.
///
/// When specified explicitly using `as [disambiguation_path]` in the macro attr, the
/// disambiguation is used as is. If not, we infer the disambiguation path from the
/// `foreign_impl_path` and the computed scope.
fn compute_disambiguation_path(
	disambiguation_path: Option<Path>,
	foreign_impl: ItemImpl,
	default_impl_path: Path,
) -> Result<Path> {
	match (disambiguation_path, foreign_impl.clone().trait_) {
		(Some(disambiguation_path), _) => Ok(disambiguation_path),
		(None, Some((_, foreign_impl_path, _))) => {
			if default_impl_path.segments.len() > 1 {
				let scope = default_impl_path.segments.first();
				Ok(parse_quote!(#scope :: #foreign_impl_path))
			} else {
				Ok(foreign_impl_path)
			}
		},
		_ => Err(syn::Error::new(
			default_impl_path.span(),
			"Impl statement must have a defined type being implemented \
			for a defined type such as `impl A for B`",
		)),
	}
}

/// Internal implementation behind [`#[derive_impl(..)]`](`macro@crate::derive_impl`).
///
/// `default_impl_path`: the module path of the external `impl` statement whose tokens we are
///	                     importing via `macro_magic`
///
/// `foreign_tokens`: the tokens for the external `impl` statement
///
/// `local_tokens`: the tokens for the local `impl` statement this attribute is attached to
///
/// `disambiguation_path`: the module path of the external trait we will use to qualify
///                        defaults imported from the external `impl` statement
pub fn derive_impl(
	default_impl_path: TokenStream2,
	foreign_tokens: TokenStream2,
	local_tokens: TokenStream2,
	disambiguation_path: Option<Path>,
	no_aggregated_types: Option<keyword::no_aggregated_types>,
	generics: Option<AngleBracketedGenericArguments>,
) -> Result<TokenStream2> {
	let local_impl = parse2::<ItemImpl>(local_tokens)?;
	let foreign_impl = parse2::<ItemImpl>(foreign_tokens)?;
	let default_impl_path = parse2::<Path>(default_impl_path)?;

	let disambiguation_path = compute_disambiguation_path(
		disambiguation_path,
		foreign_impl.clone(),
		default_impl_path.clone(),
	)?;

	// generate the combined impl
	let combined_impl = combine_impls(
		local_impl,
		foreign_impl,
		default_impl_path,
		disambiguation_path,
		no_aggregated_types.is_none(),
		generics,
	);

	Ok(quote!(#combined_impl))
}

#[test]
fn test_derive_impl_attr_args_parsing() {
	parse2::<DeriveImplAttrArgs>(quote!(
		some::path::TestDefaultConfig as some::path::DefaultConfig
	))
	.unwrap();
	parse2::<DeriveImplAttrArgs>(quote!(
		frame_system::prelude::testing::TestDefaultConfig as DefaultConfig
	))
	.unwrap();
	parse2::<DeriveImplAttrArgs>(quote!(Something as some::path::DefaultConfig)).unwrap();
	parse2::<DeriveImplAttrArgs>(quote!(Something as DefaultConfig)).unwrap();
	parse2::<DeriveImplAttrArgs>(quote!(DefaultConfig)).unwrap();
	assert!(parse2::<DeriveImplAttrArgs>(quote!()).is_err());
	assert!(parse2::<DeriveImplAttrArgs>(quote!(Config Config)).is_err());
}

#[test]
fn test_runtime_type_with_doc() {
	#[allow(dead_code)]
	trait TestTrait {
		type Test;
	}
	#[allow(unused)]
	struct TestStruct;
	let p = parse2::<ItemImpl>(quote!(
		impl TestTrait for TestStruct {
			/// Some doc
			#[inject_runtime_type]
			type Test = u32;
		}
	))
	.unwrap();
	for item in p.items {
		if let ImplItem::Type(typ) = item {
			assert_eq!(is_runtime_type(&typ), true);
		}
	}
}

#[test]
fn test_disambiguation_path() {
	let foreign_impl: ItemImpl = parse_quote!(impl SomeTrait for SomeType {});
	let default_impl_path: Path = parse_quote!(SomeScope::SomeType);

	// disambiguation path is specified
	let disambiguation_path = compute_disambiguation_path(
		Some(parse_quote!(SomeScope::SomePath)),
		foreign_impl.clone(),
		default_impl_path.clone(),
	);
	assert_eq!(disambiguation_path.unwrap(), parse_quote!(SomeScope::SomePath));

	// disambiguation path is not specified and the default_impl_path has more than one segment
	let disambiguation_path =
		compute_disambiguation_path(None, foreign_impl.clone(), default_impl_path.clone());
	assert_eq!(disambiguation_path.unwrap(), parse_quote!(SomeScope::SomeTrait));

	// disambiguation path is not specified and the default_impl_path has only one segment
	let disambiguation_path =
		compute_disambiguation_path(None, foreign_impl.clone(), parse_quote!(SomeType));
	assert_eq!(disambiguation_path.unwrap(), parse_quote!(SomeTrait));
}

#[test]
fn test_derive_impl_attr_args_parsing_with_generic() {
	let args = parse2::<DeriveImplAttrArgs>(quote!(
		some::path::TestDefaultConfig<Config> as some::path::DefaultConfig
	))
	.unwrap();
	assert_eq!(args.default_impl_path, parse_quote!(some::path::TestDefaultConfig));
	assert_eq!(args.generics.unwrap().args[0], parse_quote!(Config));
	let args = parse2::<DeriveImplAttrArgs>(quote!(TestDefaultConfig<Config2>)).unwrap();
	assert_eq!(args.default_impl_path, parse_quote!(TestDefaultConfig));
	assert_eq!(args.generics.unwrap().args[0], parse_quote!(Config2));
}
