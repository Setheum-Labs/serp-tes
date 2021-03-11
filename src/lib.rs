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
	price::PriceProvider as TesPriceProvider,
	serp_tes::SerpTes,
	Stp258Asset, Stp258Currency, Stp258CurrencyExtended, 
	Stp258CurrencyReservable, Stp258AssetReservable,
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
	use super::*;

	pub(crate) type BalanceOf<T> =
		<<T as Config>::Currency as as Stp258Currency<<T as frame_system::Config>::AccountId>>::Balance;
	pub(crate) type CurrencyIdOf<T> =
		<<T as Config>::Currency as as Stp258Currency<<T as frame_system::Config>::AccountId>>::CurrencyId;
	
	/// The pallet's configuration trait.
	pub trait  Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The frequency of adjustments of the SettCurrency supply.
		type ElastAdjustmentFrequency: Get<<Self as system::Trait>::BlockNumber;

		/// The amount of SettCurrency that are meant to track the value. Example: A value of 1_000 when tracking
		/// Dollars means that the SettCurrencys will try to maintain a price of 1_000 SettCurrency for 1$.
		type BaseUnit: Get<u64>;
	}

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	pub enum Event<T: Config> {
		where
			Amount = AmountOf<T>,
			CurrencyId = CurrencyIdOf<T>
		{
			/// The supply was expanded by the amount. Sett-Mint - This 
			ExpandedSupply(CurrencyId, Amount),
			/// The supply was contracted by the amount. Dinar-Mint
			ContractedSupply(CurrencyId, Amount),
		}
	}

	// Errors inform users that something went wrong.
	// The possible errors returned by calls to this pallet's functions.
	#[pallet::error]
	pub enum Error<T> {
			/// While trying to expand the supply, it overflowed.
			SupplyOverflow,
			/// While trying to contract the supply, it underflowed.
			SupplyUnderflow,
			/// Something went very wrong and the price of the currency is zero.
			ZeroPrice,
			/// An arithmetic operation caused an overflow.
			GenericOverflow,
			/// An arithmetic operation caused an underflow.
			GenericUnderflow,
	}

	/// The frequency of adjustments for the Currency supply.
	pub struct ElastAdjustmentFrequency<BlockNumber> {
		/// Number of blocks for adjustment frequency.
		pub adjustment_frequency: BlockNumber,
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
		///   - `F` being the complexity of `Price::get_price()`
		///   - `P` being the complexity of `on_block_with_price`
		fn on_initialize(n: T::BlockNumber) {
			let price = T::Price::get_price();
			Self::on_block_with_price(n, price).unwrap_or_else(|e| {
				native::error!("could not adjust supply: {:?}", e);
			});
		}	
	}
}

impl<T: Config> SerpTes<T::AccountId> for Pallet<T> {
	type CurrencyId = CurrencyIdOf<T>;
	type Balance = BalanceOf<T>;

	// on block - adjustment frequency

	/// Contracts or expands the supply based on conditions.
	///
	/// **Weight:**
	/// Calls `serp_elast` (expand_or_contract_on_price) every `ElastAdjustmentFrequency` blocks.
	/// - complexity: `O(P)` with `P` being the complexity of `serp_elast`
	fn on_block_with_price(block: T::BlockNumber, price: Price) -> DispatchResult {
		// This can be changed to only correct for small or big price swings.
		if block % T::ElastAdjustmentFrequency::get() == 0.into() {
			Self::serp_elast(price)        
		} else {
			Ok(())
		}
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
	fn serp_elast(price: Price) -> DispatchResult {
		match price {
			0 => {
				native::error!("currency price is zero!");
				return Err(DispatchError::from(Error::<T>::ZeroPrice));
			}
			price if price > T::BaseUnit::get() => {
				// safe from underflow because `price` is checked to be greater than `BaseUnit`
				let supply = Pallet::<T>::total_issuance();
				let contract_by = Self::calculate_supply_change(price, T::BaseUnit::get(), supply);
				Self::contract_supply(supply, contract_by)?;
			}
			price if price < T::BaseUnit::get() => {
				// safe from underflow because `price` is checked to be less than `BaseUnit`
				let supply = Pallet::<T>::total_issuance();
				let expand_by = Self::calculate_supply_change(T::BaseUnit::get(), price, supply);
				Self::expand_supply(supply, expand_by)?;
			}
			_ => {
				native::info!("settcurrency price is equal to base as is desired --> nothing to do");
			}
		}
		Ok(())
	}

	/// Calculate the amount of supply change from a fraction given as `numerator` and `denominator`.
	fn calculate_supply_change(numerator: u64, denominator: u64, supply: u64) -> u64 {
		type Fix = FixedU128<U64>;
		let fraction = Fix::from_num(numerator) / Fix::from_num(denominator) - Fix::from_num(1);
		fraction.saturating_mul_int(supply as u128).to_num::<u64>()
	}                          
}

/// A `PriceProvider` implementation based on price data from a `DataProvider`.
pub struct SerpMarketPriceProvider<CurrencyId, Source>(PhantomData<(CurrencyId, Source)>);

impl<CurrencyId, Source, Price> MarketPriceProvider<CurrencyId, Price> for SerpMarketPriceProvider<CurrencyId, Source>
where
	CurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize,
	Source: DataProvider<CurrencyId, Price>,
	Price: CheckedDiv,
{
	fn get_price(base_currency_id: CurrencyId, quote_currency_id: CurrencyId) -> Option<Price> {
		let base_price = Source::get(&base_currency_id)?;
		let quote_price = Source::get(&quote_currency_id)?;

		base_price.checked_div(&quote_price)
	}

	/// Provide relative `serping_price` for two currencies
    /// with additional `serp_quote`.
	fn get_serpup_price(base_currency_id: CurrencyId, quote_currency_id: CurrencyId) -> Option<Price> {
		let base_price = Source::get(&base_currency_id)?; // base currency price compared to currency (native currency could work best)
		let quote_price = Source::get(&quote_currency_id)?;
        let market_price = base_price.checked_div(&quote_price); // market_price of the currency.
        let mint_rate = Perbill::from_percent(); // supply change of the currency.
        let serp_quote = market_price.checked_add(Perbill::from_percent(&mint_rate * 2)); // serping_price of the currency.
        serp_quote.checked_add(Perbill::from_percent(&mint_rate * 2)); 
	}

	/// Provide relative `serping_price` for two currencies
    /// with additional `serp_quote`.
	fn get_serpdown_price(base_currency_id: CurrencyId, quote_currency_id: CurrencyId) -> Option<Price> {
		let base_price = Source::get(&base_currency_id)?; // base currency price compared to currency (native currency could work best)
		let quote_price = Source::get(&quote_currency_id)?;
        let market_price = base_price.checked_div(&quote_price); // market_price of the currency.
        let mint_rate = Perbill::from_percent(); // supply change of the currency.
        let serp_quote = market_price.checked_add(Perbill::from_percent(&mint_rate * 2)); // serping_price of the currency.
        serp_quote.checked_add(Perbill::from_percent(&mint_rate * 2)); 
	}
}