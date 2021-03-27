#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use codec::Codec;
use frame_support::{
	debug::native,
	pallet_prelude::*,
	traits::{
		Currency as SetheumCurrency, ExistenceRequirement, Get, 
		LockableCurrency as SetheumLockableCurrency,
		ReservableCurrency as SetheumReservableCurrency, WithdrawReasons,
	},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use serp_traits::{
	account::MergeAccount,
	arithmetic::{Signed, SimpleArithmetic},
	BalanceStatus, SerpMarket, SerpTes, Stp258Asset, Stp258AssetExtended, Stp258AssetLockable, Stp258AssetReservable,
	LockIdentifier, Stp258Currency, Stp258CurrencyExtended, Stp258CurrencyReservable, Stp258CurrencyLockable,
};
use orml_utilities::with_transaction_result;
use sp_runtime::{
	traits::{CheckedSub, MaybeSerializeDeserialize, StaticLookup, Zero},
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
	pub(crate) type BlockOf<T> =
		<<T as Config>::Stp258Currency as SerpTes<<T as frame_system::Config>::AccountId>>::BlockNumber;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Stp258Currency: MergeAccount<Self::AccountId>
			+ Stp258CurrencyExtended<Self::AccountId>
			+ Stp258CurrencyLockable<Self::AccountId>
			+ Stp258CurrencyReservable<Self::AccountId>
			+ SerpMarket<Self::AccountId>
			+ SerpTes<Self::AccountId>;

		type Stp258Native: Stp258AssetExtended<Self::AccountId, Balance = BalanceOf<Self>, Amount = AmountOf<Self>>
			+ Stp258AssetLockable<Self::AccountId, Balance = BalanceOf<Self>>
			+ Stp258AssetReservable<Self::AccountId, Balance = BalanceOf<Self>>;

		#[pallet::constant]
		type GetStp258NativeId: Get<CurrencyIdOf<Self>>;

		/// Weight information for extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Unable to convert the Amount type into Balance.
		AmountIntoBalanceFailed,
		/// Balance is too low.
		BalanceTooLow,
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
		// Supply Expansion Successful. \[currency_id, expand_by\]
		SerpedUpSupply(CurrencyIdOf<T>, BalanceOf<T>),
		/// Supply Contraction Successful. \[currency_id, contract_by\]
		SerpedDownSupply(CurrencyIdOf<T>, BalanceOf<T>),
		// Serp Elast Successful. \[currency_id, expand_by\]
		SerpElast(CurrencyIdOf<T>),
		/// On Serp Block Successful. \[currency_id, contract_by\]
		OnSerpBlock(CurrencyIdOf<T>),
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Transfer some balance to another account under `currency_id`.
		///
		/// The dispatch origin for this call must be `Signed` by the
		/// transactor.
		#[pallet::weight(T::WeightInfo::transfer_non_native_currency())]
		pub fn transfer(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: CurrencyIdOf<T>,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;
			<Self as Stp258Currency<T::AccountId>>::transfer(currency_id, &from, &to, amount)?;
			Ok(().into())
		}

		/// Transfer some native currency to another account.
		///
		/// The dispatch origin for this call must be `Signed` by the
		/// transactor.
		#[pallet::weight(T::WeightInfo::transfer_native_currency())]
		pub fn transfer_native_currency(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;
			T::Stp258Native::transfer(&from, &to, amount)?;

			Self::deposit_event(Event::Transferred(T::GetStp258NativeId::get(), from, to, amount));
			Ok(().into())
		}

		/// update amount of account `who` under `currency_id`.
		///
		/// The dispatch origin of this call must be _Root_.
		#[pallet::weight(T::WeightInfo::update_balance_non_native_currency())]
		pub fn update_balance(
			origin: OriginFor<T>,
			who: <T::Lookup as StaticLookup>::Source,
			currency_id: CurrencyIdOf<T>,
			amount: AmountOf<T>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let dest = T::Lookup::lookup(who)?;
			<Self as Stp258CurrencyExtended<T::AccountId>>::update_balance(currency_id, &dest, amount)?;
			Ok(().into())
		}
	}
}

