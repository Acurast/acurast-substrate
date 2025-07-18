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
use frame_support_procedural_tools::{get_cfg_attributes, get_doc_literals, is_using_frame_crate};
use quote::ToTokens;
use syn::{spanned::Spanned, token, Token, TraitItemType};

/// List of additional token to be used for parsing.
mod keyword {
	syn::custom_keyword!(Config);
	syn::custom_keyword!(From);
	syn::custom_keyword!(T);
	syn::custom_keyword!(I);
	syn::custom_keyword!(config);
	syn::custom_keyword!(pallet);
	syn::custom_keyword!(IsType);
	syn::custom_keyword!(RuntimeEvent);
	syn::custom_keyword!(Event);
	syn::custom_keyword!(frame_system);
	syn::custom_keyword!(disable_frame_system_supertrait_check);
	syn::custom_keyword!(no_default);
	syn::custom_keyword!(no_default_bounds);
	syn::custom_keyword!(constant);
	syn::custom_keyword!(include_metadata);
}

#[derive(Default)]
pub struct DefaultTrait {
	/// A bool for each sub-trait item indicates whether the item has
	/// `#[pallet::no_default_bounds]` attached to it. If true, the item will not have any bounds
	/// in the generated default sub-trait.
	pub items: Vec<(syn::TraitItem, bool)>,
	pub has_system: bool,
}

/// Input definition for the pallet config.
pub struct ConfigDef {
	/// The index of item in pallet module.
	pub index: usize,
	/// Whether the trait has instance (i.e. define with `Config<I = ()>`)
	pub has_instance: bool,
	/// Const associated type.
	pub consts_metadata: Vec<ConstMetadataDef>,
	/// Associated types metadata.
	pub associated_types_metadata: Vec<AssociatedTypeMetadataDef>,
	/// Whether the trait has the associated type `Event`, note that those bounds are
	/// checked:
	/// * `IsType<Self as frame_system::Config>::RuntimeEvent`
	/// * `From<Event>` or `From<Event<T>>` or `From<Event<T, I>>`
	pub has_event_type: bool,
	/// The where clause on trait definition but modified so `Self` is `T`.
	pub where_clause: Option<syn::WhereClause>,
	/// Whether a default sub-trait should be generated.
	///
	/// Contains default sub-trait items (instantiated by `#[pallet::config(with_default)]`).
	/// Vec will be empty if `#[pallet::config(with_default)]` is not specified or if there are
	/// no trait items.
	pub default_sub_trait: Option<DefaultTrait>,
}

/// Input definition for an associated type in pallet config.
pub struct AssociatedTypeMetadataDef {
	/// Name of the associated type.
	pub ident: syn::Ident,
	/// The doc associated.
	pub doc: Vec<syn::Expr>,
	/// The cfg associated.
	pub cfg: Vec<syn::Attribute>,
}

impl From<&syn::TraitItemType> for AssociatedTypeMetadataDef {
	fn from(trait_ty: &syn::TraitItemType) -> Self {
		let ident = trait_ty.ident.clone();
		let doc = get_doc_literals(&trait_ty.attrs);
		let cfg = get_cfg_attributes(&trait_ty.attrs);

		Self { ident, doc, cfg }
	}
}

/// Input definition for a constant in pallet config.
pub struct ConstMetadataDef {
	/// Name of the associated type.
	pub ident: syn::Ident,
	/// The type in Get, e.g. `u32` in `type Foo: Get<u32>;`, but `Self` is replaced by `T`
	pub type_: syn::Type,
	/// The doc associated
	pub doc: Vec<syn::Expr>,
	/// attributes
	pub attrs: Vec<syn::Attribute>,
}

impl TryFrom<&syn::TraitItemType> for ConstMetadataDef {
	type Error = syn::Error;

