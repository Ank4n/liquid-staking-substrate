#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::too_many_arguments)]

use frame_support::{sp_runtime::traits::StaticLookup, transactional, PalletId};
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// use sp_staking::{EraIndex, SessionIndex};

use codec::{Decode, Encode, MaxEncodedLen};
use orml_traits::MultiCurrency;
pub use pallet::*;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
	traits::{AccountIdConversion, Saturating, Zero},
	ArithmeticError, FixedPointNumber, FixedPointOperand, FixedU128, RuntimeDebug,
};

pub type BalanceOf<T> = <T as pallet_staking::Config>::CurrencyBalance;
pub type MintRate = FixedU128;

// FIXME: should be in a common lib
#[derive(
	Encode,
	Decode,
	Eq,
	PartialEq,
	Copy,
	Clone,
	RuntimeDebug,
	PartialOrd,
	Ord,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum CurrencyId {
	DOT,
	LDOT,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_staking::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// This is used to get the account ID for the liquid staking pot
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Multi-currency support
		type Currency: MultiCurrency<
			Self::AccountId,
			CurrencyId = CurrencyId,
			Balance = <Self as pallet_staking::Config>::CurrencyBalance,
		>;

		/// Staking Currency ID
		#[pallet::constant]
		type StakingCurrencyId: Get<CurrencyId>;

		/// Liquid Currency ID
		#[pallet::constant]
		type LiquidCurrencyId: Get<CurrencyId>;

		/// Default Mint rate = liquid currency / staking currency.
		#[pallet::constant]
		type DefaultMintRate: Get<MintRate>;

		/// Minimum bond amount of stake currency to mint liquidCurrency.
		#[pallet::constant]
		type BondThreshold: Get<BalanceOf<Self>>;

		/// Minimum liquid amount for unstaking staked currency.
		#[pallet::constant]
		type UnbondThreshold: Get<BalanceOf<Self>>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn something)]
	pub type Something<T> = StorageValue<_, u32>;

	/// The total amount of issued liquid currency.
	#[pallet::storage]
	#[pallet::getter(fn total_liquid_issuance)]
	pub type TotalLiquidIssuance<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		BondAndMint(BalanceOf<T>, T::AccountId),
		UnbondingAndBurn(BalanceOf<T>, T::AccountId),
		Withdraw(T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// The bond amount in staking currency is below threshold
		BelowBondThreshold,
		/// The unbond amount in Liquid currency is below threshold
		BelowUnbondThreshold,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		BalanceOf<T>: FixedPointOperand,
	{
		/// Amount of staking currency to bond and used
		/// to mint the liquid currency
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn bond_and_mint(
			origin: OriginFor<T>,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResult {
			let staker = ensure_signed(origin.clone())?;

			// Ensure the amount is above the Bond Threshold
			ensure!(amount >= T::BondThreshold::get(), Error::<T>::BelowBondThreshold);
			let pot_account = &Self::account_id();

			// transfer staking currency from staker to the pot
			<T as pallet::Config>::Currency::transfer(
				T::StakingCurrencyId::get(),
				&staker,
				&pot_account,
				amount,
			)?;

			let pot_origin = frame_system::RawOrigin::Signed(pot_account.clone()).into();
			pallet_staking::Pallet::<T>::bond(
				pot_origin,
				T::Lookup::unlookup(pot_account.clone()),
				amount,
				pallet_staking::RewardDestination::Controller,
			)?;

			let liquid_amount = Self::mint_liquid(amount)?;
			<T as pallet::Config>::Currency::deposit(
				T::LiquidCurrencyId::get(),
				&staker,
				liquid_amount,
			)?;
			TotalLiquidIssuance::<T>::mutate(|total| *total = total.saturating_add(liquid_amount));

			// Emit an event.
			Self::deposit_event(Event::BondAndMint(amount, staker));
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn unbond(
			origin: OriginFor<T>,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			pallet_staking::Pallet::<T>::unbond(origin, amount)?;

			// Emit an event.
			Self::deposit_event(Event::UnbondingAndBurn(amount, who));
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn withdraw_unbonded(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let _ = pallet_staking::Pallet::<T>::withdraw_unbonded(origin, 0);

			// Emit an event.
			Self::deposit_event(Event::Withdraw(who));
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}
	}

	impl<T: Config> Pallet<T>
	where
		BalanceOf<T>: FixedPointOperand, {
		/// Module account id
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		pub fn mint_liquid(staking_amount: BalanceOf<T>) -> Result<BalanceOf<T>, DispatchError> {
			Self::current_mint_rate()
				//FIXME how to multiply
				.checked_mul_int(staking_amount)
				.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))
		}

		/// Calculate mint rate
		/// total_liquid_amount / total_staking_amount
		/// If mint rate cannot be calculated, T::DefaultExchangeRate is used.
		pub fn current_mint_rate() -> MintRate {
			let total_staking = <T as pallet::Config>::Currency::total_balance(
				T::StakingCurrencyId::get(),
				&Self::account_id(),
			);
			let total_liquid = Self::total_liquid_issuance();
			if total_staking.is_zero() {
				T::DefaultMintRate::get()
			} else {
				MintRate::checked_from_rational(total_staking, total_liquid)
					.unwrap_or_else(T::DefaultMintRate::get)
			}
		}
	}
}
