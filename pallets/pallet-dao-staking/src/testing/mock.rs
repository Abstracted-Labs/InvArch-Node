use crate::{self as pallet_dao_staking, CustomAggregateMessageOrigin, CustomMessageProcessor};
use codec::{Decode, Encode};
use core::convert::{TryFrom, TryInto};
use cumulus_primitives_core::AggregateMessageOrigin;
use frame_support::{
    construct_runtime, derive_impl,
    dispatch::DispatchClass,
    parameter_types,
    traits::{
        fungibles::Credit, ConstU128, ConstU32, Contains, Currency, OnFinalize, OnInitialize,
    },
    weights::{
        constants::{BlockExecutionWeight, ExtrinsicBaseWeight, WEIGHT_REF_TIME_PER_SECOND},
        ConstantMultiplier, Weight,
    },
    PalletId,
};
use pallet_dao_manager::DaoAccountDerivation;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    AccountId32, BuildStorage, Perbill,
};

pub(crate) type AccountId = AccountId32;
pub(crate) type BlockNumber = u64;
pub(crate) type Balance = u128;
pub(crate) type EraIndex = u32;

type Block = frame_system::mocking::MockBlock<Test>;

pub(crate) const EXISTENTIAL_DEPOSIT: Balance = 2;
pub(crate) const MAX_NUMBER_OF_STAKERS: u32 = 4;
pub(crate) const _MAX_NUMBER_OF_STAKERS_TINKERNET: u32 = 10000;
pub(crate) const MINIMUM_STAKING_AMOUNT: Balance = 10;
pub(crate) const MAX_UNLOCKING: u32 = 4;
pub(crate) const UNBONDING_PERIOD: EraIndex = 3;
pub(crate) const MAX_ERA_STAKE_VALUES: u32 = 8;
pub(crate) const BLOCKS_PER_ERA: BlockNumber = 3;
pub(crate) const REGISTER_DEPOSIT: Balance = 10;
const MICROUNIT: Balance = 1_000_000;

construct_runtime!(
    pub struct Test {
        System: frame_system,
        Balances: pallet_balances,
        Timestamp: pallet_timestamp,
        OcifStaking: pallet_dao_staking,
        CoreAssets: orml_tokens,
        INV4: pallet_dao_manager,
        MessageQueue: pallet_message_queue,
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type RuntimeTask = RuntimeTask;
    type RuntimeOrigin = RuntimeOrigin;
    type Nonce = u64;
    type RuntimeCall = RuntimeCall;
    type Block = Block;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
    pub const MaxLocks: u32 = 4;
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
    type MaxLocks = MaxLocks;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = [u8; 8];
    type MaxFreezes = ();
}

parameter_types! {
    pub const MinimumPeriod: u64 = 3;
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

parameter_types! {
    pub const RegisterDeposit: Balance = REGISTER_DEPOSIT;
    pub const BlockPerEra: BlockNumber = BLOCKS_PER_ERA;
    pub const MaxStakersPerDao: u32 = MAX_NUMBER_OF_STAKERS;
    pub const MinimumStakingAmount: Balance = MINIMUM_STAKING_AMOUNT;
    pub const PotId: PalletId = PalletId(*b"ocif-pot");
    pub const MaxUnlocking: u32 = MAX_UNLOCKING;
    pub const UnbondingPeriod: EraIndex = UNBONDING_PERIOD;
    pub const MaxEraStakeValues: u32 = MAX_ERA_STAKE_VALUES;
    pub const RewardRatio: (u32, u32) = (50, 50);
}

pub type DaoId = u32;

pub const THRESHOLD: u128 = 50;

parameter_types! {
    pub const MaxMetadata: u32 = 100;
    pub const MaxCallers: u32 = 100;
    pub const DaoSeedBalance: u32 = 1000000;
    pub const DaoCreationFee: u128 = 1000000000000;
    pub const GenesisHash: <Test as frame_system::Config>::Hash = H256([
        212, 46, 150, 6, 169, 149, 223, 228, 51, 220, 121, 85, 220, 42, 112, 244, 149, 243, 80,
        243, 115, 218, 162, 0, 9, 138, 232, 68, 55, 129, 106, 210,
    ]);
    pub const RelayAssetId: u32 = 9999;
    pub const UnregisterOrigin: CustomAggregateMessageOrigin<AggregateMessageOrigin> = CustomAggregateMessageOrigin::UnregisterMessageOrigin;
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo, Debug)]
pub struct FeeCharger;

impl pallet_dao_manager::fee_handling::MultisigFeeHandler<Test> for FeeCharger {
    type Pre = (
        // tip
        Balance,
        // who paid the fee
        AccountId,
        // imbalance resulting from withdrawing the fee
        (),
        // asset_id for the transaction payment
        Option<u32>,
    );

    fn pre_dispatch(
        fee_asset: &pallet_dao_manager::fee_handling::FeeAsset,
        who: &AccountId,
        _call: &RuntimeCall,
        _info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        _len: usize,
    ) -> Result<Self::Pre, frame_support::unsigned::TransactionValidityError> {
        Ok((
            0u128,
            who.clone(),
            (),
            match fee_asset {
                pallet_dao_manager::fee_handling::FeeAsset::Native => None,
                pallet_dao_manager::fee_handling::FeeAsset::Relay => Some(1u32),
            },
        ))
    }

    fn post_dispatch(
        _fee_asset: &pallet_dao_manager::fee_handling::FeeAsset,
        _pre: Option<Self::Pre>,
        _info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        _post_info: &sp_runtime::traits::PostDispatchInfoOf<RuntimeCall>,
        _len: usize,
        _result: &sp_runtime::DispatchResult,
    ) -> Result<(), frame_support::unsigned::TransactionValidityError> {
        Ok(())
    }

    fn handle_creation_fee(
        _imbalance: pallet_dao_manager::fee_handling::FeeAssetNegativeImbalance<
            <Balances as Currency<AccountId>>::NegativeImbalance,
            Credit<AccountId, CoreAssets>,
        >,
    ) {
    }
}

pub struct DustRemovalWhitelist;
impl Contains<AccountId> for DustRemovalWhitelist {
    fn contains(_: &AccountId) -> bool {
        true
    }
}

pub type Amount = i128;

orml_traits::parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: u32| -> Balance {
        ExistentialDeposit::get()
    };
}

