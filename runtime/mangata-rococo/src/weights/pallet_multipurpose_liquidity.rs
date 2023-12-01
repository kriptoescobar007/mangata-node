// This file is part of Mangata.

// Copyright (C) 2020-2022 Mangata Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Autogenerated weights for pallet_multipurpose_liquidity
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-11-29, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: , WASM-EXECUTION: Compiled, CHAIN: Some("mangata-kusama"), DB CACHE: 1024

// Executed Command:
// target/release/mangata-node
// benchmark
// pallet
// -l=info,runtime::collective=warn,xyk=warn
// --chain
// mangata-kusama
// --execution
// wasm
// --wasm-execution
// compiled
// --pallet
// *
// --extrinsic
// *
// --steps
// 50
// --repeat
// 20
// --template
// ./templates/module-weight-template.hbs
// --output
// ./benchmarks/

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_multipurpose_liquidity.
pub trait WeightInfo {
	fn reserve_vesting_liquidity_tokens() -> Weight;
	fn unreserve_and_relock_instance() -> Weight;
}

/// Weights for pallet_multipurpose_liquidity using the Mangata node and recommended hardware.
pub struct ModuleWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_multipurpose_liquidity::WeightInfo for ModuleWeight<T> {
	// Storage: `Xyk::LiquidityPools` (r:1 w:0)
	// Proof: `Xyk::LiquidityPools` (`max_values`: None, `max_size`: Some(41), added: 2516, mode: `MaxEncodedLen`)
	// Storage: `Vesting::Vesting` (r:1 w:1)
	// Proof: `Vesting::Vesting` (`max_values`: None, `max_size`: Some(1869), added: 4344, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Locks` (r:1 w:1)
	// Proof: `Tokens::Locks` (`max_values`: None, `max_size`: Some(1261), added: 3736, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Accounts` (r:1 w:1)
	// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	// Storage: `MultiPurposeLiquidity::ReserveStatus` (r:1 w:1)
	// Proof: `MultiPurposeLiquidity::ReserveStatus` (`max_values`: None, `max_size`: Some(124), added: 2599, mode: `MaxEncodedLen`)
	// Storage: `MultiPurposeLiquidity::RelockStatus` (r:1 w:1)
	// Proof: `MultiPurposeLiquidity::RelockStatus` (`max_values`: None, `max_size`: Some(1845), added: 4320, mode: `MaxEncodedLen`)
	fn reserve_vesting_liquidity_tokens() -> Weight {
		(Weight::from_parts(129_510_000, 0))
			.saturating_add(T::DbWeight::get().reads(6 as u64))
			.saturating_add(T::DbWeight::get().writes(5 as u64))
	}
	// Storage: `MultiPurposeLiquidity::RelockStatus` (r:1 w:1)
	// Proof: `MultiPurposeLiquidity::RelockStatus` (`max_values`: None, `max_size`: Some(1845), added: 4320, mode: `MaxEncodedLen`)
	// Storage: `MultiPurposeLiquidity::ReserveStatus` (r:1 w:1)
	// Proof: `MultiPurposeLiquidity::ReserveStatus` (`max_values`: None, `max_size`: Some(124), added: 2599, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Accounts` (r:1 w:1)
	// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	// Storage: `Vesting::Vesting` (r:1 w:1)
	// Proof: `Vesting::Vesting` (`max_values`: None, `max_size`: Some(1869), added: 4344, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Locks` (r:1 w:1)
	// Proof: `Tokens::Locks` (`max_values`: None, `max_size`: Some(1261), added: 3736, mode: `MaxEncodedLen`)
	fn unreserve_and_relock_instance() -> Weight {
		(Weight::from_parts(125_040_000, 0))
			.saturating_add(T::DbWeight::get().reads(5 as u64))
			.saturating_add(T::DbWeight::get().writes(5 as u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: `Xyk::LiquidityPools` (r:1 w:0)
	// Proof: `Xyk::LiquidityPools` (`max_values`: None, `max_size`: Some(41), added: 2516, mode: `MaxEncodedLen`)
	// Storage: `Vesting::Vesting` (r:1 w:1)
	// Proof: `Vesting::Vesting` (`max_values`: None, `max_size`: Some(1869), added: 4344, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Locks` (r:1 w:1)
	// Proof: `Tokens::Locks` (`max_values`: None, `max_size`: Some(1261), added: 3736, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Accounts` (r:1 w:1)
	// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	// Storage: `MultiPurposeLiquidity::ReserveStatus` (r:1 w:1)
	// Proof: `MultiPurposeLiquidity::ReserveStatus` (`max_values`: None, `max_size`: Some(124), added: 2599, mode: `MaxEncodedLen`)
	// Storage: `MultiPurposeLiquidity::RelockStatus` (r:1 w:1)
	// Proof: `MultiPurposeLiquidity::RelockStatus` (`max_values`: None, `max_size`: Some(1845), added: 4320, mode: `MaxEncodedLen`)
	fn reserve_vesting_liquidity_tokens() -> Weight {
		(Weight::from_parts(129_510_000, 0))
			.saturating_add(RocksDbWeight::get().reads(6 as u64))
			.saturating_add(RocksDbWeight::get().writes(5 as u64))
	}
	// Storage: `MultiPurposeLiquidity::RelockStatus` (r:1 w:1)
	// Proof: `MultiPurposeLiquidity::RelockStatus` (`max_values`: None, `max_size`: Some(1845), added: 4320, mode: `MaxEncodedLen`)
	// Storage: `MultiPurposeLiquidity::ReserveStatus` (r:1 w:1)
	// Proof: `MultiPurposeLiquidity::ReserveStatus` (`max_values`: None, `max_size`: Some(124), added: 2599, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Accounts` (r:1 w:1)
	// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	// Storage: `Vesting::Vesting` (r:1 w:1)
	// Proof: `Vesting::Vesting` (`max_values`: None, `max_size`: Some(1869), added: 4344, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Locks` (r:1 w:1)
	// Proof: `Tokens::Locks` (`max_values`: None, `max_size`: Some(1261), added: 3736, mode: `MaxEncodedLen`)
	fn unreserve_and_relock_instance() -> Weight {
		(Weight::from_parts(125_040_000, 0))
			.saturating_add(RocksDbWeight::get().reads(5 as u64))
			.saturating_add(RocksDbWeight::get().writes(5 as u64))
	}
}
