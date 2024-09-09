use crate::{
    traits::{ChainAssetsList, ChainList},
    *,
};
use codec::{Decode, Encode, MaxEncodedLen};
use core::convert::TryFrom;
use frame_support::{
    derive_impl, parameter_types,
    traits::{
        fungibles::Credit, ConstU128, ConstU32, ConstU64, Contains, Currency, EnsureOrigin,
        EnsureOriginWithArg, Everything, Nothing,
    },
    weights::ConstantMultiplier,
};
use frame_system::EnsureRoot;
use orml_asset_registry::AssetMetadata;
use pallet_balances::AccountData;
use pallet_dao_manager::fee_handling::*;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{traits::IdentityLookup, AccountId32, BuildStorage};
pub use sp_std::{cell::RefCell, fmt::Debug};
use sp_std::{convert::TryInto, vec};
use xcm::latest::prelude::*;
use xcm_builder::{
    AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom,
    AllowTopLevelPaidExecutionFrom, FixedRateOfFungible, FixedWeightBounds,
    FungibleAdapter as XcmCurrencyAdapter, IsConcrete, SignedAccountId32AsNative,
    SignedToAccountId32, SovereignSignedViaLocation, TakeWeightCredit,
};
use xcm_executor::XcmExecutor;

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;

type AccountId = AccountId32;

const MICROUNIT: Balance = 1_000_000;

pub const EXISTENTIAL_DEPOSIT: Balance = 1_000_000_000;

pub const ALICE: AccountId = AccountId32::new([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);
pub const BOB: AccountId = AccountId32::new([
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);
pub const CHARLIE: AccountId = AccountId32::new([
    2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Tokens: orml_tokens,
        CoreAssets: orml_tokens2,
        AssetRegistry: orml_asset_registry,
        dao_manager: pallet_dao_manager,
        Rings: pallet,
        XcmPallet: pallet_xcm,
    }
);

pub struct TestBaseCallFilter;
impl Contains<RuntimeCall> for TestBaseCallFilter {
    fn contains(_c: &RuntimeCall) -> bool {
        true
    }
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeTask = RuntimeTask;
    type Nonce = u64;
    type Block = Block;
    type RuntimeCall = RuntimeCall;
    type Hash = H256;
    type Hashing = ::sp_runtime::traits::BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type BlockWeights = ();
    type BlockLength = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type DbWeight = ();
    type BaseCallFilter = TestBaseCallFilter;
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<50>;
    /// The type for recording an account's balance.
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxReserves = ConstU32<50>;
    type ReserveIdentifier = [u8; 8];
    type MaxHolds = ConstU32<1>;
    type MaxFreezes = ();
}

thread_local! {
      pub static SENT_XCM: RefCell<Vec<(MultiLocation, Xcm<()>)>> = RefCell::new(Vec::new());
}

/// Sender that never returns error, always sends
pub struct TestSendXcm;
impl SendXcm for TestSendXcm {
    type Ticket = (MultiLocation, Xcm<()>);
    fn validate(
        dest: &mut Option<MultiLocation>,
        msg: &mut Option<Xcm<()>>,
    ) -> SendResult<(MultiLocation, Xcm<()>)> {
        let pair = (dest.take().unwrap(), msg.take().unwrap());
        Ok((pair, MultiAssets::new()))
    }
    fn deliver(pair: (MultiLocation, Xcm<()>)) -> Result<XcmHash, SendError> {
        let hash = pair.1.using_encoded(sp_io::hashing::blake2_256);
        SENT_XCM.with(|q| q.borrow_mut().push(pair));
        Ok(hash)
    }
}

parameter_types! {
    pub const RelayLocation: MultiLocation = MultiLocation::parent();
    pub const AnyNetwork: Option<NetworkId> = None;
    pub Ancestry: MultiLocation = Here.into();
    pub UnitWeightCost: u64 = 1_000;
}

pub type SovereignAccountOf = (AccountId32Aliases<AnyNetwork, AccountId>,);

pub type LocalAssetTransactor =
    XcmCurrencyAdapter<Balances, IsConcrete<RelayLocation>, SovereignAccountOf, AccountId, ()>;

type LocalOriginConverter = (
    SovereignSignedViaLocation<SovereignAccountOf, RuntimeOrigin>,
    SignedAccountId32AsNative<AnyNetwork, RuntimeOrigin>,
);

parameter_types! {
    pub const BaseXcmWeight: u64 = 1_000;
    pub CurrencyPerSecond: (xcm::latest::AssetId, u128, u128) = (Concrete(RelayLocation::get()), 1, 1);
    pub TrustedAssets: (MultiAssetFilter, MultiLocation) = (All.into(), Here.into());
    pub const MaxInstructions: u32 = 100;
    pub UniversalLocation: InteriorMultiLocation = Here;
    pub const MaxAssetsIntoHolding: u32 = 64;
}

pub type Barrier = (
    TakeWeightCredit,
    AllowTopLevelPaidExecutionFrom<Everything>,
    AllowKnownQueryResponses<XcmPallet>,
    AllowSubscriptionsFrom<Everything>,
);

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
    type RuntimeCall = RuntimeCall;
    type XcmSender = TestSendXcm;
    type AssetTransactor = LocalAssetTransactor;
    type OriginConverter = LocalOriginConverter;
    type IsReserve = ();
    type IsTeleporter = ();
    type Barrier = Barrier;
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type Trader = FixedRateOfFungible<CurrencyPerSecond, ()>;
    type ResponseHandler = XcmPallet;
    type AssetTrap = XcmPallet;
    type AssetClaims = XcmPallet;
    type SubscriptionService = XcmPallet;
    type AssetLocker = ();
    type AssetExchanger = ();
    type FeeManager = ();
    type MessageExporter = ();
    type UniversalAliases = Nothing;
    type UniversalLocation = UniversalLocation;
    type CallDispatcher = RuntimeCall;
    type SafeCallFilter = Everything;
    type PalletInstancesInfo = AllPalletsWithSystem;
    type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
    type Aliasers = ();
}

pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, AnyNetwork>;

impl pallet_xcm::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type SendXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmRouter = TestSendXcm;
    type ExecuteXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmExecuteFilter = Everything;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmTeleportFilter = Everything;
    type XcmReserveTransferFilter = Everything;
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    type UniversalLocation = UniversalLocation;
    type MaxLockers = frame_support::traits::ConstU32<8>;
    type MaxRemoteLockConsumers = frame_support::traits::ConstU32<0>;
    type Currency = Balances;
    type CurrencyMatcher = IsConcrete<RelayLocation>;
    type AdminOrigin = EnsureRoot<AccountId>;
    type TrustedLockers = ();
    type SovereignAccountOf = AccountId32Aliases<(), AccountId32>;
    type RemoteLockConsumerIdentifier = ();
    type WeightInfo = pallet_xcm::TestWeightInfo;
}

