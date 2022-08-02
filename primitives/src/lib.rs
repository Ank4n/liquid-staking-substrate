#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use sp_runtime::{FixedU128};

pub type MintRate = FixedU128;

pub type CurrencyId = u32;
pub const DOT: CurrencyId = 1;
pub const LDOT: CurrencyId = 2;