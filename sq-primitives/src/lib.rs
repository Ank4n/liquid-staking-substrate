#![cfg_attr(not(feature = "std"), no_std)]
use sp_runtime::{FixedU128};

pub type MintRate = FixedU128;

pub type CurrencyId = u32;

// Native
pub const STAKING_CURRENCY_ID: CurrencyId = 1;

pub const LIQUID_CURRENCY_ID: CurrencyId = 2;