const UNIT: u128 = 1000000000000;

orml_traits2::parameter_type_with_key! {
    pub DaoExistentialDeposits: |_currency_id: <Test as pallet_dao_manager::Config>::DaoId| -> Balance {
        1u128
    };
}

pub struct DaoDustRemovalWhitelist;
impl Contains<AccountId> for DaoDustRemovalWhitelist {
    fn contains(_: &AccountId) -> bool {
        true
    }
}

pub struct DisallowIfFrozen;
impl
    orml_traits2::currency::OnTransfer<
        AccountId,
        <Test as pallet_dao_manager::Config>::DaoId,
        Balance,
    > for DisallowIfFrozen
{
    fn on_transfer(
        currency_id: <Test as pallet_dao_manager::Config>::DaoId,
        _from: &AccountId,
        _to: &AccountId,
        _amount: Balance,
    ) -> sp_runtime::DispatchResult {
        if let Some(true) = dao_manager::is_asset_frozen(currency_id) {
            Err(sp_runtime::DispatchError::Token(
                sp_runtime::TokenError::Frozen,
            ))
        } else {
            Ok(())
        }
    }
}

pub struct HandleNewMembers;
impl orml_traits2::Happened<(AccountId, <Test as pallet_dao_manager::Config>::DaoId)>
    for HandleNewMembers
{
    fn happened((member, dao_id): &(AccountId, <Test as pallet_dao_manager::Config>::DaoId)) {
        dao_manager::add_member(dao_id, member)
    }
}

pub struct HandleRemovedMembers;
impl orml_traits2::Happened<(AccountId, <Test as pallet_dao_manager::Config>::DaoId)>
    for HandleRemovedMembers
{
    fn happened((member, dao_id): &(AccountId, <Test as pallet_dao_manager::Config>::DaoId)) {
        dao_manager::remove_member(dao_id, member)
    }
}

