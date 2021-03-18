#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use codec::Codec;
use frame_support::{
	pallet_prelude::*,
	traits::{
		Currency as SetheumCurrency, ExistenceRequirement, Get, 
		LockableCurrency as SetheumLockableCurrency,
		ReservableCurrency as SetheumReservableCurrency, WithdrawReasons,
	},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use stp258_traits::{
	account::MergeAccount,
	arithmetic::{Signed, SimpleArithmetic},
	BalanceStatus, Stp258Asset, Stp258AssetExtended, Stp258AssetLockable, Stp258AssetReservable,
	LockIdentifier, Stp258Currency, Stp258CurrencyExtended, Stp258CurrencyReservable, Stp258CurrencyLockable,
};
use orml_utilities::with_transaction_result;
use sp_runtime::{
	traits::{CheckedSub,  MaybeSerializeDeserialize, StaticLookup, Zero},
	DispatchError, DispatchResult,
};
use sp_std::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
	marker, result,
};

mod default_weight;
mod mock;
mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub trait WeightInfo {
		fn transfer_non_native_currency() -> Weight;
		fn transfer_native_currency() -> Weight;
		fn update_balance_non_native_currency() -> Weight;
		fn update_balance_native_currency_creating() -> Weight;
		fn update_balance_native_currency_killing() -> Weight;
	}

	pub(crate) type BalanceOf<T> =
		<<T as Config>::Stp258Currency as Stp258Currency<<T as frame_system::Config>::AccountId>>::Balance;
	pub(crate) type CurrencyIdOf<T> =
		<<T as Config>::Stp258Currency as Stp258Currency<<T as frame_system::Config>::AccountId>>::CurrencyId;
	pub(crate) type AmountOf<T> =
		<<T as Config>::Stp258Currency as Stp258CurrencyExtended<<T as frame_system::Config>::AccountId>>::Amount;

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);
	
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Stp258Currency: MergeAccount<Self::AccountId>
			+ Stp258CurrencyExtended<Self::AccountId>
			+ Stp258CurrencyReservable<Self::AccountId>;

		type Stp258Native: Stp258AssetExtended<Self::AccountId, Balance = BalanceOf<Self>, Amount = AmountOf<Self>>
			+ Stp258AssetReservable<Self::AccountId, Balance = BalanceOf<Self>>;

		
		#[pallet::constant]
		type GetStp258NativeId: Get<CurrencyIdOf<Self>>;


		/// The balance of an account.
		#[pallet::constant]
		type GetBaseUnit: Get<BalanceOf<Self>>;

		/// The single unit to avoid data loss with mized type arithmetic.
		#[pallet::constant]
		type GetSingleUnit: Get<BalanceOf<Self>>;

		/// The Serper ratio type getter
		#[pallet::constant]
		type GetSerperRatio: Get<BalanceOf<Self>>;

		/// The SettPay ratio type getter
		#[pallet::constant]
		type GetSettPayRatio: Get<BalanceOf<Self>>;	

		/// The SettPay Account type
		#[pallet::constant]
		type GetSettPayAcc: Get<Self::AccountId>;

		/// The Serpers Account type
		#[pallet::constant]
		type GetSerperAcc: Get<Self::AccountId>;

		/// The Serp quote multiple type for qUOTE, quoting 
		/// `(mintrate * SERP_QUOTE_MULTIPLE) = SerpQuotedPrice`.
		#[pallet::constant]
		type GetSerpQuoteMultiple: Get<BalanceOf<Self>>;

		/// Weight information for extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Unable to convert the Amount type into Balance.
		AmountIntoBalanceFailed,
		/// Balance is too low.
		BalanceTooLow,
		// Cannott expand or contract Native Asset, only SettCurrency	Serping.
		CannotSerpNativeAssetOnlySerpSettCurrency,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Currency transfer success. [currency_id, from, to, amount]
		Transferred(CurrencyIdOf<T>, T::AccountId, T::AccountId, BalanceOf<T>),
		/// Update balance success. [currency_id, who, amount]
		BalanceUpdated(CurrencyIdOf<T>, T::AccountId, AmountOf<T>),
		/// Deposit success. [currency_id, who, amount]
		Deposited(CurrencyIdOf<T>, T::AccountId, BalanceOf<T>),
		/// Withdraw success. [currency_id, who, amount]
		Withdrawn(CurrencyIdOf<T>, T::AccountId, BalanceOf<T>),
		/// Serp Expand Supply successful. [currency_id, who, amount]
		SerpedUpSupply(CurrencyIdOf<T>, BalanceOf<T>),
		/// Serp Contract Supply successful. [currency_id, who, amount]
		SerpedDownSupply(CurrencyIdOf<T>, BalanceOf<T>),
		/// The New Price of Currency. [currency_id, price]
		NewPrice(CurrencyIdOf<T>, BalanceOf<T>),
	}


	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		/// Contracts or expands the supply based on conditions.
		///
		/// **Weight:**
		/// Calls `serp_elast` (expand_or_contract_on_price) every `ElastAdjustmentFrequency` blocks.
		/// - complexity: `O(P)` with `P` being the complexity of `serp_elast`
		#[weight = 0]
		fn on_block_with_price(block: T::BlockNumber, price: Balance) -> DispatchResult {
			// This can be changed to only correct for small or big price swings.
			if block % T::ElastAdjustmentFrequency::get() == 0.into() {
				Self::serp_elast(price)        
			} else {
				Ok(())
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		
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
		fn serp_elast(currency_id: CurrencyId, price: Balance) -> DispatchResult {
			let base_unit = T::GetBaseUnit;
			let price = T::SerpMarket::Price::get_stable_price(currency_id, quote_price: Balance);
			match price {
				0 => {
					native::error!("currency price is zero!");
					return Err(DispatchError::from(Error::<T>::ZeroPrice));
				}
				price if price > base_unit => {
					// safe from underflow because `price` is checked to be less than `GetBaseUnit`
					let expand_by = Self::calculate_currency_supply_change(currency_id, price);
					Self::expand_supply(origin: OriginFor<T>, currency_id, expand_by, price)?;
				}
				price if price < base_unit => {
					// safe from underflow because `price` is checked to be greater than `GetBaseUnit`
					let contract_by = Self::calculate_currency_supply_change(currency_id, price);
					Self::contract_supply(origin: OriginFor<T>, currency_id, contract_by, price)?;
				}
				_ => {
					native::info!("settcurrency price is equal to base as is desired --> nothing to do");
				}
			}
			Ok(())
		}
	}
}

