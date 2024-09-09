use codec::{Compact, Decode, Encode};
use frame_support::{
    construct_runtime, parameter_types,
    traits::{fungibles::Credit, Currency, Everything, EverythingBut, Nothing},
    weights::{constants::WEIGHT_REF_TIME_PER_SECOND, ConstantMultiplier, Weight},
};
use frame_system::EnsureRoot;
use pallet_xcm::XcmPassthrough;
use polkadot_core_primitives::BlockNumber as RelayBlockNumber;
use polkadot_parachain::primitives::{
    DmpMessageHandler, Id as ParaId, Sibling, XcmpMessageFormat, XcmpMessageHandler,
};
use scale_info::TypeInfo;
use sp_core::{blake2_256, ConstU32, H256};
use sp_runtime::{
    traits::{Hash, IdentityLookup, TryConvert},
    AccountId32,
};
use sp_std::prelude::*;
use xcm::{latest::prelude::*, VersionedXcm};
use xcm_builder::{
    AccountId32Aliases, AllowUnpaidExecutionFrom, EnsureXcmOrigin, FixedRateOfFungible,
    FixedWeightBounds, FungibleAdapter as XcmCurrencyAdapter, IsConcrete, NativeAsset,
    ParentAsSuperuser, ParentIsPreset, SiblingParachainConvertsVia, SignedAccountId32AsNative,
    SignedToAccountId32, SovereignSignedViaLocation,
};
use xcm_executor::{traits::ConvertLocation, Config, XcmExecutor};
use xcm_simulator::PhantomData;

use crate::TransactionByteFee;

pub type _SovereignAccountOf = (
    SiblingParachainConvertsVia<Sibling, AccountId>,
    AccountId32Aliases<RelayNetwork, AccountId>,
    ParentIsPreset<AccountId>,
);

pub type AccountId = AccountId32;
pub type Balance = u128;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Block = Block;
    type Hash = H256;
    type Hashing = ::sp_runtime::traits::BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeTask = RuntimeTask;
    type BlockHashCount = BlockHashCount;
    type BlockWeights = ();
    type BlockLength = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type DbWeight = ();
    type BaseCallFilter = Everything;
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

parameter_types! {
    pub ExistentialDeposit: Balance = 1;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
    type MaxLocks = MaxLocks;
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = ();
    type MaxHolds = ConstU32<0>;
    type MaxFreezes = ConstU32<0>;
    type RuntimeFreezeReason = ();
    type RuntimeHoldReason = ();
}

parameter_types! {
    pub const ReservedXcmpWeight: Weight = Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_div(4), 0);
    pub const ReservedDmpWeight: Weight = Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_div(4), 0);
}

parameter_types! {
    pub const KsmLocation: MultiLocation = MultiLocation::parent();
    pub const RelayNetwork: NetworkId = NetworkId::Kusama;
    pub UniversalLocation: InteriorMultiLocation = Parachain(MsgQueue::parachain_id().into()).into();
}

pub trait DescribeLocation {
    fn describe_location(location: &MultiLocation) -> Option<Vec<u8>>;
}

pub struct DescribeBodyTerminal;
impl DescribeLocation for DescribeBodyTerminal {
    fn describe_location(l: &MultiLocation) -> Option<Vec<u8>> {
        match (l.parents, &l.interior) {
            (0, X1(Plurality { id, part })) => Some((b"Body", id, part).encode()),
            _ => return None,
        }
    }
}

pub struct DescribeFamily<DescribeInterior>(PhantomData<DescribeInterior>);
impl<Suffix: DescribeLocation> DescribeLocation for DescribeFamily<Suffix> {
    fn describe_location(l: &MultiLocation) -> Option<Vec<u8>> {
        match (l.parents, l.interior.first()) {
            (0, Some(Parachain(index))) => {
                let tail = l.interior.split_first().0;
                let interior = Suffix::describe_location(&tail.into())?;
                Some((b"ChildChain", Compact::<u32>::from(*index), interior).encode())
            }
            (1, Some(Parachain(index))) => {
                let tail = l.interior.split_first().0;
                let interior = Suffix::describe_location(&tail.into())?;
                Some((b"SiblingChain", Compact::<u32>::from(*index), interior).encode())
            }
            (1, _) => {
                let tail = l.interior.into();
                let interior = Suffix::describe_location(&tail)?;
                Some((b"ParentChain", interior).encode())
            }
            _ => return None,
        }
    }
}