impl orml_tokens::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = u32;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type MaxLocks = MaxLocks;
    type DustRemovalWhitelist = DustRemovalWhitelist;
    type MaxReserves = MaxCallers;
    type ReserveIdentifier = [u8; 8];
    type CurrencyHooks = ();
}

impl pallet_dao_manager::Config for Test {
    type MaxMetadata = MaxMetadata;
    type DaoId = u32;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type RuntimeCall = RuntimeCall;
    type MaxCallers = MaxCallers;
    type DaoSeedBalance = DaoSeedBalance;
    type AssetsProvider = CoreAssets;
    type RuntimeOrigin = RuntimeOrigin;
    type DaoCreationFee = DaoCreationFee;
    type FeeCharger = FeeCharger;
    type WeightInfo = pallet_dao_manager::weights::SubstrateWeight<Test>;

    type Tokens = CoreAssets;
    type RelayAssetId = RelayAssetId;
    type RelayDaoCreationFee = DaoCreationFee;
    type MaxCallSize = ConstU32<51200>;

    type ParaId = ConstU32<2125>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
}

impl pallet_dao_staking::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BlocksPerEra = BlockPerEra;
    type RegisterDeposit = RegisterDeposit;
    type MaxStakersPerDao = MaxStakersPerDao;
    type MinimumStakingAmount = MinimumStakingAmount;
    type PotId = PotId;
    type ExistentialDeposit = ExistentialDeposit;
    type MaxUnlocking = MaxUnlocking;
    type UnbondingPeriod = UnbondingPeriod;
    type MaxEraStakeValues = MaxEraStakeValues;
    type MaxDescriptionLength = ConstU32<300>;
    type MaxNameLength = ConstU32<20>;
    type MaxImageUrlLength = ConstU32<60>;
    type RewardRatio = RewardRatio;
    type StakeThresholdForActiveDao = ConstU128<THRESHOLD>;
    type WeightInfo = crate::weights::SubstrateWeight<Test>;
    type StakingMessage = frame_support::traits::EnqueueWithOrigin<MessageQueue, UnregisterOrigin>;
    type WeightToFee = ConstantMultiplier<Balance, ZeroFee>;
    type OnUnbalanced = ();
}

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
/// `Operational` extrinsics. (from tinkernet)
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// We allow for 0.5 of a second of compute with a 12 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
    WEIGHT_REF_TIME_PER_SECOND.saturating_div(2),
    cumulus_primitives_core::relay_chain::MAX_POV_SIZE as u64,
);

/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers.
/// This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

parameter_types! {
    pub RuntimeBlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::builder()
    .base_block(BlockExecutionWeight::get())
    .for_class(DispatchClass::all(), |weights| {
        weights.base_extrinsic = ExtrinsicBaseWeight::get();
    })
    .for_class(DispatchClass::Normal, |weights| {
        weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
    })
    .for_class(DispatchClass::Operational, |weights| {
        weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
        // Operational transactions have some extra reserved space, so that they
        // are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
        weights.reserved = Some(
            MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
        );
    })
    .avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
    .build_or_panic();
    pub MessageQueueServiceWeight: Weight = Perbill::from_percent(35) * RuntimeBlockWeights::get().max_block;
    pub const MessageQueueMaxStale: u32 = 8;
    pub const MessageQueueHeapSize: u32 = 128 * 1048;
    pub const TransactionByteFee: Balance = 10 * MICROUNIT;
    pub const ZeroFee: Balance = 0;
}

