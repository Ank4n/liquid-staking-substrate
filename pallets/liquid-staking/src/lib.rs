#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::too_many_arguments)]

use frame_support::{sp_runtime::traits::StaticLookup, transactional};
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// use sp_staking::{EraIndex, SessionIndex};

pub use pallet::*;
use codec::{Decode, Encode, MaxEncodedLen};
use orml_traits::MultiCurrency;
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};


pub type BalanceOf<T> = <T as pallet_staking::Config>::CurrencyBalance;

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
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn something)]
	pub type Something<T> = StorageValue<_, u32>;

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
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Amount of staking currency to bond and used
		/// to mint the liquid currency
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn bond_and_mint(
			origin: OriginFor<T>,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let controller_lookup: <T::Lookup as StaticLookup>::Source =
				T::Lookup::unlookup(who.clone());

			pallet_staking::Pallet::<T>::bond(
				origin,
				controller_lookup, // reward, stash and controller are same for simplicity
				amount,
				pallet_staking::RewardDestination::Controller,
			)?;
			// Update storage.
			<Something<T>>::put(5u32);

			// Emit an event.
			Self::deposit_event(Event::BondAndMint(amount, who));
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
}
