#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::too_many_arguments)]

use frame_support::{sp_runtime::traits::StaticLookup, transactional, BoundedVec, PalletId};
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod tests;

use frame_support::storage::IterableStorageMap;
use orml_traits::{currency::MultiReservableCurrency, MultiCurrency};
pub use pallet::*;

use sp_staking::EraIndex;
use sp_std::{vec, vec::Vec};
pub use sq_primitives::{CurrencyId, MintRate};
pub type BalanceOf<T> = <T as pallet_staking::Config>::CurrencyBalance;

// Waiting period before tokens are unlocked
pub type UnbondWait<T> = <T as pallet_staking::Config>::BondingDuration;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::{ensure_root, pallet_prelude::*};
	use sp_runtime::{
		traits::{AccountIdConversion, Saturating, Zero},
		FixedPointNumber, FixedPointOperand, ArithmeticError,
	};

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
			> + MultiReservableCurrency<Self::AccountId, CurrencyId = CurrencyId>;

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

		/// Max validator count
		#[pallet::constant]
		type MaxValidatorCount: Get<u32>;
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

	/// Validator simple vote count in liquid currency amount
	/// k-plurality to select winner
	#[pallet::storage]
	#[pallet::getter(fn liquid_vote_count)]
	pub type LiquidVoteCount<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

	/// Voter list by their votes
	#[pallet::storage]
	#[pallet::getter(fn voters)]
	pub type Voters<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		BondAndMint(BalanceOf<T>, T::AccountId),
		RequestUnbond(BalanceOf<T>, T::AccountId),
		Withdraw(T::AccountId),
		Voted(T::AccountId, T::AccountId, BalanceOf<T>),
		NominationsApplied(T::AccountId, T::AccountId),
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
			#[pallet::compact] staking_amount: BalanceOf<T>,
		) -> DispatchResult {
			let staker = ensure_signed(origin.clone())?;

			// Ensure the amount is above the Bond Threshold
			ensure!(staking_amount >= T::BondThreshold::get(), Error::<T>::BelowBondThreshold);
			let pot_account = &Self::account_id();

			// transfer staking currency from staker to the pot
			<T as pallet::Config>::Currency::transfer(
				T::StakingCurrencyId::get(),
				&staker,
				&pot_account,
				staking_amount,
			)?;

			let liquid_amount = Self::staking_to_liquid(staking_amount)?;

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
				pallet_staking::Pallet::<T>::bond_extra(pot_origin, staking_amount)?;
			} else {
				pallet_staking::Pallet::<T>::bond(
					pot_origin,
					T::Lookup::unlookup(pot_account.clone()),
					staking_amount,
					pallet_staking::RewardDestination::Controller,
				)?;
			}

			// Emit an event.
			Self::deposit_event(Event::BondAndMint(staking_amount, staker));
			// Return a successful result
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn vote(
			origin: OriginFor<T>,
			target: T::AccountId,
			#[pallet::compact] liquid_amount: BalanceOf<T>,
		) -> DispatchResult {
			let voter = ensure_signed(origin.clone())?;

			<<T as pallet::Config>::Currency as MultiReservableCurrency<_>>::reserve(
				T::LiquidCurrencyId::get(),
				&voter,
				liquid_amount,
			)?;

			// probably a create if not exist like api is there?
			let exists = LiquidVoteCount::<T>::try_get(target.clone()).is_ok();

			if exists {
				LiquidVoteCount::<T>::mutate(target.clone(), |votes| {
					votes.saturating_add(liquid_amount);
				});
			} else {
				LiquidVoteCount::<T>::insert(target.clone(), liquid_amount);
			}

			Voters::<T>::insert(voter.clone(), liquid_amount);

			// Emit an event.
			Self::deposit_event(Event::Voted(voter, target, liquid_amount));
			// Return a successful result
			Ok(())
		}

		/// should be called at end of era
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn apply_votes(origin: OriginFor<T>) -> DispatchResult {
			ensure_root(origin)?;
			// Probably super bad to sort and do unwraps
			// fix it before going to production
			let votes =
				<LiquidVoteCount<T> as IterableStorageMap<T::AccountId, BalanceOf<T>>>::iter()
					.map(|(tar, votes)| {
						// clear votes for the next era
						LiquidVoteCount::<T>::remove(&tar);
						(tar, votes)
					})
					.collect::<Vec<_>>();

			let mut votes: BoundedVec<_, T::MaxValidatorCount> =
				BoundedVec::try_from(votes).expect("value not expected to be higher");
			// sort in descending order of votes
			votes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

			let pot_account = &Self::account_id();
			let pot_origin = frame_system::RawOrigin::Signed(pot_account.clone()).into();
			// super naive selection of only top two validators
			// extremely unsafe to use indexes like this
			let val1 = T::Lookup::unlookup(votes[0].0.clone());
			let val2 = T::Lookup::unlookup(votes[1].0.clone());
			pallet_staking::Pallet::<T>::nominate(pot_origin, vec![val1.clone(), val2.clone()])?;
			// unreserve voter's money
			<Voters<T> as IterableStorageMap<T::AccountId, BalanceOf<T>>>::iter().for_each(
				|(voter, liquid_amount)| {
					// Clear votes for the next era
					Voters::<T>::remove(&voter);
					<<T as pallet::Config>::Currency as MultiReservableCurrency<_>>::unreserve(
						T::LiquidCurrencyId::get(),
						&voter,
						liquid_amount,
					);
				},
			);
			// Emit an event.
			Self::deposit_event(Event::NominationsApplied(votes[0].0.clone(), votes[1].0.clone()));
			// Return a successful result
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		#[transactional]
		pub fn request_unbond(
			origin: OriginFor<T>,
			#[pallet::compact] liquid_amount: BalanceOf<T>,
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
				liquid_amount,
			);

			// no rewards/slash are counted once unbonding is requested
			let staking_amount = Self::liquid_to_staking(liquid_amount)?;
			// can unwrap as we checked previously current era exists
			UnbondingRequests::<T>::insert(
				&who,
				(staking_amount, liquid_amount, current_era.unwrap()),
			);
			// unbond funds from pot account
			let pot_account = &Self::account_id();
			let pot_origin = frame_system::RawOrigin::Signed(pot_account.clone()).into();
			pallet_staking::Pallet::<T>::unbond(pot_origin, staking_amount)?;

			// Emit an event.
			Self::deposit_event(Event::RequestUnbond(liquid_amount, who));
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
				old_era + unbond_wait <= current_era.unwrap(),
				Error::<T>::UnbondingWaitNotComplete
			);

			let pot_account = Self::account_id();
			let pot_origin = frame_system::RawOrigin::Signed(pot_account.clone()).into();
			let _ = pallet_staking::Pallet::<T>::withdraw_unbonded(pot_origin, 0);

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
			if total_liquid.is_zero() || total_staking.is_zero() {
				T::DefaultMintRate::get()
			} else {
				MintRate::checked_from_rational(total_liquid, total_staking)
					.unwrap_or_else(T::DefaultMintRate::get)
			}
		}

		pub fn current_era() -> Option<EraIndex> {
			pallet_staking::Pallet::<T>::current_era()
		}
	}
}