pub struct INV4TokenHooks;
impl
    orml_traits2::currency::MutationHooks<
        AccountId,
        <Test as pallet_dao_manager::Config>::DaoId,
        Balance,
    > for INV4TokenHooks
{
    type PreTransfer = DisallowIfFrozen;
    type OnDust = ();
    type OnSlash = ();
    type PreDeposit = ();
    type PostDeposit = ();
    type PostTransfer = ();
    type OnNewTokenAccount = HandleNewMembers;
    type OnKilledTokenAccount = HandleRemovedMembers;
}

impl orml_tokens2::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = i128;
    type CurrencyId = <Test as pallet_dao_manager::Config>::DaoId;
    type WeightInfo = ();
    type ExistentialDeposits = DaoExistentialDeposits;
    type MaxLocks = ConstU32<0u32>;
    type MaxReserves = ConstU32<0u32>;
    type DustRemovalWhitelist = DaoDustRemovalWhitelist;
    type ReserveIdentifier = [u8; 8];
    type CurrencyHooks = INV4TokenHooks;
}

parameter_types! {
    pub const MaxMetadata: u32 = 10000;
    pub const MaxCallers: u32 = 10000;
    pub const DaoSeedBalance: Balance = 1000000u128;
    pub const DaoCreationFee: Balance = UNIT;
    pub const StringLimit: u32 = 2125;
    pub const RelayDaoCreationFee: Balance = UNIT;
}

pub type AssetId = u32;

pub const NATIVE_ASSET_ID: AssetId = 0;
pub const RELAY_ASSET_ID: AssetId = 1;

parameter_types! {
    pub const NativeAssetId: AssetId = NATIVE_ASSET_ID;
    pub const RelayAssetId: AssetId = RELAY_ASSET_ID;
    pub const ExistentialDeposit: u128 = 100000000000;
    pub const MaxLocks: u32 = 1;
    pub const MaxReserves: u32 = 1;
    pub const TransactionByteFee: Balance = 10 * MICROUNIT;
}

pub struct AssetAuthority;
impl EnsureOriginWithArg<RuntimeOrigin, Option<u32>> for AssetAuthority {
    type Success = ();

    fn try_origin(
        origin: RuntimeOrigin,
        _asset_id: &Option<u32>,
    ) -> Result<Self::Success, RuntimeOrigin> {
        <EnsureRoot<_> as EnsureOrigin<RuntimeOrigin>>::try_origin(origin)
    }
}

impl orml_asset_registry::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AuthorityOrigin = AssetAuthority;
    type AssetId = AssetId;
    type Balance = Balance;
    type AssetProcessor = orml_asset_registry::SequentialId<Test>;
    type CustomMetadata = ();
    type WeightInfo = ();
    type StringLimit = StringLimit;
}

pub struct DustRemovalWhitelist;
impl Contains<AccountId> for DustRemovalWhitelist {
    fn contains(_: &AccountId) -> bool {
        true
    }
}

pub type Amount = i128;

orml_traits::parameter_type_with_key! {
      pub ExistentialDeposits: |currency_id: AssetId| -> Balance {
          if currency_id == &NATIVE_ASSET_ID {
              ExistentialDeposit::get()
          } else {
              orml_asset_registry::ExistentialDeposits::<Test>::get(currency_id)
          }
      };
}

impl orml_tokens::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type MaxLocks = MaxLocks;
    type DustRemovalWhitelist = DustRemovalWhitelist;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type CurrencyHooks = ();
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo, Debug)]
pub struct FeeCharger;

impl MultisigFeeHandler<Test> for FeeCharger {
    type Pre = (
        // tip
        Balance,
        // who paid the fee
        AccountId,
        // imbalance resulting from withdrawing the fee
        (),
        // asset_id for the transaction payment
        Option<AssetId>,
    );

    fn pre_dispatch(
        fee_asset: &FeeAsset,
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
                FeeAsset::Native => None,
                FeeAsset::Relay => Some(1u32),
            },
        ))
    }

    fn post_dispatch(
        _fee_asset: &FeeAsset,
        _pre: Option<Self::Pre>,
        _info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        _post_info: &sp_runtime::traits::PostDispatchInfoOf<RuntimeCall>,
        _len: usize,
        _result: &sp_runtime::DispatchResult,
    ) -> Result<(), frame_support::unsigned::TransactionValidityError> {
        Ok(())
    }

    fn handle_creation_fee(
        _imbalance: FeeAssetNegativeImbalance<
            <Balances as Currency<AccountId>>::NegativeImbalance,
            Credit<AccountId, Tokens>,
        >,
    ) {
    }
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

    type Tokens = Tokens;
    type RelayAssetId = RelayAssetId;
    type RelayDaoCreationFee = RelayDaoCreationFee;
    type MaxCallSize = ConstU32<51200>;

    type ParaId = ConstU32<2125>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
}

