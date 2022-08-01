use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};

#[test]
fn it_works_for_default_value() {
	// ExtBuilder::default()
	// 	.one_hundred_for_alice_n_bob()
	// 	.build()
	// 	.execute_with(|| {
	// 		// assert_ok!(true);
	// 	});
}

#[test]
fn correct_error_for_none_value() {
	new_test_ext().execute_with(|| {
		// Ensure the expected error is thrown when no value is present.
		// assert_noop!(LiquidStakingModule::cause_error(Origin::signed(1)), Error::<Test>::NoneValue);
	});
}
