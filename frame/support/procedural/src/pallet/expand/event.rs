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

use crate::{
	pallet::{parse::event::PalletEventDepositAttr, Def},
	COUNTER,
};
use frame_support_procedural_tools::get_doc_literals;
use syn::{spanned::Spanned, Ident};

///
/// * Add __Ignore variant on Event
/// * Impl various trait on Event including metadata
/// * if deposit_event is defined, implement deposit_event on module.
pub fn expand_event(def: &mut Def) -> proc_macro2::TokenStream {
	let count = COUNTER.with(|counter| counter.borrow_mut().inc());

	let (event, macro_ident) = if let Some(event) = &def.event {
		let ident = Ident::new(&format!("__is_event_part_defined_{}", count), event.attr_span);
		(event, ident)
	} else {
		let macro_ident =
			Ident::new(&format!("__is_event_part_defined_{}", count), def.item.span());

		return quote::quote! {
			#[doc(hidden)]
			pub mod __substrate_event_check {
				#[macro_export]
				#[doc(hidden)]
				macro_rules! #macro_ident {
					($pallet_name:ident) => {
						compile_error!(concat!(
							"`",
							stringify!($pallet_name),
							"` does not have #[pallet::event] defined, perhaps you should \
							remove `Event` from construct_runtime?",
						));
					}
				}

				#[doc(hidden)]
				pub use #macro_ident as is_event_part_defined;
			}
		};
	};

	let event_where_clause = &event.where_clause;

	// NOTE: actually event where clause must be a subset of config where clause because of
	// `type RuntimeEvent: From<Event<Self>>`. But we merge either way for potential better error
	// message
	let completed_where_clause =
		super::merge_where_clauses(&[&event.where_clause, &def.config.where_clause]);

	let event_ident = &event.event;
	let frame_system = &def.frame_system;
	let frame_support = &def.frame_support;
	let event_use_gen = &event.gen_kind.type_use_gen(event.attr_span);
	let event_impl_gen = &event.gen_kind.type_impl_gen(event.attr_span);
	let event_item = {
		let item = &mut def.item.content.as_mut().expect("Checked by def parser").1[event.index];
		if let syn::Item::Enum(item) = item {
			item
		} else {
			unreachable!("Checked by event parser")
		}
	};

	// Phantom data is added for generic event.
	if event.gen_kind.is_generic() {
		let variant = syn::parse_quote!(
			#[doc(hidden)]
			#[codec(skip)]
			__Ignore(
				::core::marker::PhantomData<(#event_use_gen)>,
				#frame_support::Never,
			)
		);

		// Push ignore variant at the end.
		event_item.variants.push(variant);
	}

	let deprecation = match crate::deprecation::get_deprecation_enum(
		&quote::quote! {#frame_support},
		&event.attrs,
		event_item.variants.iter().enumerate().map(|(index, item)| {
			let index = crate::deprecation::variant_index_for_deprecation(index as u8, item);

			(index, item.attrs.as_ref())
		}),
	) {
		Ok(deprecation) => deprecation,
		Err(e) => return e.into_compile_error(),
	};

	if get_doc_literals(&event_item.attrs).is_empty() {
		event_item
			.attrs
			.push(syn::parse_quote!(#[doc = "The `Event` enum of this pallet"]));
	}

	// derive some traits because system event require Clone, FullCodec, Eq, PartialEq and Debug
	event_item.attrs.push(syn::parse_quote!(
		#[derive(
			#frame_support::CloneNoBound,
			#frame_support::EqNoBound,
			#frame_support::PartialEqNoBound,
			#frame_support::DebugNoBound,
			#frame_support::__private::codec::Encode,
			#frame_support::__private::codec::Decode,
			#frame_support::__private::codec::DecodeWithMemTracking,
			#frame_support::__private::scale_info::TypeInfo,
		)]
	));

	let capture_docs = if cfg!(feature = "no-metadata-docs") { "never" } else { "always" };

	// skip requirement for type params to implement `TypeInfo`, and set docs capture
	event_item.attrs.push(syn::parse_quote!(
		#[scale_info(skip_type_params(#event_use_gen), capture_docs = #capture_docs)]
	));

	let deposit_event = if let Some(deposit_event) = &event.deposit_event {
		let event_use_gen = &event.gen_kind.type_use_gen(event.attr_span);
		let trait_use_gen = &def.trait_use_generics(event.attr_span);
		let type_impl_gen = &def.type_impl_generics(event.attr_span);
		let type_use_gen = &def.type_use_generics(event.attr_span);
		let pallet_ident = &def.pallet_struct.pallet;

		let PalletEventDepositAttr { fn_vis, fn_span, .. } = deposit_event;

		quote::quote_spanned!(*fn_span =>
			impl<#type_impl_gen> #pallet_ident<#type_use_gen> #completed_where_clause {
				#fn_vis fn deposit_event(event: Event<#event_use_gen>) {
					let event = <
						<T as Config #trait_use_gen>::RuntimeEvent as
						From<Event<#event_use_gen>>
					>::from(event);

					let event = <
						<T as Config #trait_use_gen>::RuntimeEvent as
						Into<<T as #frame_system::Config>::RuntimeEvent>
					>::into(event);

					<#frame_system::Pallet<T>>::deposit_event(event)
				}
			}
		)
	} else {
		Default::default()
	};

	quote::quote_spanned!(event.attr_span =>
		#[doc(hidden)]
		pub mod __substrate_event_check {
			#[macro_export]
			#[doc(hidden)]
			macro_rules! #macro_ident {
				($pallet_name:ident) => {};
			}

			#[doc(hidden)]
			pub use #macro_ident as is_event_part_defined;
		}

		#deposit_event

		impl<#event_impl_gen> From<#event_ident<#event_use_gen>> for () #event_where_clause {
			fn from(_: #event_ident<#event_use_gen>) {}
		}

		impl<#event_impl_gen> #event_ident<#event_use_gen> #event_where_clause {
			#[allow(dead_code)]
			#[doc(hidden)]
			pub fn event_metadata<W: #frame_support::__private::scale_info::TypeInfo + 'static>() -> #frame_support::__private::metadata_ir::PalletEventMetadataIR {
				#frame_support::__private::metadata_ir::PalletEventMetadataIR {
					ty: #frame_support::__private::scale_info::meta_type::<W>(),
					deprecation_info: #deprecation,
				}
			}
		}
	)
}
