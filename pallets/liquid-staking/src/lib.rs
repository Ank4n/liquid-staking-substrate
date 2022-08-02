#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::too_many_arguments)]

use frame_support::{sp_runtime::traits::StaticLookup, transactional, PalletId};
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod tests;

use sp_staking::{EraIndex, SessionIndex};

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

pub use primitives::MintRate;
type CurrencyId = u32;

pub type BalanceOf<T> = <T as pallet_staking::Config>::CurrencyBalance;

// Waiting period before tokens are unlocked
pub type UnbondWait<T> = <T as pallet_staking::Config>::BondingDuration;

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

	/// The total amount of issued liquid currency.
	#[pallet::storage]
	#[pallet::getter(fn total_liquid_issuance)]
	pub type TotalLiquidIssuance<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	/// Unbonding requests: AccountId => (Staking amount, Liquid amount, Era Index)
	#[pallet::storage]
	#[pallet::getter(fn unbonding_requests)]
	pub type UnbondingRequests<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		(BalanceOf<T>, BalanceOf<T>, EraIndex),
		OptionQuery,
	>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		BondAndMint(BalanceOf<T>, T::AccountId),
		RequestUnbond(BalanceOf<T>, T::AccountId),
		Withdraw(T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// The bond amount in staking currency is below threshold
		BelowBondThreshold,
		/// The unbond amount in Liquid currency is below threshold
		BelowUnbondThreshold,
		/// User already has a redeem request that has not been claimed yet
		UnclaimedRedeemRequestAlreadyExist,
		/// Era not set by the session
		CurrentEraNotSet,
		/// Unbonding request not found for the claim
		UnbondingRequestNotExist,
		/// Unbonding period has not elapsed
		UnbondingWaitNotComplete,
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

			let liquid_amount = Self::staking_to_liquid(amount)?;

			<T as pallet::Config>::Currency::deposit(
				T::LiquidCurrencyId::get(),
				&staker,
				liquid_amount,
			)?;

			TotalLiquidIssuance::<T>::mutate(|total| *total = total.saturating_add(liquid_amount));

			let pot_origin = frame_system::RawOrigin::Signed(pot_account.clone()).into();

			// FIXME Bond some in genesis config 
			// so never have to bond again and not check ledger
			let ledger = pallet_staking::Pallet::<T>::ledger(&pot_account);
			if ledger.is_some() {
				pallet_staking::Pallet::<T>::bond_extra(pot_origin, amount)?;
			} else {
				pallet_staking::Pallet::<T>::bond(
					pot_origin,
					T::Lookup::unlookup(pot_account.clone()),
					amount,
					pallet_staking::RewardDestination::Controller,
				)?;
			}

			// Emit an event.
			Self::deposit_event(Event::BondAndMint(amount, staker));
			// Return a successful result
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn vote(
			origin: OriginFor<T>,
			targets: Vec<<T::Lookup as StaticLookup>::Source>,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let voter = ensure_signed(origin.clone())?;
			let pot_account = &Self::account_id();

			// FIXME just reserve it, not transfer
			<T as pallet::Config>::Currency::transfer(
				T::LiquidCurrencyId::get(),
				&voter,
				&pot_account,
				amount,
			)?;

			let pot_origin = frame_system::RawOrigin::Signed(pot_account.clone()).into();

			pallet_staking::Pallet::<T>::nominate(pot_origin, targets)?;

			// Emit an event.
			// Self::deposit_event(Event::BondAndMint(amount, staker));
			// Return a successful result
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn request_unbond(
			origin: OriginFor<T>,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let already_requested = UnbondingRequests::<T>::contains_key(&who);
			ensure!(!already_requested, Error::<T>::UnclaimedRedeemRequestAlreadyExist);
			let current_era = pallet_staking::Pallet::<T>::current_era();
			ensure!(current_era.is_some(), Error::<T>::CurrentEraNotSet);

			let _ = <T as pallet::Config>::Currency::transfer(
				T::LiquidCurrencyId::get(),
				&who,
				&Self::account_id(),
				amount,
			);

			// no rewards/slash are counted once unbonding is requested
			let staking_amount = Self::liquid_to_staking(amount)?;
			// can unwrap as we checked previously current era exists
			UnbondingRequests::<T>::insert(&who, (staking_amount, amount, current_era.unwrap()));
			// unbond funds from pot account
			pallet_staking::Pallet::<T>::unbond(origin, amount)?;

			// Emit an event.
			Self::deposit_event(Event::RequestUnbond(amount, who));
			// Return a successful result
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn withdraw_unbonded(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			// Get the unbonding request
			let unbonding_request = UnbondingRequests::<T>::get(&who);

			ensure!(unbonding_request.is_some(), Error::<T>::UnbondingRequestNotExist);
			let (stake_amount, liquid_amount, old_era) = unbonding_request.unwrap();

			let current_era = Self::current_era();
			ensure!(current_era.is_some(), Error::<T>::CurrentEraNotSet);

			let unbond_wait = UnbondWait::<T>::get();

			ensure!(
				old_era + unbond_wait < current_era.unwrap(),
				Error::<T>::UnbondingWaitNotComplete
			);

			let pot_account = Self::account_id();
			let pot_free_balance = <T as pallet::Config>::Currency::free_balance(
				T::StakingCurrencyId::get(),
				&pot_account,
			);

			if pot_free_balance < stake_amount {
				let _ = pallet_staking::Pallet::<T>::withdraw_unbonded(origin, 0);
			}

			// burn liquid amount
			<T as pallet::Config>::Currency::withdraw(
				T::LiquidCurrencyId::get(),
				&pot_account,
				liquid_amount,
			)?;

			// transfer redeemed_staking to redeemer.
			<T as pallet::Config>::Currency::transfer(
				T::StakingCurrencyId::get(),
				&pot_account,
				&who,
				stake_amount,
			)?;

			TotalLiquidIssuance::<T>::mutate(|total| *total = total.saturating_sub(liquid_amount));

			// Emit an event.
			Self::deposit_event(Event::Withdraw(who));
			// Return a successful result
			Ok(())
		}
	}

	impl<T: Config> Pallet<T>
	where
		BalanceOf<T>: FixedPointOperand,
	{
		/// Module account id
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		pub fn staking_to_liquid(
			staking_amount: BalanceOf<T>,
		) -> Result<BalanceOf<T>, DispatchError> {
			Self::current_mint_rate()
				.checked_mul_int(staking_amount)
				.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))
		}

		pub fn liquid_to_staking(
			liquid_amount: BalanceOf<T>,
		) -> Result<BalanceOf<T>, DispatchError> {
			Self::current_mint_rate()
				.reciprocal()
				.expect("shouldn't be invalid!")
				.checked_mul_int(liquid_amount)
				.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))
		}

		/// Calculate mint rate
		/// total_liquid_amount / total_staking_amount
		/// If mint rate cannot be calculated, T::DefaultMintRate is used.
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

		pub fn current_era() -> Option<EraIndex> {
			pallet_staking::Pallet::<T>::current_era()
		}
	}
}
