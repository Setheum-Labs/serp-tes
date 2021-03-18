//! Unit tests for the serp-tes module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use sp_runtime::traits::BadOrigin;

use traits::SettCurrency;

#[test]
fn calculate_supply_change_should_work() {
	let price = TEST_BASE_UNIT + 100;
	let supply = u64::max_value();
	let contract_by = SettCurrency::calculate_supply_change(price, TEST_BASE_UNIT, supply);
	// the error should be low enough
	assert_ge!(contract_by, u64::max_value() / 10 - 1);
	assert_le!(contract_by, u64::max_value() / 10 + 1);
}

#[test]
fn serp_elast_contract_supply_should_work() {
	ExtBuilder::default()
		.five_hundred_thousand_for_sett_pay_n_serper()
		.build()
		.execute_with(|| {
			let base_unit = T::GetBaseUnit;
			let price = STP258_TOKEN_ID, 990;
			assert_eq!(SerpMarket::get_stable_price(STP258_TOKEN_ID, price) 990);
			assert_ok!(SerpTes::serp_elast(STP258_TOKEN_ID, price));
		});
}