pub struct HashedDescription;
impl TryConvert<MultiLocation, AccountId> for HashedDescription {
    fn try_convert(value: MultiLocation) -> Result<AccountId, MultiLocation> {
        log::trace!(target: "xcm::HashedDescription", "HashedDescription: location: {:?}", value);
        if let Some(l) = DescribeFamily::<DescribeBodyTerminal>::describe_location(&value) {
            let a: AccountId = blake2_256(&l).into();
            log::trace!(target: "xcm::HashedDescription", "HashedDescription Ok: location: {:?} account: {:?}", value, a);
            Ok(a)
        } else {
            log::trace!(target: "xcm::HashedDescription", "HashedDescription Error");
            Err(value)
        }
    }
}

impl ConvertLocation<AccountId> for HashedDescription {
    fn convert_location(location: &MultiLocation) -> Option<AccountId> {
        if let Some(l) = DescribeFamily::<DescribeBodyTerminal>::describe_location(&location) {
            let a: AccountId = blake2_256(&l).into();
            log::trace!(target: "xcm::HashedDescription", "HashedDescription Some: location: {:?} account: {:?}", location, a);
            Some(a)
        } else {
            log::trace!(target: "xcm::HashedDescription", "HashedDescription None");
            None
        }
    }
}

pub type LocationToAccountId = (
    ParentIsPreset<AccountId>,
    SiblingParachainConvertsVia<Sibling, AccountId>,
    AccountId32Aliases<RelayNetwork, AccountId>,
    HashedDescription,
);

pub type XcmOriginToCallOrigin = (
    SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
    ParentAsSuperuser<RuntimeOrigin>,
    SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
    XcmPassthrough<RuntimeOrigin>,
);

parameter_types! {
    pub const UnitWeightCost: Weight = Weight::from_parts(1, 1);
    pub KsmPerSecondPerByte: (AssetId, u128, u128) = (Concrete(Parent.into()), 1, 1);
    pub const MaxInstructions: u32 = 100;
    pub const MaxAssetsIntoHolding: u32 = 64;
    pub ForeignPrefix: MultiLocation = (Parent,).into();
}

pub type LocalAssetTransactor =
    (XcmCurrencyAdapter<Balances, IsConcrete<KsmLocation>, LocationToAccountId, AccountId, ()>,);

pub type XcmRouter = super::ParachainXcmRouter<MsgQueue>;
pub type Barrier = AllowUnpaidExecutionFrom<Everything>;

parameter_types! {
    pub NftCollectionOne: MultiAssetFilter
        = Wild(AllOf { fun: WildNonFungible, id: Concrete((Parent, GeneralIndex(1)).into()) });
    pub NftCollectionOneForRelay: (MultiAssetFilter, MultiLocation)
        = (NftCollectionOne::get(), (Parent,).into());
}
pub type TrustedTeleporters = xcm_builder::Case<NftCollectionOneForRelay>;
pub type TrustedReserves = EverythingBut<xcm_builder::Case<NftCollectionOneForRelay>>;

pub struct XcmConfig;
impl Config for XcmConfig {
    type RuntimeCall = RuntimeCall;
    type XcmSender = XcmRouter;
    type AssetTransactor = LocalAssetTransactor;
    type OriginConverter = XcmOriginToCallOrigin;
    type IsReserve = (NativeAsset, TrustedReserves);
    type IsTeleporter = TrustedTeleporters;
    type UniversalLocation = UniversalLocation;
    type Barrier = Barrier;
    type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
    type Trader = FixedRateOfFungible<KsmPerSecondPerByte, ()>;
    type ResponseHandler = ();
    type AssetTrap = ();
    type AssetLocker = ();
    type AssetExchanger = ();
    type AssetClaims = ();
    type SubscriptionService = ();
    type PalletInstancesInfo = ();
    type FeeManager = ();
    type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
    type MessageExporter = ();
    type UniversalAliases = Nothing;
    type CallDispatcher = RuntimeCall;
    type SafeCallFilter = Everything;
    type Aliasers = ();
}