	fn try_from(trait_ty: &syn::TraitItemType) -> Result<Self, Self::Error> {
		let err = |span, msg| {
			syn::Error::new(span, format!("Invalid usage of `#[pallet::constant]`: {}", msg))
		};
		let doc = get_doc_literals(&trait_ty.attrs);
		let ident = trait_ty.ident.clone();
		let bound = trait_ty
			.bounds
			.iter()
			.find_map(|param_bound| {
				let syn::TypeParamBound::Trait(trait_bound) = param_bound else { return None };

				trait_bound.path.segments.last().and_then(|s| (s.ident == "Get").then(|| s))
			})
			.ok_or_else(|| err(trait_ty.span(), "`Get<T>` trait bound not found"))?;

		let syn::PathArguments::AngleBracketed(ref ab) = bound.arguments else {
			return Err(err(bound.span(), "Expected trait generic args"));
		};

		// Only one type argument is expected.
		if ab.args.len() != 1 {
			return Err(err(bound.span(), "Expected a single type argument"));
		}

		let syn::GenericArgument::Type(ref type_arg) = ab.args[0] else {
			return Err(err(ab.args[0].span(), "Expected a type argument"));
		};

		let type_ = syn::parse2::<syn::Type>(replace_self_by_t(type_arg.to_token_stream()))
			.expect("Internal error: replacing `Self` by `T` should result in valid type");

		Ok(Self { ident, type_, doc, attrs: trait_ty.attrs.clone() })
	}
}

/// Parse for `#[pallet::disable_frame_system_supertrait_check]`
pub struct DisableFrameSystemSupertraitCheck;

impl syn::parse::Parse for DisableFrameSystemSupertraitCheck {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		input.parse::<syn::Token![#]>()?;
		let content;
		syn::bracketed!(content in input);
		content.parse::<syn::Ident>()?;
		content.parse::<syn::Token![::]>()?;

		content.parse::<keyword::disable_frame_system_supertrait_check>()?;
		Ok(Self)
	}
}

/// Parsing for the `typ` portion of `PalletAttr`
#[derive(derive_syn_parse::Parse, PartialEq, Eq)]
pub enum PalletAttrType {
	#[peek(keyword::no_default, name = "no_default")]
	NoDefault(keyword::no_default),
	#[peek(keyword::no_default_bounds, name = "no_default_bounds")]
	NoBounds(keyword::no_default_bounds),
	#[peek(keyword::constant, name = "constant")]
	Constant(keyword::constant),
	#[peek(keyword::include_metadata, name = "include_metadata")]
	IncludeMetadata(keyword::include_metadata),
}

/// Parsing for `#[pallet::X]`
#[derive(derive_syn_parse::Parse)]
pub struct PalletAttr {
	_pound: Token![#],
	#[bracket]
	_bracket: token::Bracket,
	#[inside(_bracket)]
	_pallet: keyword::pallet,
	#[prefix(Token![::] in _bracket)]
	#[inside(_bracket)]
	typ: PalletAttrType,
}

/// Parse for `IsType<<Self as $path>::RuntimeEvent>` and retrieve `$path`
pub struct IsTypeBoundEventParse(syn::Path);

impl syn::parse::Parse for IsTypeBoundEventParse {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		input.parse::<keyword::IsType>()?;
		input.parse::<syn::Token![<]>()?;
		input.parse::<syn::Token![<]>()?;
		input.parse::<syn::Token![Self]>()?;
		input.parse::<syn::Token![as]>()?;
		let config_path = input.parse::<syn::Path>()?;
		input.parse::<syn::Token![>]>()?;
		input.parse::<syn::Token![::]>()?;
		input.parse::<keyword::RuntimeEvent>()?;
		input.parse::<syn::Token![>]>()?;

		Ok(Self(config_path))
	}
}

/// Parse for `From<Event>` or `From<Event<Self>>` or `From<Event<Self, I>>`
pub struct FromEventParse {
	is_generic: bool,
	has_instance: bool,
}

impl syn::parse::Parse for FromEventParse {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let mut is_generic = false;
		let mut has_instance = false;

		input.parse::<keyword::From>()?;
		input.parse::<syn::Token![<]>()?;
		input.parse::<keyword::Event>()?;
		if input.peek(syn::Token![<]) {
			is_generic = true;
			input.parse::<syn::Token![<]>()?;
			input.parse::<syn::Token![Self]>()?;
			if input.peek(syn::Token![,]) {
				input.parse::<syn::Token![,]>()?;
				input.parse::<keyword::I>()?;
				has_instance = true;
			}
			input.parse::<syn::Token![>]>()?;
		}
		input.parse::<syn::Token![>]>()?;

		Ok(Self { is_generic, has_instance })
	}
}

