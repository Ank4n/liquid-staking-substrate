#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use sp_runtime::traits::BadOrigin;
use orml_traits::MultiReservableCurrency;

#[test]
fn total_issuance() {
	ExtBuilder::default()
		.topup_balances()
		.build()
		.execute_with(|| {
			assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 16400);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 2000);
		});
}

#[test]
fn bonding_works() {
	ExtBuilder::default()
		.topup_balances()
		.build()
		.execute_with(|| {
		
		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &101), 1000);
		assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &101), 0);
		
		assert_ok!(LiquidStaking::bond_and_mint(Origin::signed(101), 200));
		
		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &101), 800);
		// assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &101), 2000);
	});
}