impl pallet_message_queue::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_message_queue::weights::SubstrateWeight<Self>;
    #[cfg(feature = "runtime-benchmarks")]
    type MessageProcessor = pallet_message_queue::mock_helpers::NoopMessageProcessor<
        CustomAggregateMessageOrigin<AggregateMessageOrigin>,
    >;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type MessageProcessor = CustomMessageProcessor<
        CustomAggregateMessageOrigin<AggregateMessageOrigin>,
        AggregateMessageOrigin,
        pallet_message_queue::mock_helpers::NoopMessageProcessor<AggregateMessageOrigin>,
        RuntimeCall,
        Test,
    >;
    type Size = u32;
    type QueueChangeHandler = ();
    type QueuePausedQuery = ();
    type HeapSize = MessageQueueHeapSize;
    type MaxStale = MessageQueueMaxStale;
    type ServiceWeight = MessageQueueServiceWeight;
}
pub struct ExternalityBuilder;

pub fn account(dao: DaoId) -> AccountId {
    INV4::derive_dao_account(dao)
}

pub const A: DaoId = 0;
pub const B: DaoId = 1;
pub const C: DaoId = 2;
pub const D: DaoId = 3;
pub const E: DaoId = 4;
pub const F: DaoId = 5;
pub const G: DaoId = 6;
pub const H: DaoId = 7;
pub const I: DaoId = 8;
pub const J: DaoId = 9;
pub const K: DaoId = 10;
pub const L: DaoId = 11;
pub const M: DaoId = 12;
pub const N: DaoId = 13;

impl ExternalityBuilder {
    pub fn build() -> TestExternalities {
        let storage = frame_system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();

        let mut ext = TestExternalities::from(storage);

        ext.execute_with(|| {
            Balances::resolve_creating(&account(A), Balances::issue(9000));
            Balances::resolve_creating(&account(B), Balances::issue(800));
            Balances::resolve_creating(&account(C), Balances::issue(10000));
            Balances::resolve_creating(&account(D), Balances::issue(4900));
            Balances::resolve_creating(&account(E), Balances::issue(3800));
            Balances::resolve_creating(&account(F), Balances::issue(10));
            Balances::resolve_creating(&account(G), Balances::issue(1000));
            Balances::resolve_creating(&account(H), Balances::issue(2000));
            Balances::resolve_creating(&account(I), Balances::issue(10000));
            Balances::resolve_creating(&account(J), Balances::issue(300));
            Balances::resolve_creating(&account(K), Balances::issue(400));
            Balances::resolve_creating(&account(L), Balances::issue(10));
            Balances::resolve_creating(&account(M), Balances::issue(EXISTENTIAL_DEPOSIT));
            Balances::resolve_creating(&account(N), Balances::issue(1_000_000_000_000));
        });

        ext.execute_with(|| System::set_block_number(1));

        ext
    }
}

pub const ISSUE_PER_BLOCK: Balance = 1000000;

pub const ISSUE_PER_ERA: Balance = ISSUE_PER_BLOCK * BLOCKS_PER_ERA as u128;

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        OcifStaking::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);

        OcifStaking::rewards(Balances::issue(ISSUE_PER_BLOCK));

        OcifStaking::on_initialize(System::block_number());
        MessageQueue::on_initialize(System::block_number());
    }
}

pub fn run_to_block_no_rewards(n: u64) {
    while System::block_number() < n {
        OcifStaking::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        OcifStaking::on_initialize(System::block_number());
        MessageQueue::on_initialize(System::block_number());
    }
}

pub fn issue_rewards(amount: Balance) {
    OcifStaking::rewards(Balances::issue(amount));
}

pub fn run_for_blocks(n: u64) {
    run_to_block(System::block_number() + n);
}

pub fn run_for_blocks_no_rewards(n: u64) {
    run_to_block_no_rewards(System::block_number() + n);
}

pub fn advance_to_era(n: EraIndex) {
    while OcifStaking::current_era() < n {
        run_for_blocks(1);
    }
}

pub fn advance_to_era_no_rewards(n: EraIndex) {
    while OcifStaking::current_era() < n {
        run_for_blocks_no_rewards(1);
    }
}

pub fn initialize_first_block() {
    assert_eq!(System::block_number(), 1 as BlockNumber);

    OcifStaking::on_initialize(System::block_number());
    MessageQueue::on_initialize(System::block_number());
    run_to_block(2);
}

pub fn split_reward_amount(amount: Balance) -> (Balance, Balance) {
    let percent = Perbill::from_percent(RewardRatio::get().0);

    let amount_for_dao = percent * amount;

    (amount_for_dao, amount - amount_for_dao)
}
