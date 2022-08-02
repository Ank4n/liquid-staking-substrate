#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use orml_traits::MultiReservableCurrency;
use sp_runtime::traits::BadOrigin;
use pallet_staking::RewardDestination;
use sp_runtime::FixedU128;

#[test]
fn total_issuance() {
	ExtBuilder::default().topup_balances().build().execute_with(|| {
		assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 16400);
		assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 2000);
	});
}

#[test]
fn bonding_works() {
	ExtBuilder::default().topup_balances().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &101), 1000);
		assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &101), 0);

		assert_ok!(LiquidStaking::bond_and_mint(Origin::signed(101), 200));

		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &101), 800);
		let treasury = LiquidStaking::account_id();
		let total_liquid_issuance = LiquidStaking::total_liquid_issuance();

		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &treasury), 200);

		assert_eq!(total_liquid_issuance, 2000);

		assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &101), 2000);
	});
}

#[test]
fn staking_works() {
	ExtBuilder::default().topup_balances().build().execute_with(|| {
			
	});
}