impl<T: Config> SerpTes<T::AccountId> for Pallet<T> {
	type BlockNumber = BlockOf<T>;
	/// Contracts or expands the currency supply based on conditions.
	/// Filters through the conditions to see whether it's time to adjust supply or not.
	fn on_serp_block(
		now: Self::BlockNumber,
		stable_currency_id: Self::CurrencyId,
		stable_currency_price: Self::Balance, 
		native_currency_id: Self::CurrencyId,
		native_currency_price: Self::Balance, 
	) -> DispatchResult {
        if native_currency_id == T::GetStp258NativeId::get() {
			if stable_currency_id != T::GetStp258NativeId::get() {
				T::Stp258Currency::on_serp_block(
					now,
                    stable_currency_id,
                    stable_currency_price,
					native_currency_id, 
					native_currency_price,
				)?;
			} else {
				native::info!("💸 This currency ({:?}) cannot be serped on block.", stable_currency_id);
			}
		} else {
			native::info!("💸 The native serping currency is not recognised.");
		}
		Self::deposit_event(Event::OnSerpBlock(stable_currency_id));
		Ok(())
	}

    /// Calculate the amount of supply change from a fraction.
	fn supply_change(currency_id:  Self::CurrencyId, new_price: Self::Balance) -> Self::Balance {
		let base_unit = T::Stp258Currency::base_unit(currency_id);
		let supply = <Self as Stp258Currency<T::AccountId>>::total_issuance(currency_id);
		let fraction = new_price * supply;
		let fractioned = fraction / base_unit;
		fractioned - supply
	}

    /// Expands (if the price is above pegbase) or contracts (if the price is below pegbase) 
	/// the supply of SettCurrencies.
	///
	/// **Weight:**
	/// - complexity: `O(S + C)`
	///   - `S` being the complexity of executing either `expand_supply` or `contract_supply`
	///   - `C` being a constant amount of storage reads for SettCurrency supply
	/// - DB access:
	///   - 1 read for total_issuance
	///   - execute `expand_supply` OR execute `contract_supply` which have DB accesses
	fn serp_elast(
		stable_currency_id: Self::CurrencyId, 
		stable_currency_price: Self::Balance, 
		native_currency_id: Self::CurrencyId,
		native_currency_price: Self::Balance,
	) -> DispatchResult {
        if native_currency_id == T::GetStp258NativeId::get() {
			if stable_currency_id != T::GetStp258NativeId::get() {
				T::Stp258Currency::serp_elast(
                    stable_currency_id,
                    stable_currency_price,
					native_currency_id, 
					native_currency_price,
				)?;
			} else {
				native::info!("💸 Cannot serp serp native currency ({:?}).", native_currency_id);
			}
		} else {
			native::info!("💸 The native serping currency is not recognised.");
		}
		Self::deposit_event(Event::SerpElast(stable_currency_id));
		Ok(())
	}
}
impl<T: Config> SerpMarket<T::AccountId> for Pallet<T> {
	/// Called when `expand_supply` is received from the SERP by the SerpTes 
	/// through the `on_expand_supply` trigger.
	/// Implementation should `deposit` the `amount` to `serpup_to`, 
	/// then `amount` will be slashed from `serpup_from` and update
	/// `new_supply`. `quote_price` is the price ( relative to the settcurrency) of 
	/// the `native_currency` used to expand settcurrency supply.
	/// `who` is the account to serp with.
	/// `quote_price` here is sampled from mock and can be connected to an oracle.
	fn expand_supply(
		native_currency_id: Self::CurrencyId, 
		stable_currency_id: Self::CurrencyId, 
		expand_by: Self::Balance, 
		quote_price: Self::Balance, 
	) -> DispatchResult {
		if expand_by.is_zero() {
			return Ok(());
		}
		if native_currency_id == T::GetStp258NativeId::get() {
			if stable_currency_id != T::GetStp258NativeId::get() {
				T::Stp258Currency::expand_supply(
					native_currency_id, 
					stable_currency_id, 
					expand_by as Self::Balance, 
					quote_price,
				)?;
			} else {
				native::info!("💸 This currency cannot be serped.");
			}
		} else {
			native::info!("💸 The native serping currency is not recognised.");
		}
		Self::deposit_event(Event::SerpedUpSupply(stable_currency_id, expand_by));
		Ok(())
	}

