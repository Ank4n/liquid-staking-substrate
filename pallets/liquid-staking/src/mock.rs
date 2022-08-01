use crate as pallet_liquid_staking;
use frame_election_provider_support::{onchain, SequentialPhragmen};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
	curve::PiecewiseLinear,
	testing::{Header, UintAuthorityId},
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

use frame_system::{EnsureRoot, EnsureSigned};

use primitives::MintRate;
use sp_staking::{EraIndex, SessionIndex};

use crate::mock::sp_api_hidden_includes_construct_runtime::hidden_include::traits::GenesisBuild;
use frame_support::{
	parameter_types,
	traits::{
		ConstU128, ConstU16, ConstU32, ConstU64, EqualPrivilegeOnly, Nothing, OneSessionHandler,
	},
	PalletId,
};
use orml_currencies::BasicCurrencyAdapter;
use orml_traits::parameter_type_with_key;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

type Balance = u128;
pub type Amount = i128;
pub type BlockNumber = u64;
pub const RID_1: ReserveIdentifier = [1u8; 8];
pub const RID_2: ReserveIdentifier = [2u8; 8];
pub type ReserveIdentifier = [u8; 8];

type CurrencyId = u32;
pub const STAKING_CURRENCY_ID: CurrencyId = 1;
pub const LIQUID_CURRENCY_ID: CurrencyId = 2;
pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		LiquidStaking: pallet_liquid_staking::{Pallet, Call, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Staking: pallet_staking::{Pallet, Call, Config<T>, Storage, Event<T>},
		Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>},
		Currencies: orml_currencies::{Pallet, Call},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Democracy: pallet_democracy::{Pallet, Storage, Config<T>, Event<T>, Call},
		Historical: pallet_session::historical::{Pallet, Storage},
		Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(1024);
	pub static Period: BlockNumber = 5;
	pub static Offset: BlockNumber = 0;
}

type AccountId = u64;

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = STAKING_CURRENCY_ID;
}

impl orml_currencies::Config for Test {
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		1
	};
}

impl orml_tokens::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
	type MaxLocks = ConstU32<2>;
	type MaxReserves = ConstU32<2>;
	type ReserveIdentifier = ReserveIdentifier;
	type DustRemovalWhitelist = Nothing;
}

/// Another session handler struct to test on_disabled.
pub struct OtherSessionHandler;
impl OneSessionHandler<AccountId> for OtherSessionHandler {
	type Key = UintAuthorityId;