/// Check if trait_item is `type RuntimeEvent`, if so checks its bounds are those expected.
/// (Event type is reserved type)
fn check_event_type(
	frame_system: &syn::Path,
	trait_item: &syn::TraitItem,
	trait_has_instance: bool,
) -> syn::Result<bool> {
	let syn::TraitItem::Type(type_) = trait_item else { return Ok(false) };

	if type_.ident != "RuntimeEvent" {
		return Ok(false);
	}

	// Check event has no generics
	if !type_.generics.params.is_empty() || type_.generics.where_clause.is_some() {
		let msg =
			"Invalid `type RuntimeEvent`, associated type `RuntimeEvent` is reserved and must have\
					no generics nor where_clause";
		return Err(syn::Error::new(trait_item.span(), msg));
	}

	// Check bound contains IsType and From
	let has_is_type_bound = type_.bounds.iter().any(|s| {
		syn::parse2::<IsTypeBoundEventParse>(s.to_token_stream())
			.map_or(false, |b| has_expected_system_config(b.0, frame_system))
	});

	if !has_is_type_bound {
		let msg =
			"Invalid `type RuntimeEvent`, associated type `RuntimeEvent` is reserved and must \
					bound: `IsType<<Self as frame_system::Config>::RuntimeEvent>`"
				.to_string();
		return Err(syn::Error::new(type_.span(), msg));
	}

	let from_event_bound = type_
		.bounds
		.iter()
		.find_map(|s| syn::parse2::<FromEventParse>(s.to_token_stream()).ok());

	let Some(from_event_bound) = from_event_bound else {
		let msg =
			"Invalid `type RuntimeEvent`, associated type `RuntimeEvent` is reserved and must \
				bound: `From<Event>` or `From<Event<Self>>` or `From<Event<Self, I>>`";
		return Err(syn::Error::new(type_.span(), msg));
	};

	if from_event_bound.is_generic && (from_event_bound.has_instance != trait_has_instance) {
		let msg =
			"Invalid `type RuntimeEvent`, associated type `RuntimeEvent` bounds inconsistent \
					`From<Event..>`. Config and generic Event must be both with instance or \
					without instance";
		return Err(syn::Error::new(type_.span(), msg));
	}

	Ok(true)
}

/// Check that the path to `frame_system::Config` is valid, this is that the path is just
/// `frame_system::Config` or when using the `frame` crate it is
/// `polkadot_sdk_frame::xyz::frame_system::Config`.
fn has_expected_system_config(path: syn::Path, frame_system: &syn::Path) -> bool {
	// Check if `frame_system` is actually 'frame_system'.
	if path.segments.iter().all(|s| s.ident != "frame_system") {
		return false;
	}

	let mut expected_system_config =
		match (is_using_frame_crate(&path), is_using_frame_crate(&frame_system)) {
			(true, false) =>
			// We can't use the path to `frame_system` from `frame` if `frame_system` is not being
			// in scope through `frame`.
			{
				return false
			},
			(false, true) =>
			// We know that the only valid frame_system path is one that is `frame_system`, as
			// `frame` re-exports it as such.
			{
				syn::parse2::<syn::Path>(quote::quote!(frame_system)).expect("is a valid path; qed")
			},
			(_, _) =>
			// They are either both `frame_system` or both `polkadot_sdk_frame::xyz::frame_system`.
			{
				frame_system.clone()
			},
		};

	expected_system_config
		.segments
		.push(syn::PathSegment::from(syn::Ident::new("Config", path.span())));

	// the parse path might be something like `frame_system::Config<...>`, so we
	// only compare the idents along the path.
	expected_system_config
		.segments
		.into_iter()
		.map(|ps| ps.ident)
		.collect::<Vec<_>>()
		== path.segments.into_iter().map(|ps| ps.ident).collect::<Vec<_>>()
}

/// Replace ident `Self` by `T`
pub fn replace_self_by_t(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
	input
		.into_iter()
		.map(|token_tree| match token_tree {
			proc_macro2::TokenTree::Group(group) => {
				proc_macro2::Group::new(group.delimiter(), replace_self_by_t(group.stream())).into()
			},
			proc_macro2::TokenTree::Ident(ident) if ident == "Self" => {
				proc_macro2::Ident::new("T", ident.span()).into()
			},
			other => other,
		})
		.collect()
}

/// Check that the trait item requires the `TypeInfo` bound (or similar).
fn contains_type_info_bound(ty: &TraitItemType) -> bool {
	const KNOWN_TYPE_INFO_BOUNDS: &[&str] = &[
		// Explicit TypeInfo trait.
		"TypeInfo",
		// Implicit known substrate traits that implement type info.
		// Note: Aim to keep this list as small as possible.
		"Parameter",
	];

	ty.bounds.iter().any(|bound| {
		let syn::TypeParamBound::Trait(bound) = bound else { return false };

		KNOWN_TYPE_INFO_BOUNDS
			.iter()
			.any(|known| bound.path.segments.last().map_or(false, |last| last.ident == *known))
	})
}