	/// Called when `contract_supply` is received from the SERP by the SerpTes 
	/// through the `on_contract_supply` trigger.
	/// Implementation should `deposit` the `base_currency_id` (The Native Currency) 
	/// of `amount` to `serpup_to`, then `amount` will be slashed from `serpup_from` 
	/// and update `new_supply`. `quote_price` is the price ( relative to the settcurrency) of 
	/// the `native_currency` used to contract settcurrency supply.
	/// `who` is the account to serp with.
	/// `quote_price` here is sampled from mock and can be connected to an oracle.
	fn contract_supply(
		native_currency_id: Self::CurrencyId, 
		stable_currency_id: Self::CurrencyId, 
		contract_by: Self::Balance, 
		quote_price: Self::Balance, 
	) -> DispatchResult {
		if contract_by.is_zero() {
			return Ok(());
		}
		if native_currency_id == T::GetStp258NativeId::get() {
			if stable_currency_id != T::GetStp258NativeId::get() {
				T::Stp258Currency::contract_supply(
					native_currency_id, 
					stable_currency_id, 
					contract_by,
					quote_price,
				)?;
			} else {
				native::info!("💸 This currency cannot be serped.");
			}
		} else {
			native::info!("💸 The native serping currency is not recognised.");
		}
		Self::deposit_event(Event::SerpedDownSupply(stable_currency_id, contract_by));
		Ok(())
	}
}

impl<T: Config> Stp258Currency<T::AccountId> for Pallet<T> {
	type CurrencyId = CurrencyIdOf<T>;
	type Balance = BalanceOf<T>;

	fn base_unit(currency_id: Self::CurrencyId) -> Self::Balance {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::minimum_balance()
		} else {
			T::Stp258Currency::base_unit(currency_id)
		}
	}

	fn minimum_balance(currency_id: Self::CurrencyId) -> Self::Balance {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::minimum_balance()
		} else {
			T::Stp258Currency::minimum_balance(currency_id)
		}
	}

	fn total_issuance(currency_id: Self::CurrencyId) -> Self::Balance {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::total_issuance()
		} else {
			T::Stp258Currency::total_issuance(currency_id)
		}
	}

	fn total_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::total_balance(who)
		} else {
			T::Stp258Currency::total_balance(currency_id, who)
		}
	}

	fn free_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::free_balance(who)
		} else {
			T::Stp258Currency::free_balance(currency_id, who)
		}
	}

	fn ensure_can_withdraw(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::ensure_can_withdraw(who, amount)
		} else {
			T::Stp258Currency::ensure_can_withdraw(currency_id, who, amount)
		}
	}

	fn transfer(
		currency_id: Self::CurrencyId,
		from: &T::AccountId,
		to: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if amount.is_zero() || from == to {
			return Ok(());
		}
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::transfer(from, to, amount)?;
		} else {
			T::Stp258Currency::transfer(currency_id, from, to, amount)?;
		}
		Self::deposit_event(Event::Transferred(currency_id, from.clone(), to.clone(), amount));
		Ok(())
	}

	fn deposit(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::deposit(who, amount)?;
		} else {
			T::Stp258Currency::deposit(currency_id, who, amount)?;
		}
		Self::deposit_event(Event::Deposited(currency_id, who.clone(), amount));
		Ok(())
	}

	fn withdraw(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::withdraw(who, amount)?;
		} else {
			T::Stp258Currency::withdraw(currency_id, who, amount)?;
		}
		Self::deposit_event(Event::Withdrawn(currency_id, who.clone(), amount));
		Ok(())
	}

	fn can_slash(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> bool {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::can_slash(who, amount)
		} else {
			T::Stp258Currency::can_slash(currency_id, who, amount)
		}
	}

	fn slash(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> Self::Balance {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::slash(who, amount)
		} else {
			T::Stp258Currency::slash(currency_id, who, amount)
		}
	}
}

impl<T: Config> Stp258CurrencyExtended<T::AccountId> for Pallet<T> {
	type Amount = AmountOf<T>;

	fn update_balance(currency_id: Self::CurrencyId, who: &T::AccountId, by_amount: Self::Amount) -> DispatchResult {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::update_balance(who, by_amount)?;
		} else {
			T::Stp258Currency::update_balance(currency_id, who, by_amount)?;
		}
		Self::deposit_event(Event::BalanceUpdated(currency_id, who.clone(), by_amount));
		Ok(())
	}
}

