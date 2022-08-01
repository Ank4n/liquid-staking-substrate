#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{AccountIdConversion, Saturating, Zero, MaybeSerializeDeserialize},
	ArithmeticError, FixedPointNumber, FixedPointOperand, FixedU128, RuntimeDebug,
};

pub type MintRate = FixedU128;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	DOT,
	LDOT,
}
