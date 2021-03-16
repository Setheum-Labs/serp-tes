//! # SERP-TES Module
//!
//! ## Overview
//!
//! The SERP-TES (Setheum Elastic Reserve Protocol - Token Elasticity of Supply) 
//! module provides a token elasticity system for the SERP-STP258 mixed stablecoin system, 
//! by configuring an expansion which implements an `expand_supply` (Sett-Mint) to expand stablecoin supply
//! and a `contract_supply` (Dinar-Mint) which contracts the stablecoin supply.
//!
//! Then to determine whether the SERP should expand or contract supply, the TES provides
//! a `serp_elast` to tell the TES when to expand and when to contract supply depending on 
//! the outcome of the price of the stablecoin.
//!
//! The serp-tes module provides functionality of both the `Stp258` module that needs 
//! to contract and expand the supply of its currencies for its stablecoin system stability 
//! and the `SerpMarket` module that needs to trade/auction the currencies minted and 
//! contracted by the `SerpTes` module, which it has to do with the `SerpStaking` module to be 
//! built in the next Milestone of the Serp Modules.
//! 
//! The `SerpTes` module depends on the `FetchPrice` module to feed the prices of the 
//! currencies in to adjust the stablecoin supply.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use sp_std::prelude::*;
use codec::{Decode, Encode};
use core::cmp::{max, min, Ord, Ordering};
use fixed::{types::extra::U64, FixedU128};
use frame_support::pallet_prelude::*;
use stp258_traits::{
	arithmetic::{Signed, SimpleArithmetic},
	DataProvider as SerpTesProvider,
	serp_market::SerpMarket,
	Stp258Asset, Stp258Currency,
};
use num_rational::Ratio;
use sp_runtime::{
	traits::{CheckedMul, Zero, CheckedSub, CheckedAdd, MaybeSerializeDeserialize, StaticLookup},
	PerThing, Perbill, RuntimeDebug,
};
use sp_std::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
	marker, result,
};
use frame_system::{ensure_signed, pallet_prelude::*};

mod mock;
mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {

	pub(crate) type BalanceOf<T> =
		<<T as Config>::Stp258StableCurrency as SettCurrency<<T as frame_system::Config>::AccountId>>::Balance;
	pub(crate) type CurrencyIdOf<T> =
		<<T as Config>::Stp258StableCurrency as SettCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;
	pub(crate) type AccountIdOf<T> =
		<<T as Config>::Stp258Currency as SettCurrency<<T as frame_system::Config>::AccountId>>::AccountId;

	/// The pallet's configuration trait.
	pub trait  Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The price type
		type Price = Parameter + Member + AtLeast32BitUnsigned + Default + Copy + MaybeSerializeDeserialize;

		/// The base_unit type
		type BaseUnit = Parameter + Member + AtLeast32BitUnsigned + Default + Copy + MaybeSerializeDeserialize;

		/// The currency ID type
		type CurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord;

		/// The stable currency (SettCurrency) type
		type SettCurrency: SettCurrency<Self::AccountId>;

		/// The frequency of adjustments of the SettCurrency supply.
		type ElastAdjustmentFrequency: Get<<Self as system::Trait>::BlockNumber;

		/// The base_unit getter
		#[pallet::constant]
		type GetBaseUnit: Get<BaseUnit>;
	}

	// Errors inform users that something went wrong.
	// The possible errors returned by calls to this pallet's functions.
	#[pallet::error]
	pub enum Error<T> {
		/// Some wrong behavior
		Wrong,
		/// Something went very wrong and the price of the currency is zero.
		ZeroPrice,
		/// While trying to expand the supply, it overflowed.
		SupplyOverflow,
		/// While trying to contract the supply, it underflowed.
		SupplyUnderflow,
	}
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Serp Expand Supply successful. [currency_id, who, amount]
		SerpedUpSupply(CurrencyIdOf<T>, BalanceOf<T>),
		/// Serp Contract Supply successful. [currency_id, who, amount]
		SerpedDownSupply(CurrencyIdOf<T>, BalanceOf<T>),
		/// The New Price of Currency. [currency_id, price]
		NewPrice(CurrencyIdOf<T>, BalanceOf<T>),
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {

		// Dispatchable functions allows users to interact with the pallet and invoke state changes.
		// These functions materialize as "extrinsics", which are often compared to transactions.
		// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
		//

		/// Adjust the amount of SettCurrency according to the price.
		///
		/// **Weight:**
		/// - complexity: `O(F + P)`
		///   - `F` being the complexity of `T::SerpMarket::Price::get_stable_price()`
		///   - `P` being the complexity of `on_block_with_price`
		#[weight = 0]
		fn on_initialize(n: T::BlockNumber) {
			let price = T::SerpMarket::Price::get_stable_price();
			Self::on_block_with_price(n, price).unwrap_or_else(|e| {
				native::error!("could not adjust supply: {:?}", e);
			});
		}	

		// on block - adjustment frequency

		/// Contracts or expands the supply based on conditions.
		///
		/// **Weight:**
		/// Calls `serp_elast` (expand_or_contract_on_price) every `ElastAdjustmentFrequency` blocks.
		/// - complexity: `O(P)` with `P` being the complexity of `serp_elast`
		#[weight = 0]
		fn on_block_with_price(block: T::BlockNumber, price: Price) -> DispatchResult {
			// This can be changed to only correct for small or big price swings.
			if block % T::ElastAdjustmentFrequency::get() == 0.into() {
				Self::serp_elast(price)        
			} else {
				Ok(())
			}
		}

		/// Calculate the amount of supply change from a fraction given as `numerator` and `denominator`.
		fn calculate_supply_change(currency_id: CurrencyIdOf<T>, new_price: BalanceOf<T>) -> Self::Balance {
			let base_unit = T::GetBaseUnit::get(); 
			let supply = T::Stp258Currency::total_issuance(currency_id);
			let fraction = new_price / base_unit;
			let fractioned = fraction.saturating_sub(1);
			fractioned.saturating_mul_int(supply);
			Ok(())
		}

		/// Expands (if the price is too high) or contracts (if the price is too low) the SettCurrency supply.
		///
		/// **Weight:**
		/// - complexity: `O(S + C)`
		///   - `S` being the complexity of executing either `expand_supply` or `contract_supply`
		///   - `C` being a constant amount of storage reads for SettCurrency supply
		/// - DB access:
		///   - 1 read for total_issuance
		///   - execute `expand_supply` OR execute `contract_supply` which have DB accesses
		#[weight = 0]
		fn serp_elast(currency_id: CurrencyId, price: Price) -> DispatchResult {
			let base_unit = T::GetBaseUnit;
			let price = T::SerpMarket::Price::get_stable_price(currency_id, quote_price: Price);
			match price {
				0 => {
					native::error!("currency price is zero!");
					return Err(DispatchError::from(Error::<T>::ZeroPrice));
				}
				price if price > base_unit => {
					// safe from underflow because `price` is checked to be less than `GetBaseUnit`
					let expand_by = Self::calculate_currency_supply_change(currency_id, price);
					T::SerpMarket::expand_supply(currency_id, expand_by)?;
				}
				price if price < base_unit => {
					// safe from underflow because `price` is checked to be greater than `GetBaseUnit`
					let contract_by = Self::calculate_currency_supply_change(currency_id, price);
					T::SerpMarket::contract_supply(currency_id, contract_by)?;
				}
				_ => {
					native::info!("settcurrency price is equal to base as is desired --> nothing to do");
				}
			}
			Ok(())
		}
	}
}