impl<T: Config> Stp258CurrencyLockable<T::AccountId> for Pallet<T> {
	type Moment = T::BlockNumber;

	fn set_lock(
		lock_id: LockIdentifier,
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::set_lock(lock_id, who, amount)
		} else {
			T::Stp258Currency::set_lock(lock_id, currency_id, who, amount)
		}
	}

	fn extend_lock(
		lock_id: LockIdentifier,
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::extend_lock(lock_id, who, amount)
		} else {
			T::Stp258Currency::extend_lock(lock_id, currency_id, who, amount)
		}
	}

	fn remove_lock(lock_id: LockIdentifier, currency_id: Self::CurrencyId, who: &T::AccountId) -> DispatchResult {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::remove_lock(lock_id, who)
		} else {
			T::Stp258Currency::remove_lock(lock_id, currency_id, who)
		}
	}
}

impl<T: Config> Stp258CurrencyReservable<T::AccountId> for Pallet<T> {
	fn can_reserve(currency_id: Self::CurrencyId, who: &T::AccountId, value: Self::Balance) -> bool {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::can_reserve(who, value)
		} else {
			T::Stp258Currency::can_reserve(currency_id, who, value)
		}
	}

	fn slash_reserved(currency_id: Self::CurrencyId, who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::slash_reserved(who, value)
		} else {
			T::Stp258Currency::slash_reserved(currency_id, who, value)
		}
	}

	fn reserved_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::reserved_balance(who)
		} else {
			T::Stp258Currency::reserved_balance(currency_id, who)
		}
	}

	fn reserve(currency_id: Self::CurrencyId, who: &T::AccountId, value: Self::Balance) -> DispatchResult {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::reserve(who, value)
		} else {
			T::Stp258Currency::reserve(currency_id, who, value)
		}
	}

	fn unreserve(currency_id: Self::CurrencyId, who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::unreserve(who, value)
		} else {
			T::Stp258Currency::unreserve(currency_id, who, value)
		}
	}

	fn repatriate_reserved(
		currency_id: Self::CurrencyId,
		slashed: &T::AccountId,
		beneficiary: &T::AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		if currency_id == T::GetStp258NativeId::get() {
			T::Stp258Native::repatriate_reserved(slashed, beneficiary, value, status)
		} else {
			T::Stp258Currency::repatriate_reserved(currency_id, slashed, beneficiary, value, status)
		}
	}
}

pub struct Currency<T, GetCurrencyId>(marker::PhantomData<T>, marker::PhantomData<GetCurrencyId>);

impl<T, GetCurrencyId> Stp258Asset<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyIdOf<T>>,
{
	type Balance = BalanceOf<T>;

	fn minimum_balance() -> Self::Balance {
		<Pallet<T>>::minimum_balance(GetCurrencyId::get())
	}

	fn total_issuance() -> Self::Balance {
		<Pallet<T>>::total_issuance(GetCurrencyId::get())
	}

	fn total_balance(who: &T::AccountId) -> Self::Balance {
		<Pallet<T>>::total_balance(GetCurrencyId::get(), who)
	}

	fn free_balance(who: &T::AccountId) -> Self::Balance {
		<Pallet<T>>::free_balance(GetCurrencyId::get(), who)
	}

	fn ensure_can_withdraw(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T>>::ensure_can_withdraw(GetCurrencyId::get(), who, amount)
	}

	fn transfer(from: &T::AccountId, to: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T> as Stp258Currency<T::AccountId>>::transfer(GetCurrencyId::get(), from, to, amount)
	}

	fn deposit(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T>>::deposit(GetCurrencyId::get(), who, amount)
	}

	fn withdraw(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T>>::withdraw(GetCurrencyId::get(), who, amount)
	}

	fn can_slash(who: &T::AccountId, amount: Self::Balance) -> bool {
		<Pallet<T>>::can_slash(GetCurrencyId::get(), who, amount)
	}

	fn slash(who: &T::AccountId, amount: Self::Balance) -> Self::Balance {
		<Pallet<T>>::slash(GetCurrencyId::get(), who, amount)
	}
}

impl<T, GetCurrencyId> Stp258AssetExtended<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyIdOf<T>>,
{
	type Amount = AmountOf<T>;