impl ConfigDef {
	pub fn try_from(
		frame_system: &syn::Path,
		index: usize,
		item: &mut syn::Item,
		enable_default: bool,
		disable_associated_metadata: bool,
	) -> syn::Result<Self> {
		let syn::Item::Trait(item) = item else {
			let msg = "Invalid pallet::config, expected trait definition";
			return Err(syn::Error::new(item.span(), msg));
		};

		if !matches!(item.vis, syn::Visibility::Public(_)) {
			let msg = "Invalid pallet::config, trait must be public";
			return Err(syn::Error::new(item.span(), msg));
		}

		syn::parse2::<keyword::Config>(item.ident.to_token_stream())?;

		let where_clause = {
			let stream = replace_self_by_t(item.generics.where_clause.to_token_stream());
			syn::parse2::<Option<syn::WhereClause>>(stream).expect(
				"Internal error: replacing `Self` by `T` should result in valid where
					clause",
			)
		};

		if item.generics.params.len() > 1 {
			let msg = "Invalid pallet::config, expected no more than one generic";
			return Err(syn::Error::new(item.generics.params[2].span(), msg));
		}

		let has_instance = if item.generics.params.first().is_some() {
			helper::check_config_def_gen(&item.generics, item.ident.span())?;
			true
		} else {
			false
		};

		let has_frame_system_supertrait = item.supertraits.iter().any(|s| {
			syn::parse2::<syn::Path>(s.to_token_stream())
				.map_or(false, |b| has_expected_system_config(b, frame_system))
		});

		let mut has_event_type = false;
		let mut consts_metadata = vec![];
		let mut associated_types_metadata = vec![];
		let mut default_sub_trait = if enable_default {
			Some(DefaultTrait {
				items: Default::default(),
				has_system: has_frame_system_supertrait,
			})
		} else {
			None
		};
		for trait_item in &mut item.items {
			let is_event = check_event_type(frame_system, trait_item, has_instance)?;
			has_event_type = has_event_type || is_event;

			let mut already_no_default = false;
			let mut already_constant = false;
			let mut already_no_default_bounds = false;
			let mut already_collected_associated_type = None;

			while let Ok(Some(pallet_attr)) =
				helper::take_first_item_pallet_attr::<PalletAttr>(trait_item)
			{
				match (pallet_attr.typ, &trait_item) {
					(PalletAttrType::Constant(_), syn::TraitItem::Type(ref typ)) => {
						if already_constant {
							return Err(syn::Error::new(
								pallet_attr._bracket.span.join(),
								"Duplicate #[pallet::constant] attribute not allowed.",
							));
						}
						already_constant = true;
						consts_metadata.push(ConstMetadataDef::try_from(typ)?);
					},
					(PalletAttrType::Constant(_), _) =>
						return Err(syn::Error::new(
							trait_item.span(),
							"Invalid #[pallet::constant] in #[pallet::config], expected type item",
						)),
					// Pallet developer has explicitly requested to include metadata for this associated type.
					//
					// They must provide a type item that implements `TypeInfo`.
					(PalletAttrType::IncludeMetadata(_), syn::TraitItem::Type(ref typ)) => {
						if already_collected_associated_type.is_some() {
							return Err(syn::Error::new(
								pallet_attr._bracket.span.join(),
								"Duplicate #[pallet::include_metadata] attribute not allowed.",
							));
						}
						already_collected_associated_type = Some(pallet_attr._bracket.span.join());
						associated_types_metadata.push(AssociatedTypeMetadataDef::from(AssociatedTypeMetadataDef::from(typ)));
					}
					(PalletAttrType::IncludeMetadata(_), _) =>
						return Err(syn::Error::new(
							pallet_attr._bracket.span.join(),
							"Invalid #[pallet::include_metadata] in #[pallet::config], expected type item",
						)),
					(PalletAttrType::NoDefault(_), _) => {
						if !enable_default {
							return Err(syn::Error::new(
								pallet_attr._bracket.span.join(),
								"`#[pallet::no_default]` can only be used if `#[pallet::config(with_default)]` \
								has been specified"
							));
						}
						if already_no_default {
							return Err(syn::Error::new(
								pallet_attr._bracket.span.join(),
								"Duplicate #[pallet::no_default] attribute not allowed.",
							));
						}

						already_no_default = true;
					},
					(PalletAttrType::NoBounds(_), _) => {
						if !enable_default {
							return Err(syn::Error::new(
								pallet_attr._bracket.span.join(),
								"`#[pallet:no_default_bounds]` can only be used if `#[pallet::config(with_default)]` \
								has been specified"
							));
						}
						if already_no_default_bounds {
							return Err(syn::Error::new(
								pallet_attr._bracket.span.join(),
								"Duplicate #[pallet::no_default_bounds] attribute not allowed.",
							));
						}
						already_no_default_bounds = true;
					},
				}
			}

			if let Some(span) = already_collected_associated_type {
				// Events and constants are already propagated to the metadata
				if is_event {
					return Err(syn::Error::new(
						span,
						"Invalid #[pallet::include_metadata] for `type RuntimeEvent`. \
						The associated type `RuntimeEvent` is already collected in the metadata.",
					));
				}

				if already_constant {
					return Err(syn::Error::new(
						span,
						"Invalid #[pallet::include_metadata]: conflict with #[pallet::constant]. \
						Pallet constant already collect the metadata for the type.",
					));
				}

				if let syn::TraitItem::Type(ref ty) = trait_item {
					if !contains_type_info_bound(ty) {
						let msg = format!(
						"Invalid #[pallet::include_metadata] in #[pallet::config], collected type `{}` \
						does not implement `TypeInfo` or `Parameter`",
						ty.ident,
					);
						return Err(syn::Error::new(span, msg));
					}
				}
			} else {
				// Metadata of associated types is collected by default, if the associated type
				// implements `TypeInfo`, or a similar trait that requires the `TypeInfo` bound.
				if !disable_associated_metadata && !is_event && !already_constant {
					if let syn::TraitItem::Type(ref ty) = trait_item {
						// Collect the metadata of the associated type if it implements `TypeInfo`.
						if contains_type_info_bound(ty) {
							associated_types_metadata.push(AssociatedTypeMetadataDef::from(ty));
						}
					}
				}
			}

			if !already_no_default && enable_default {
				default_sub_trait
					.as_mut()
					.expect("is 'Some(_)' if 'enable_default'; qed")
					.items
					.push((trait_item.clone(), already_no_default_bounds));
			}
		}

		let attr: Option<DisableFrameSystemSupertraitCheck> =
			helper::take_first_item_pallet_attr(&mut item.attrs)?;
		let disable_system_supertrait_check = attr.is_some();

		if !has_frame_system_supertrait && !disable_system_supertrait_check {
			let found = if item.supertraits.is_empty() {
				"none".to_string()
			} else {
				let mut found = item
					.supertraits
					.iter()
					.fold(String::new(), |acc, s| format!("{}`{}`, ", acc, quote::quote!(#s)));
				found.pop();
				found.pop();
				found
			};

			let msg = format!(
				"Invalid pallet::trait, expected explicit `{}::Config` as supertrait, \
				found {}. \
				(try `pub trait Config: frame_system::Config {{ ...` or \
				`pub trait Config<I: 'static>: frame_system::Config {{ ...`). \
				To disable this check, use `#[pallet::disable_frame_system_supertrait_check]`",
				frame_system.to_token_stream(),
				found,
			);
			return Err(syn::Error::new(item.span(), msg));
		}

		Ok(Self {
			index,
			has_instance,
			consts_metadata,
			associated_types_metadata,
			has_event_type,
			where_clause,
			default_sub_trait,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn has_expected_system_config_works() {
		let frame_system = syn::parse2::<syn::Path>(quote::quote!(frame_system)).unwrap();
		let path = syn::parse2::<syn::Path>(quote::quote!(frame_system::Config)).unwrap();
		assert!(has_expected_system_config(path, &frame_system));
	}

	#[test]
	fn has_expected_system_config_works_with_assoc_type() {
		let frame_system = syn::parse2::<syn::Path>(quote::quote!(frame_system)).unwrap();
		let path =
			syn::parse2::<syn::Path>(quote::quote!(frame_system::Config<RuntimeCall = Call>))
				.unwrap();
		assert!(has_expected_system_config(path, &frame_system));
	}

	#[test]
	fn has_expected_system_config_works_with_frame() {
		let path = syn::parse2::<syn::Path>(quote::quote!(frame_system::Config)).unwrap();

		let frame_system =
			syn::parse2::<syn::Path>(quote::quote!(polkadot_sdk_frame::deps::frame_system))
				.unwrap();
		assert!(has_expected_system_config(path.clone(), &frame_system));

		let frame_system =
			syn::parse2::<syn::Path>(quote::quote!(frame::deps::frame_system)).unwrap();
		assert!(has_expected_system_config(path, &frame_system));
	}

	#[test]
	fn has_expected_system_config_works_with_frame_full_path() {
		let frame_system =
			syn::parse2::<syn::Path>(quote::quote!(polkadot_sdk_frame::deps::frame_system))
				.unwrap();
		let path =
			syn::parse2::<syn::Path>(quote::quote!(polkadot_sdk_frame::deps::frame_system::Config))
				.unwrap();
		assert!(has_expected_system_config(path, &frame_system));

		let frame_system =
			syn::parse2::<syn::Path>(quote::quote!(frame::deps::frame_system)).unwrap();
		let path =
			syn::parse2::<syn::Path>(quote::quote!(frame::deps::frame_system::Config)).unwrap();
		assert!(has_expected_system_config(path, &frame_system));
	}

	#[test]
	fn has_expected_system_config_works_with_other_frame_full_path() {
		let frame_system =
			syn::parse2::<syn::Path>(quote::quote!(polkadot_sdk_frame::xyz::frame_system)).unwrap();
		let path =
			syn::parse2::<syn::Path>(quote::quote!(polkadot_sdk_frame::xyz::frame_system::Config))
				.unwrap();
		assert!(has_expected_system_config(path, &frame_system));

		let frame_system =
			syn::parse2::<syn::Path>(quote::quote!(frame::xyz::frame_system)).unwrap();
		let path =
			syn::parse2::<syn::Path>(quote::quote!(frame::xyz::frame_system::Config)).unwrap();
		assert!(has_expected_system_config(path, &frame_system));
	}

	#[test]
	fn has_expected_system_config_does_not_works_with_mixed_frame_full_path() {
		let frame_system =
			syn::parse2::<syn::Path>(quote::quote!(polkadot_sdk_frame::xyz::frame_system)).unwrap();
		let path =
			syn::parse2::<syn::Path>(quote::quote!(polkadot_sdk_frame::deps::frame_system::Config))
				.unwrap();
		assert!(!has_expected_system_config(path, &frame_system));
	}

	#[test]
	fn has_expected_system_config_does_not_works_with_other_mixed_frame_full_path() {
		let frame_system =
			syn::parse2::<syn::Path>(quote::quote!(polkadot_sdk_frame::deps::frame_system))
				.unwrap();
		let path =
			syn::parse2::<syn::Path>(quote::quote!(polkadot_sdk_frame::xyz::frame_system::Config))
				.unwrap();
		assert!(!has_expected_system_config(path, &frame_system));
	}

	#[test]
	fn has_expected_system_config_does_not_work_with_frame_full_path_if_not_frame_crate() {
		let frame_system = syn::parse2::<syn::Path>(quote::quote!(frame_system)).unwrap();
		let path =
			syn::parse2::<syn::Path>(quote::quote!(polkadot_sdk_frame::deps::frame_system::Config))
				.unwrap();
		assert!(!has_expected_system_config(path, &frame_system));
	}

	#[test]
	fn has_expected_system_config_unexpected_frame_system() {
		let frame_system =
			syn::parse2::<syn::Path>(quote::quote!(framez::deps::frame_system)).unwrap();
		let path = syn::parse2::<syn::Path>(quote::quote!(frame_system::Config)).unwrap();
		assert!(!has_expected_system_config(path, &frame_system));
	}

	#[test]
	fn has_expected_system_config_unexpected_path() {
		let frame_system = syn::parse2::<syn::Path>(quote::quote!(frame_system)).unwrap();
		let path = syn::parse2::<syn::Path>(quote::quote!(frame_system::ConfigSystem)).unwrap();
		assert!(!has_expected_system_config(path, &frame_system));
	}

	#[test]
	fn has_expected_system_config_not_frame_system() {
		let frame_system = syn::parse2::<syn::Path>(quote::quote!(something)).unwrap();
		let path = syn::parse2::<syn::Path>(quote::quote!(something::Config)).unwrap();
		assert!(!has_expected_system_config(path, &frame_system));
	}
}