	fn on_genesis_session<'a, I: 'a>(_: I)
	where
		I: Iterator<Item = (&'a AccountId, Self::Key)>,
		AccountId: 'a,
	{
	}

	fn on_new_session<'a, I: 'a>(_: bool, _: I, _: I)
	where
		I: Iterator<Item = (&'a AccountId, Self::Key)>,
		AccountId: 'a,
	{
	}

	fn on_disabled(_validator_index: u32) {}
}

impl sp_runtime::BoundToRuntimeAppPublic for OtherSessionHandler {
	type Public = UintAuthorityId;
}

sp_runtime::impl_opaque_keys! {
	pub struct SessionKeys {
		pub other: OtherSessionHandler,
	}
}

impl pallet_session::Config for Test {
	type SessionManager = pallet_session::historical::NoteHistoricalRoot<Test, Staking>;
	type Keys = SessionKeys;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionHandler = (OtherSessionHandler,);
	type Event = Event;
	type ValidatorId = AccountId;
	type ValidatorIdOf = pallet_staking::StashOf<Test>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type WeightInfo = ();
}

impl pallet_session::historical::Config for Test {
	type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = pallet_staking::ExposureOf<Test>;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type MaxReserves = ConstU32<2>;
	type ReserveIdentifier = ReserveIdentifier;
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ConstU128<2>;
	type AccountStore = System;
	type WeightInfo = ();
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<3>;
	type WeightInfo = ();
}

pallet_staking_reward_curve::build! {
	const REWARD_CURVE: PiecewiseLinear<'static> = curve!(
		min_inflation: 0_025_000u64,
		max_inflation: 0_100_000,
		ideal_stake: 0_500_000,
		falloff: 0_050_000,
		max_piece_count: 40,
		test_precision: 0_005_000,
	);
}

parameter_types! {
	pub const SessionsPerEra: SessionIndex = 3;
	pub const BondingDuration: EraIndex = 3;
	pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
	pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
	pub static ExistentialDeposit: Balance = 1;
}

pub struct OnChainSeqPhragmen;
impl onchain::Config for OnChainSeqPhragmen {
	type System = Test;
	type Solver = SequentialPhragmen<u64, Perbill>;
	type DataProvider = Staking;
	type WeightInfo = ();
}

impl pallet_staking::Config for Test {
	type MaxNominations = ConstU32<16>;
	type Currency = Balances;
	type CurrencyBalance = <Self as pallet_balances::Config>::Balance;
	type UnixTime = pallet_timestamp::Pallet<Self>;
	type CurrencyToVote = frame_support::traits::SaturatingCurrencyToVote;
	type RewardRemainder = ();
	type Event = Event;
	type Slash = ();
	type Reward = ();
	type SessionsPerEra = ();
	type SlashDeferDuration = ();
	type SlashCancelOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type BondingDuration = ();
	type SessionInterface = Self;
	type EraPayout = pallet_staking::ConvertCurve<RewardCurve>;
	type NextNewSession = ();
	type MaxNominatorRewardedPerValidator = ConstU32<64>;
	type OffendingValidatorsThreshold = ();
	type ElectionProvider = onchain::UnboundedExecution<OnChainSeqPhragmen>;
	type GenesisElectionProvider = Self::ElectionProvider;
	type MaxUnlockingChunks = ConstU32<32>;
	type VoterList = pallet_staking::UseNominatorsAndValidatorsMap<Self>;
	type OnStakerSlash = ();
	type BenchmarkingConfig = pallet_staking::TestBenchmarkingConfig;
	type WeightInfo = ();
}

parameter_types! {
	pub const StakingCurrencyId: CurrencyId = STAKING_CURRENCY_ID;
	pub const LiquidCurrencyId: CurrencyId = LIQUID_CURRENCY_ID;
	pub const MyPalletId: PalletId = PalletId(*b"palletls");
	pub DefaultMintRate: MintRate = MintRate::from_inner(10);
	pub const UnBondWait: EraIndex = 28;
	pub static BondThreshold: Balance = 0;
	pub static UnbondThreshold: Balance = 0;
}

impl pallet_liquid_staking::Config for Test {
	type Event = Event;
	type PalletId = MyPalletId;
	type Currency = Currencies;
	type StakingCurrencyId = StakingCurrencyId;
	type LiquidCurrencyId = LiquidCurrencyId;
	type DefaultMintRate = DefaultMintRate;
	type BondThreshold = BondThreshold;
	type UnbondThreshold = UnbondThreshold;
}

parameter_types! {
	pub const LaunchPeriod: BlockNumber = 10;
	pub const VotingPeriod: BlockNumber = 10;
	pub const VoteLockingPeriod: BlockNumber = 10;
	pub const FastTrackVotingPeriod: BlockNumber = 5;
	pub const EnactmentPeriod: BlockNumber = 10;
	pub const CooloffPeriod: BlockNumber = 10;
	pub const MinimumDeposit: Balance = 10;
	pub const MaxVotes: u32 = 10;
	pub const MaxProposals: u32 = 10;
	pub const PreimageByteDeposit: Balance = 10;
	pub const InstantAllowed: bool = false;
}
impl pallet_democracy::Config for Test {
	type Proposal = Call;
	type Event = Event;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type VoteLockingPeriod = VoteLockingPeriod;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	type MinimumDeposit = MinimumDeposit;
	type ExternalOrigin = EnsureRoot<AccountId>;
	type ExternalMajorityOrigin = EnsureRoot<AccountId>;
	type ExternalDefaultOrigin = EnsureRoot<AccountId>;
	type FastTrackOrigin = EnsureRoot<AccountId>;
	type InstantOrigin = EnsureRoot<AccountId>;
	type CancellationOrigin = EnsureRoot<AccountId>;
	type CancelProposalOrigin = EnsureRoot<AccountId>;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	type VetoOrigin = EnsureSigned<AccountId>;
	type CooloffPeriod = CooloffPeriod;
	type PreimageByteDeposit = PreimageByteDeposit;
	type Slash = ();
	type InstantAllowed = InstantAllowed;
	type Scheduler = Scheduler;
	type MaxVotes = MaxVotes;
	type OperationalPreimageOrigin = EnsureSigned<AccountId>;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = ();
	type MaxProposals = MaxProposals;
}

impl pallet_scheduler::Config for Test {
	type Event = Event;
	type Origin = Origin;
	type PalletsOrigin = OriginCaller;
	type Call = Call;
	type MaximumWeight = ();
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = ();
	type WeightInfo = ();
	type OriginPrivilegeCmp = EqualPrivilegeOnly; // TODO : Simplest type, maybe there is better ?
	type PreimageProvider = ();
	type NoPreimagePostponement = ();
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
	Call: From<C>,
{
	type OverarchingCall = Call;
	type Extrinsic = sp_runtime::testing::TestXt<Call, ()>;
}

pub use pallet_staking::StakerStatus;
pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self { balances: vec![] }
	}
}