	fn update_balance(who: &T::AccountId, by_amount: Self::Amount) -> DispatchResult {
		<Pallet<T> as Stp258CurrencyExtended<T::AccountId>>::update_balance(GetCurrencyId::get(), who, by_amount)
	}
}

impl<T, GetCurrencyId> Stp258AssetLockable<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyIdOf<T>>,
{
	type Moment = T::BlockNumber;

	fn set_lock(lock_id: LockIdentifier, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T> as Stp258CurrencyLockable<T::AccountId>>::set_lock(lock_id, GetCurrencyId::get(), who, amount)
	}

	fn extend_lock(lock_id: LockIdentifier, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T> as Stp258CurrencyLockable<T::AccountId>>::extend_lock(lock_id, GetCurrencyId::get(), who, amount)
	}

	fn remove_lock(lock_id: LockIdentifier, who: &T::AccountId) -> DispatchResult {
		<Pallet<T> as Stp258CurrencyLockable<T::AccountId>>::remove_lock(lock_id, GetCurrencyId::get(), who)
	}
}

impl<T, GetCurrencyId> Stp258AssetReservable<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyIdOf<T>>,
{
	fn can_reserve(who: &T::AccountId, value: Self::Balance) -> bool {
		<Pallet<T> as Stp258CurrencyReservable<T::AccountId>>::can_reserve(GetCurrencyId::get(), who, value)
	}

	fn slash_reserved(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		<Pallet<T> as Stp258CurrencyReservable<T::AccountId>>::slash_reserved(GetCurrencyId::get(), who, value)
	}

	fn reserved_balance(who: &T::AccountId) -> Self::Balance {
		<Pallet<T> as Stp258CurrencyReservable<T::AccountId>>::reserved_balance(GetCurrencyId::get(), who)
	}

	fn reserve(who: &T::AccountId, value: Self::Balance) -> DispatchResult {
		<Pallet<T> as Stp258CurrencyReservable<T::AccountId>>::reserve(GetCurrencyId::get(), who, value)
	}

	fn unreserve(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		<Pallet<T> as Stp258CurrencyReservable<T::AccountId>>::unreserve(GetCurrencyId::get(), who, value)
	}

	fn repatriate_reserved(
		slashed: &T::AccountId,
		beneficiary: &T::AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		<Pallet<T> as Stp258CurrencyReservable<T::AccountId>>::repatriate_reserved(
			GetCurrencyId::get(),
			slashed,
			beneficiary,
			value,
			status,
		)
	}
}

pub type Stp258NativeOf<T> = Currency<T, <T as Config>::GetStp258NativeId>;

/// Adapt other currency traits implementation to `Stp258Asset`.
pub struct Stp258AssetAdapter<T, Currency, Amount, Moment>(marker::PhantomData<(T, Currency, Amount, Moment)>);

type PalletBalanceOf<A, Currency> = <Currency as SetheumCurrency<A>>::Balance;

// Adapt `frame_support::traits::Currency`
impl<T, AccountId, Currency, Amount, Moment> Stp258Asset<AccountId>
	for Stp258AssetAdapter<T, Currency, Amount, Moment>
where
	Currency: SetheumCurrency<AccountId>,
	T: Config,
{
	type Balance = PalletBalanceOf<AccountId, Currency>;

	fn minimum_balance() -> Self::Balance {
		Currency::minimum_balance()
	}

	fn total_issuance() -> Self::Balance {
		Currency::total_issuance()
	}

	fn total_balance(who: &AccountId) -> Self::Balance {
		Currency::total_balance(who)
	}

	fn free_balance(who: &AccountId) -> Self::Balance {
		Currency::free_balance(who)
	}

	fn ensure_can_withdraw(who: &AccountId, amount: Self::Balance) -> DispatchResult {
		let new_balance = Self::free_balance(who)
			.checked_sub(&amount)
			.ok_or(Error::<T>::BalanceTooLow)?;

		Currency::ensure_can_withdraw(who, amount, WithdrawReasons::all(), new_balance)
	}

	fn transfer(from: &AccountId, to: &AccountId, amount: Self::Balance) -> DispatchResult {
		Currency::transfer(from, to, amount, ExistenceRequirement::AllowDeath)
	}

	fn deposit(who: &AccountId, amount: Self::Balance) -> DispatchResult {
		let _ = Currency::deposit_creating(who, amount);
		Ok(())
	}

	fn withdraw(who: &AccountId, amount: Self::Balance) -> DispatchResult {
		Currency::withdraw(who, amount, WithdrawReasons::all(), ExistenceRequirement::AllowDeath).map(|_| ())
	}

	fn can_slash(who: &AccountId, amount: Self::Balance) -> bool {
		Currency::can_slash(who, amount)
	}

	fn slash(who: &AccountId, amount: Self::Balance) -> Self::Balance {
		let (_, gap) = Currency::slash(who, amount);
		gap
	}
}

