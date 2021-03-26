
impl<T: Config> SerpTes<T::AccountId> for Pallet<T> {
	fn adjustment_frequency() -> Result<(), &'static str> {
		T::AdjustmentFrequency::get()
	}

	fn on_serp_initialize(now: T::BlockNumber, sett_price: u64, sett_currency_id: T::CurrencyId; jusd_price: u64; jusd_currency_id: T::CurrencyId) -> DispatchResult {

		let sett_price_on_block = Self::on_block_with_price(now, sett_price, sett_currency_id).unwrap_or_else(|e| {
			native::error!("could not adjust supply: {:?}", e);
		});
		let jusd_price_on_block = Self::on_block_with_price(now, jusd_price, jusd_currency_id).unwrap_or_else(|e| {
			native::error!("could not adjust supply: {:?}", e);
		});

		Self::on_block_with_price(now, price).unwrap_or_else(|e| {
			native::error!("could not adjust supply: {:?}", e);
		});
	}

	/// Calculate the amount of supply change from a fraction.
	fn supply_change(currency_id:  Self::CurrencyId, new_price: Self::Balance) -> Self::Balance {
		let base_unit = T::GetBaseUnit::get(&currency_id);
		let supply = <Self as Stp258Currency<T::AccountId>>::total_issuance(currency_id);
		let fraction = new_price * supply;
		let fractioned = fraction / base_unit;
		fractioned - supply;
	}

	/// Contracts or expands the currency supply based on conditions.
	fn on_block_with_price(block: &T::Blocknumber, price: Self::Balance, currency_id: Self::CurrencyId) -> DispatchResult {
		// This can be changed to only correct for small or big price swings.
		let serp_elast_adjuster = T::AdjustmentFrequency::get();
		if block % serp_elast_adjuster == 0.into() {
			Self::serp_elast(currency_id, price)
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
	fn serp_elast(currency_id: CurrencyId, price: Balance) -> DispatchResult {
		let base_unit = T::GetBaseUnit;
		match price {
			0 => {
				native::error!("currency price is zero!");
				return Err(DispatchError::from(Error::<T>::ZeroPrice));
			}
			price if price > base_unit => {
				// safe from underflow because `price` is checked to be less than `GetBaseUnit`
				let expand_by = Self::supply_change(currency_id, price);
				<Self as Stp258Currency<_>>expand_supply(currency_id, expand_by, price)?;
			}
			price if price < base_unit => {
				// safe from underflow because `price` is checked to be greater than `GetBaseUnit`
				let contract_by = Self::supply_change(currency_id, price);
				<Self as Stp258Currency<_>>contract_supply(currency_id, expand_by, price)?;
			}
			_ => {
				native::info!("settcurrency price is equal to base as is desired --> nothing to do");
			}
		}
		Ok(())
	}
}