impl ExtBuilder {
	pub fn balances(mut self, balances: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.balances = balances;
		self
	}

	pub fn topup_balances(self) -> Self {
		self.balances(vec![
			(1, STAKING_CURRENCY_ID, 100),
			(2, STAKING_CURRENCY_ID, 100),
			(1, LIQUID_CURRENCY_ID, 100),
			(2, LIQUID_CURRENCY_ID, 100),
			(3, STAKING_CURRENCY_ID, 300),
			(4, STAKING_CURRENCY_ID, 400),
			// controllers
			(10, STAKING_CURRENCY_ID, 10),
			(20, STAKING_CURRENCY_ID, 10),
			(30, STAKING_CURRENCY_ID, 10),
			(40, STAKING_CURRENCY_ID, 10),
			// stashes
			(11, STAKING_CURRENCY_ID, 2000),
			(21, STAKING_CURRENCY_ID, 2000),
			(31, STAKING_CURRENCY_ID, 3000),
			(41, STAKING_CURRENCY_ID, 3000),
			// nominators
			(100, STAKING_CURRENCY_ID, 1000),
			(101, STAKING_CURRENCY_ID, 1000),
			(102, STAKING_CURRENCY_ID, 2000),
			(103, STAKING_CURRENCY_ID, 3000),
		])
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: self
				.balances
				.clone()
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id == STAKING_CURRENCY_ID)
				.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self
				.balances
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id != STAKING_CURRENCY_ID)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let stakers = vec![
			// (stash, ctrl, stake, status)
			// these two will be elected in the default test where we elect 2.
			(11, 10, 100, StakerStatus::<AccountId>::Validator),
			(21, 20, 100, StakerStatus::<AccountId>::Validator),
			// a loser validator
			(31, 30, 50, StakerStatus::<AccountId>::Validator),
			// an idle validator
			(41, 40, 100, StakerStatus::<AccountId>::Idle),
			(101, 100, 50, StakerStatus::<AccountId>::Nominator(vec![11, 21])),
		];

		let _ = pallet_staking::GenesisConfig::<Test> {
			stakers: stakers.clone(),
			validator_count: 2,
			minimum_validator_count: 0,
			invulnerables: vec![],
			slash_reward_fraction: Perbill::from_percent(10),
			min_nominator_bond: ExistentialDeposit::get(),
			min_validator_bond: ExistentialDeposit::get(),
			..Default::default()
		}
		.assimilate_storage(&mut t);

		let _ = pallet_session::GenesisConfig::<Test> {
			keys: stakers
				.into_iter()
				.map(|(id, ..)| (id, id, SessionKeys { other: id.into() }))
				.collect(),
		}
		.assimilate_storage(&mut t);

		t.into()
	}
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}