// Adapt `frame_support::traits::Currency`
impl<T, AccountId, Currency, Amount, Moment> Stp258AssetExtended<AccountId>
	for Stp258AssetAdapter<T, Currency, Amount, Moment>
where
	Amount: Signed
		+ TryInto<PalletBalanceOf<AccountId, Currency>>
		+ TryFrom<PalletBalanceOf<AccountId, Currency>>
		+ SimpleArithmetic
		+ Codec
		+ Copy
		+ MaybeSerializeDeserialize
		+ Debug
		+ Default,
	Currency: SetheumCurrency<AccountId>,
	T: Config,
{
	type Amount = Amount;

	fn update_balance(who: &AccountId, by_amount: Self::Amount) -> DispatchResult {
		let by_balance = by_amount
			.abs()
			.try_into()
			.map_err(|_| Error::<T>::AmountIntoBalanceFailed)?;
		if by_amount.is_positive() {
			Self::deposit(who, by_balance)
		} else {
			Self::withdraw(who, by_balance)
		}
	}
}

// Adapt `frame_support::traits::LockableCurrency`
impl<T, AccountId, Currency, Amount, Moment> Stp258AssetLockable<AccountId>
	for Stp258AssetAdapter<T, Currency, Amount, Moment>
where
	Currency: SetheumLockableCurrency<AccountId>,
	T: Config,
{
	type Moment = Moment;

	fn set_lock(lock_id: LockIdentifier, who: &AccountId, amount: Self::Balance) -> DispatchResult {
		Currency::set_lock(lock_id, who, amount, WithdrawReasons::all());
		Ok(())
	}

	fn extend_lock(lock_id: LockIdentifier, who: &AccountId, amount: Self::Balance) -> DispatchResult {
		Currency::extend_lock(lock_id, who, amount, WithdrawReasons::all());
		Ok(())
	}

	fn remove_lock(lock_id: LockIdentifier, who: &AccountId) -> DispatchResult {
		Currency::remove_lock(lock_id, who);
		Ok(())
	}
}

// Adapt `frame_support::traits::ReservableCurrency`
impl<T, AccountId, Currency, Amount, Moment> Stp258AssetReservable<AccountId>
	for Stp258AssetAdapter<T, Currency, Amount, Moment>
where
	Currency: SetheumReservableCurrency<AccountId>,
	T: Config,
{
	fn can_reserve(who: &AccountId, value: Self::Balance) -> bool {
		Currency::can_reserve(who, value)
	}

	fn slash_reserved(who: &AccountId, value: Self::Balance) -> Self::Balance {
		let (_, gap) = Currency::slash_reserved(who, value);
		gap
	}

	fn reserved_balance(who: &AccountId) -> Self::Balance {
		Currency::reserved_balance(who)
	}

	fn reserve(who: &AccountId, value: Self::Balance) -> DispatchResult {
		Currency::reserve(who, value)
	}

	fn unreserve(who: &AccountId, value: Self::Balance) -> Self::Balance {
		Currency::unreserve(who, value)
	}

	fn repatriate_reserved(
		slashed: &AccountId,
		beneficiary: &AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		Currency::repatriate_reserved(slashed, beneficiary, value, status)
	}
}

impl<T: Config> MergeAccount<T::AccountId> for Pallet<T> {
	fn merge_account(source: &T::AccountId, dest: &T::AccountId) -> DispatchResult {
		with_transaction_result(|| {
			// transfer non-native free to dest
			T::Stp258Currency::merge_account(source, dest)?;

			// unreserve all reserved currency
			T::Stp258Native::unreserve(source, T::Stp258Native::reserved_balance(source));

			// transfer all free to dest
			T::Stp258Native::transfer(source, dest, T::Stp258Native::free_balance(source))
		})
	}
}