#[frame_support::pallet]
pub mod mock_msg_queue {
    use super::*;
    use frame_support::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type XcmExecutor: ExecuteXcm<Self::RuntimeCall>;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn parachain_id)]
    pub type ParachainId<T: Config> = StorageValue<_, ParaId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn received_dmp)]
    /// A queue of received DMP messages
    pub type ReceivedDmp<T: Config> = StorageValue<_, Vec<Xcm<T::RuntimeCall>>, ValueQuery>;

    impl<T: Config> Get<ParaId> for Pallet<T> {
        fn get() -> ParaId {
            Self::parachain_id()
        }
    }

    pub type MessageId = [u8; 32];

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // XCMP
        /// Some XCM was executed OK.
        Success(Option<T::Hash>),
        /// Some XCM failed.
        Fail(Option<T::Hash>, XcmError),
        /// Bad XCM version used.
        BadVersion(Option<T::Hash>),
        /// Bad XCM format used.
        BadFormat(Option<T::Hash>),

        // DMP
        /// Downward message is invalid XCM.
        InvalidFormat(MessageId),
        /// Downward message is unsupported version of XCM.
        UnsupportedVersion(MessageId),
        /// Downward message executed with the given outcome.
        ExecutedDownward(MessageId, Outcome),
    }

    impl<T: Config> Pallet<T> {
        pub fn set_para_id(para_id: ParaId) {
            ParachainId::<T>::put(para_id);
        }

        fn handle_xcmp_message(
            sender: ParaId,
            _sent_at: RelayBlockNumber,
            xcm: VersionedXcm<T::RuntimeCall>,
            max_weight: Weight,
        ) -> Result<Weight, XcmError> {
            let hash = Encode::using_encoded(&xcm, T::Hashing::hash);
            let message_hash = Encode::using_encoded(&xcm, sp_io::hashing::blake2_256);
            let (result, event) = match Xcm::<T::RuntimeCall>::try_from(xcm) {
                Ok(xcm) => {
                    let location = (Parent, Parachain(sender.into()));
                    match T::XcmExecutor::execute_xcm(location, xcm, message_hash, max_weight) {
                        Outcome::Error(e) => (Err(e.clone()), Event::Fail(Some(hash), e)),
                        Outcome::Complete(w) => (Ok(w), Event::Success(Some(hash))),
                        // As far as the caller is concerned, this was dispatched without error, so
                        // we just report the weight used.
                        Outcome::Incomplete(w, e) => (Ok(w), Event::Fail(Some(hash), e)),
                    }
                }
                Err(()) => (
                    Err(XcmError::UnhandledXcmVersion),
                    Event::BadVersion(Some(hash)),
                ),
            };
            Self::deposit_event(event);
            result
        }
    }

    impl<T: Config> XcmpMessageHandler for Pallet<T> {
        fn handle_xcmp_messages<'a, I: Iterator<Item = (ParaId, RelayBlockNumber, &'a [u8])>>(
            iter: I,
            max_weight: Weight,
        ) -> Weight {
            for (sender, sent_at, data) in iter {
                let mut data_ref = data;
                let _ = XcmpMessageFormat::decode(&mut data_ref)
                    .expect("Simulator encodes with versioned xcm format; qed");

                let mut remaining_fragments = &data_ref[..];
                while !remaining_fragments.is_empty() {
                    if let Ok(xcm) =
                        VersionedXcm::<T::RuntimeCall>::decode(&mut remaining_fragments)
                    {
                        let _ = Self::handle_xcmp_message(sender, sent_at, xcm, max_weight);
                    } else {
                        debug_assert!(false, "Invalid incoming XCMP message data");
                    }
                }
            }
            max_weight
        }
    }

    impl<T: Config> DmpMessageHandler for Pallet<T> {
        fn handle_dmp_messages(
            iter: impl Iterator<Item = (RelayBlockNumber, Vec<u8>)>,
            limit: Weight,
        ) -> Weight {
            for (_i, (_sent_at, data)) in iter.enumerate() {
                let id = sp_io::hashing::blake2_256(&data[..]);
                let maybe_versioned = VersionedXcm::<T::RuntimeCall>::decode(&mut &data[..]);
                match maybe_versioned {
                    Err(_) => {
                        Self::deposit_event(Event::InvalidFormat(id));
                    }
                    Ok(versioned) => match Xcm::try_from(versioned) {
                        Err(()) => Self::deposit_event(Event::UnsupportedVersion(id)),
                        Ok(x) => {
                            let outcome = T::XcmExecutor::execute_xcm(Parent, x.clone(), id, limit);
                            <ReceivedDmp<T>>::append(x);
                            Self::deposit_event(Event::ExecutedDownward(id, outcome));
                        }
                    },
                }
            }
            limit
        }
    }
}