parameter_types! {
    pub ParaId: u32 = 2125u32;
    pub MaxWeightedLength: u32 = 100_000;
    pub DaoPalletIndex: u8 = 2u8;
}

impl pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Chains = Chains;
    type WeightInfo = weights::SubstrateWeight<Test>;
    type MaxXCMCallLength = ConstU32<100_000>;
    type MaintenanceOrigin = EnsureRoot<AccountId>;
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, MaxEncodedLen, Debug, TypeInfo)]
pub enum Chains {
    Relay,
    ChainA,
    ChainB,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, MaxEncodedLen, Debug, TypeInfo)]
pub enum Assets {
    AssetA,
    AssetB,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, MaxEncodedLen, Debug, TypeInfo)]
pub enum ChainAssets {
    Relay(Assets),
    ChainA(Assets),
    ChainB(Assets),
}

impl ChainAssetsList for ChainAssets {
    type Chains = Chains;

    fn get_chain(&self) -> Self::Chains {
        match self {
            Self::ChainA(_) => Chains::ChainA,
            Self::ChainB(_) => Chains::ChainB,
            Self::Relay(_) => Chains::Relay,
        }
    }

    fn get_asset_location(&self) -> MultiLocation {
        match {
            match self {
                Self::ChainA(asset) => asset,
                Self::ChainB(asset) => asset,
                Self::Relay(asset) => asset,
            }
        } {
            Assets::AssetA => MultiLocation {
                parents: 1,
                interior: Junctions::X1(Junction::Parachain(1234)),
            },

            Assets::AssetB => MultiLocation {
                parents: 1,
                interior: Junctions::X1(Junction::Parachain(2345)),
            },
        }
    }
}

impl ChainList for Chains {
    type Balance = Balance;
    type ChainAssets = ChainAssets;

    fn get_location(&self) -> MultiLocation {
        match self {
            Self::ChainA => MultiLocation {
                parents: 1,
                interior: Junctions::X1(Junction::Parachain(1234)),
            },
            Self::ChainB => MultiLocation {
                parents: 1,
                interior: Junctions::X1(Junction::Parachain(2345)),
            },
            Self::Relay => MultiLocation {
                parents: 1,
                interior: Junctions::Here,
            },
        }
    }

    fn get_main_asset(&self) -> Self::ChainAssets {
        match self {
            Self::ChainA => ChainAssets::ChainA(Assets::AssetA),
            Self::ChainB => ChainAssets::ChainB(Assets::AssetB),
            Self::Relay => ChainAssets::Relay(Assets::AssetA),
        }
    }
}

pub struct ExtBuilder;

impl Default for ExtBuilder {
    fn default() -> Self {
        ExtBuilder
    }
}

pub const INITIAL_BALANCE: Balance = 100000000000000000;

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();

        pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (ALICE, INITIAL_BALANCE),
                (BOB, INITIAL_BALANCE),
                (CHARLIE, INITIAL_BALANCE),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        orml_asset_registry::GenesisConfig::<Test> {
            assets: vec![
                (
                    0u32,
                    AssetMetadata {
                        decimals: 12,
                        name: sp_core::bounded_vec::BoundedVec::<u8, StringLimit>::new(),
                        symbol: sp_core::bounded_vec::BoundedVec::<u8, StringLimit>::new(),
                        existential_deposit: ExistentialDeposit::get(),
                        location: None,
                        additional: (),
                    }
                    .encode(),
                ),
                (
                    1u32,
                    AssetMetadata {
                        decimals: 12,
                        name: sp_core::bounded_vec::BoundedVec::<u8, StringLimit>::new(),
                        symbol: sp_core::bounded_vec::BoundedVec::<u8, StringLimit>::new(),
                        existential_deposit: ExistentialDeposit::get(),
                        location: None,
                        additional: (),
                    }
                    .encode(),
                ),
            ],
            last_asset_id: 1u32,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        orml_tokens::GenesisConfig::<Test> {
            balances: vec![
                (ALICE, RELAY_ASSET_ID, INITIAL_BALANCE),
                (BOB, RELAY_ASSET_ID, INITIAL_BALANCE),
                (CHARLIE, RELAY_ASSET_ID, INITIAL_BALANCE),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| System::set_block_number(0));

        ext
    }
}
