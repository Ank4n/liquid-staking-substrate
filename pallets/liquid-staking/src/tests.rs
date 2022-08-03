#![cfg(test)]

use super::*;
use frame_support::{assert_err, assert_ok};
use mock::*;
use orml_traits::MultiReservableCurrency;
use sq_primitives::{LIQUID_CURRENCY_ID, STAKING_CURRENCY_ID};
use substrate_test_utils::assert_eq_uvec;

#[test]
fn total_issuance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 18400);
		assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 2000);
	});
}

#[test]
fn bonding_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &101), 1000);
		assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &101), 0);

		assert_ok!(LiquidStaking::bond_and_mint(Origin::signed(101), 200));

		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &101), 800);
		let pot_account = LiquidStaking::account_id();
		let total_liquid_issuance = LiquidStaking::total_liquid_issuance();

		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &pot_account), 200);

		assert_eq!(total_liquid_issuance, 2000);

		assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &101), 2000);
	});
}

#[test]
fn staking_to_liquid_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(LiquidStaking::staking_to_liquid(10).unwrap(), 100);
	});
}

#[test]
fn liquid_to_staking_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(LiquidStaking::liquid_to_staking(1000).unwrap(), 100);
	});
}

#[test]
fn voting_works() {
	ExtBuilder::default().build().execute_with(|| {
		let pot_account = LiquidStaking::account_id();
		assert_eq!(LiquidStaking::current_era().unwrap(), 0);
		assert_eq_uvec!(validator_controllers(), vec![20, 10]);
		start_active_era(1);

		// stakers minting
		assert_ok!(LiquidStaking::bond_and_mint(Origin::signed(101), 200));
		assert_ok!(LiquidStaking::bond_and_mint(Origin::signed(102), 200));
		// stakers voting for their favourite nomination
		// highest votes
		assert_ok!(LiquidStaking::vote(Origin::signed(101), 30, 300));
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(LIQUID_CURRENCY_ID, &101),
			300
		);
		// second highest
		assert_ok!(LiquidStaking::vote(Origin::signed(102), 20, 200));
		// bottom
		assert_ok!(LiquidStaking::vote(Origin::signed(102), 10, 150));

		// verify vote counts
		assert_eq!(LiquidStaking::liquid_vote_count(30), 300);
		assert_eq!(LiquidStaking::liquid_vote_count(20), 200);
		assert_eq!(LiquidStaking::liquid_vote_count(10), 150);
		let expected_set = vec![30, 20];
		assert_ok!(LiquidStaking::apply_votes(Origin::root()));
		assert_eq!(expected_set, Staking::nominators(&pot_account).unwrap().targets.into_inner());

		// vote apply should clear votes
		// verify vote counts
		assert_eq!(LiquidStaking::liquid_vote_count(30), 0);
		assert_eq!(LiquidStaking::liquid_vote_count(20), 0);
		assert_eq!(LiquidStaking::liquid_vote_count(10), 0);
		// vote apply should also have unreseved the balances
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(LIQUID_CURRENCY_ID, &101),
			0
		);

		// Lets try to vote again
		start_active_era(5);
		// highest votes
		assert_ok!(LiquidStaking::vote(Origin::signed(101), 10, 200));
		// second highest
		assert_ok!(LiquidStaking::vote(Origin::signed(102), 20, 180));
		// bottom
		assert_ok!(LiquidStaking::vote(Origin::signed(101), 30, 150));

		assert_eq!(LiquidStaking::liquid_vote_count(10), 200);
		assert_eq!(LiquidStaking::liquid_vote_count(20), 180);
		assert_eq!(LiquidStaking::liquid_vote_count(30), 150);

		let expected_set = vec![10, 20];
		assert_ok!(LiquidStaking::apply_votes(Origin::root()));
		assert_eq!(expected_set, Staking::nominators(&pot_account).unwrap().targets.into_inner());
	});
}