impl mock_msg_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type XcmExecutor = XcmExecutor<XcmConfig>;
}

pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
    pub ReachableDest: Option<MultiLocation> = Some(Parent.into());
}

impl pallet_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmRouter = XcmRouter;
    type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmExecuteFilter = Everything;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmTeleportFilter = Nothing;
    type XcmReserveTransferFilter = Everything;
    type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
    type UniversalLocation = UniversalLocation;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    type Currency = Balances;
    type CurrencyMatcher = ();
    type TrustedLockers = ();
    type SovereignAccountOf = LocationToAccountId;
    type MaxLockers = ConstU32<8>;
    type MaxRemoteLockConsumers = ConstU32<0>;
    type RemoteLockConsumerIdentifier = ();
    type WeightInfo = pallet_xcm::TestWeightInfo;
    type AdminOrigin = EnsureRoot<AccountId>;
}

type _UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo, Debug)]
pub struct FeeCharger;

impl pallet_dao_manager::fee_handling::MultisigFeeHandler<Runtime> for FeeCharger {
    type Pre = ();

    fn pre_dispatch(
        _fee_asset: &pallet_dao_manager::fee_handling::FeeAsset,
        _who: &AccountId,
        _call: &RuntimeCall,
        _info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        _len: usize,
    ) -> Result<Self::Pre, frame_support::unsigned::TransactionValidityError> {
        Ok(())
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
            Credit<AccountId, Tokens>,
        >,
    ) {
    }
}

parameter_types! {
    pub PID: u32 = MsgQueue::parachain_id().into();
    pub const RelayAssetId: u32 = 1;
}

impl pallet_dao_manager::Config for Runtime {
    type MaxMetadata = crate::dao_manager::MaxMetadata;
    type DaoId = crate::common_types::CommonId;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type RuntimeCall = RuntimeCall;
    type MaxCallers = crate::dao_manager::MaxCallers;
    type DaoSeedBalance = crate::dao_manager::DaoSeedBalance;
    type AssetsProvider = CoreAssets;
    type RuntimeOrigin = RuntimeOrigin;
    type DaoCreationFee = crate::dao_manager::DaoCreationFee;
    type FeeCharger = FeeCharger;
    type WeightInfo = pallet_dao_manager::weights::SubstrateWeight<Runtime>;

    type Tokens = Tokens;
    type RelayAssetId = RelayAssetId;
    type RelayDaoCreationFee = crate::dao_manager::KSMCoreCreationFee;

    type MaxCallSize = crate::dao_manager::MaxCallSize;

    type ParaId = PID;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
}

impl orml_tokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = i128;
    type CurrencyId = u32;
    type WeightInfo = ();
    type ExistentialDeposits = crate::assets::ExistentialDeposits;
    type MaxLocks = MaxLocks;
    type DustRemovalWhitelist = crate::assets::DustRemovalWhitelist;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type CurrencyHooks = ();
}

impl orml_tokens2::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = i128;
    type CurrencyId = <Runtime as pallet_dao_manager::Config>::DaoId;
    type WeightInfo = ();
    type ExistentialDeposits = crate::dao_manager::DaoExistentialDeposits;
    type MaxLocks = ConstU32<0u32>;
    type MaxReserves = ConstU32<0u32>;
    type DustRemovalWhitelist = crate::dao_manager::DaoDustRemovalWhitelist;
    type ReserveIdentifier = [u8; 8];
    type CurrencyHooks = ();
}

impl pallet_rings::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Chains = crate::rings::Chains;
    type MaxXCMCallLength = crate::rings::MaxXCMCallLength;
    type MaintenanceOrigin = EnsureRoot<AccountId>;
    type WeightInfo = pallet_rings::weights::SubstrateWeight<Runtime>;
}

construct_runtime!(
    pub enum Runtime
    {
        System: frame_system,
        Balances: pallet_balances,
        MsgQueue: mock_msg_queue,
        PolkadotXcm: pallet_xcm,
        INV4: pallet_dao_manager,
        Tokens: orml_tokens,
        CoreAssets: orml_tokens2,
        Rings: pallet_rings,
    }
);
