//! Unit tests for the serp-tes module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use sp_runtime::traits::BadOrigin;

use traits::SettCurrency;


#[test]
fn serp_elast_quickcheck() {
	
}

#[test]
fn serp_elast_smoketest_should_work() {
	new_test_ext().execute_with(|| {
		
	})
}

#[test]
fn supply_change_calculation_should_work() {
	let price = TEST_BASE_UNIT + 100;
	let supply = u64::max_value();
	let contract_by = SettCurrency::calculate_supply_change(price, TEST_BASE_UNIT, supply);
	// the error should be low enough
	assert_ge!(contract_by, u64::max_value() / 10 - 1);
	assert_le!(contract_by, u64::max_value() / 10 + 1);
}

#[test]
fn serp_market_expand_supply_should_work() {
	new_test_ext_with(vec![1]).execute_with(|| {
		
	});
}

#[test]
fn serp_market_contract_supply_should_work() {
	new_test_ext_with(vec![1]).execute_with(|| {
		
	});
}

#[test]
fn get_price_should_work() {
	assert_eq!(
		TesPriceProvider::get_price(1, 2),
		Some(Price::saturating_from_rational(1, 2))
	);
	assert_eq!(
		TesPriceProvider::get_price(2, 1),
		Some(Price::saturating_from_rational(2, 1))
	);
}

#[test]
fn price_is_none_should_not_panic() {
	assert_eq!(TesPriceProvider::get_price(3, 3), None);
	assert_eq!(TesPriceProvider::get_price(3, 1), None);
	assert_eq!(TesPriceProvider::get_price(1, 3), None);
}

#[test]
fn price_is_zero_should_not_panic() {
	assert_eq!(TesPriceProvider::get_price(0, 0), None);
	assert_eq!(TesPriceProvider::get_price(1, 0), None);
	assert_eq!(TesPriceProvider::get_price(0, 1), Some(Price::from_inner(0)));
}