#[test]
fn request_unbond_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq_uvec!(validator_controllers(), vec![20, 10]);

		assert_ok!(LiquidStaking::bond_and_mint(Origin::signed(101), 200));
		assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &101), 2000);
		assert_eq!(LiquidStaking::liquid_to_staking(100).unwrap(), 10);

		assert_ok!(LiquidStaking::request_unbond(Origin::signed(101), 100));
		let unbond_req = LiquidStaking::unbonding_requests(&101);
		assert_eq!(unbond_req.is_some(), true);
		assert_eq!(unbond_req.unwrap(), (10, 100, 0));
	});
}

#[test]
fn mint_rate_is_consistent() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(LiquidStaking::current_mint_rate(), MintRate::saturating_from_rational(10, 1));

		assert_ok!(LiquidStaking::bond_and_mint(Origin::signed(101), 200));
		assert_eq!(LiquidStaking::current_mint_rate(), MintRate::saturating_from_rational(10, 1));

		assert_ok!(LiquidStaking::request_unbond(Origin::signed(101), 100));
		assert_eq!(LiquidStaking::current_mint_rate(), MintRate::saturating_from_rational(10, 1));
	});
}

#[test]
fn request_unbond_before_unbond_duration_not_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(LiquidStaking::bond_and_mint(Origin::signed(101), 200));

		assert_ok!(LiquidStaking::request_unbond(Origin::signed(101), 100));

		assert_err!(
			LiquidStaking::withdraw_unbonded(Origin::signed(101)),
			Error::<Test>::UnbondingWaitNotComplete
		);
	});
}

#[test]
fn request_unbond_after_unbond_duration_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(LiquidStaking::total_liquid_issuance(), 0);

		start_active_era(2);
		assert_ok!(LiquidStaking::bond_and_mint(Origin::signed(101), 200));
		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &101), 800);
		assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &101), 2000);
		// liquid tokens are minted
		assert_eq!(LiquidStaking::total_liquid_issuance(), 2000);

		start_active_era(3);
		// liquid currency used to get back staking currency
		let burn_amount = 100;
		assert_ok!(LiquidStaking::request_unbond(Origin::signed(101), burn_amount));

		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &101), 800);
		assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &101), 1900);

		// unbond request at era 3, should unlock at era 6
		let unbond_req = LiquidStaking::unbonding_requests(&101);
		assert_eq!(unbond_req.unwrap(), (10, 100, 3));
		assert_err!(
			LiquidStaking::withdraw_unbonded(Origin::signed(101)),
			Error::<Test>::UnbondingWaitNotComplete
		);

		start_active_era(5);
		// locked at era 5
		assert_err!(
			LiquidStaking::withdraw_unbonded(Origin::signed(101)),
			Error::<Test>::UnbondingWaitNotComplete
		);

		start_active_era(6);
		// user free to withdraw unbond at era 6
		assert_ok!(LiquidStaking::withdraw_unbonded(Origin::signed(101)));
		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &101), 810);
		assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &101), 1900);
		// liquid token is burnt
		assert_eq!(LiquidStaking::total_liquid_issuance(), 2000 - burn_amount);
	});
}

#[test]
fn democracy_voting_with_liquid_currency_works() {
	ExtBuilder::default().build().execute_with(|| {
		let r = begin_referendum();
		assert_ok!(Democracy::vote_v2(
			Origin::signed(1),
			r,
			aye(1, LIQUID_CURRENCY_ID),
			LIQUID_CURRENCY_ID
		));
	});
}

#[test]
fn democracy_voting_with_staking_currency_works() {
	ExtBuilder::default().build().execute_with(|| {
		let r = begin_referendum();
		assert_ok!(Democracy::vote_v2(
			Origin::signed(1),
			r,
			aye(1, STAKING_CURRENCY_ID),
			STAKING_CURRENCY_ID
		));
	});
}
