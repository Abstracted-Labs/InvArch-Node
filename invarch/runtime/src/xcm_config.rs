use super::{
    AccountId, AllPalletsWithSystem, Balances, ConstU32, MessageQueue, ParachainInfo,
    ParachainSystem, PolkadotXcm, Runtime, RuntimeBlockWeights, RuntimeCall, RuntimeEvent,
    RuntimeOrigin, Weight, WeightToFee, XcmpQueue,
};
use cumulus_primitives_core::{AggregateMessageOrigin, ParaId};
use frame_support::{
    match_types, parameter_types,
    traits::{EnqueueWithOrigin, Everything, Nothing, TransformOrigin},
};
use frame_system::EnsureRoot;
use pallet_dao_staking::primitives::{
    CustomAggregateMessageOrigin, CustomMessageProcessor, CustomNarrowOriginToSibling,
    CustomParaIdToSibling,
};
use pallet_xcm::XcmPassthrough;
use polkadot_parachain::primitives::Sibling;
use polkadot_runtime_common::{impls::ToAuthor, xcm_sender::NoPriceForMessageDelivery};
use sp_runtime::Perbill;
use xcm::latest::prelude::*;
use xcm_builder::{
    AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom,
    AllowTopLevelPaidExecutionFrom, AllowUnpaidExecutionFrom, EnsureXcmOrigin, FixedWeightBounds,
    FungibleAdapter, IsConcrete, NativeAsset, ParentIsPreset, RelayChainAsNative,
    SiblingParachainAsNative, SiblingParachainConvertsVia, SignedAccountId32AsNative,
    SignedToAccountId32, SovereignSignedViaLocation, TakeWeightCredit, UsingComponents,
};
use xcm_executor::XcmExecutor;

parameter_types! {
    pub const RelayLocation: MultiLocation = MultiLocation::parent();
    pub const RelayNetwork: NetworkId = NetworkId::Polkadot;
    pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
    pub Ancestry: MultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
    pub UniversalLocation: InteriorMultiLocation = X2(GlobalConsensus(RelayNetwork::get()), Parachain(ParachainInfo::parachain_id().into()));
    pub const RelayAggregate: CustomAggregateMessageOrigin<AggregateMessageOrigin> = CustomAggregateMessageOrigin::Aggregate(AggregateMessageOrigin::Parent);
}

/// Type for specifying how a `MultiLocation` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
    // The parent (Relay-chain) origin converts to the default `AccountId`.
    ParentIsPreset<AccountId>,
    // Sibling parachain origins convert to AccountId via the `ParaId::into`.
    SiblingParachainConvertsVia<Sibling, AccountId>,
    // Straight up local `AccountId32` origins just alias directly to `AccountId`.
    AccountId32Aliases<RelayNetwork, AccountId>,
);

/// Means for transacting assets on this chain.
pub type LocalAssetTransactor = FungibleAdapter<
    // Use this currency:
    Balances,
    // Use this currency when it is a fungible asset matching the given location or name:
    IsConcrete<RelayLocation>,
    // Do a simple punn to convert an AccountId32 MultiLocation into a native chain account ID:
    LocationToAccountId,
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // We don't track any teleports.
    (),
>;

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// biases the kind of local `Origin` it will become.
pub type XcmOriginToTransactDispatchOrigin = (
    // Sovereign account converter; this attempts to derive an `AccountId` from the origin location
    // using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
    // foreign chains who want to have a local sovereign account on this chain which they control.
    SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
    // Native converter for Relay-chain (Parent) location; will converts to a `Relay` origin when
    // recognized.
    RelayChainAsNative<RelayChainOrigin, RuntimeOrigin>,
    // Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
    // recognized.
    SiblingParachainAsNative<cumulus_pallet_xcm::Origin, RuntimeOrigin>,
    // Native signed account converter; this just converts an `AccountId32` origin into a normal
    // `Origin::Signed` origin of the same 32-byte value.
    SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
    // Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
    XcmPassthrough<RuntimeOrigin>,
);

parameter_types! {
    pub const BaseXcmWeight: Weight = Weight::from_parts(100_000_000, 0);
    pub const MaxInstructions: u32 = 100;
    pub const MaxAssetsIntoHolding: u32 = 8;
}

match_types! {
    pub type ParentOrParentsPlurality: impl Contains<MultiLocation> = {
        MultiLocation { parents: 1, interior: Here } |
        MultiLocation { parents: 1, interior: X1(Plurality { .. }) }
    };
}

pub type Barrier = (
    TakeWeightCredit,
    AllowTopLevelPaidExecutionFrom<Everything>,
    // Parent and its plurality get free execution
    AllowUnpaidExecutionFrom<ParentOrParentsPlurality>,
    // Expected responses are OK.
    AllowKnownQueryResponses<PolkadotXcm>,
    // Subscriptions for version tracking are OK.
    AllowSubscriptionsFrom<Everything>,
);

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
    type RuntimeCall = RuntimeCall;
    type XcmSender = XcmRouter;
    // How to withdraw and deposit an asset.
    type AssetTransactor = LocalAssetTransactor;
    type OriginConverter = XcmOriginToTransactDispatchOrigin;
    type IsReserve = NativeAsset;
    type IsTeleporter = (); // Teleporting is disabled.
    type Barrier = Barrier;
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type Trader =
        UsingComponents<WeightToFee, RelayLocation, AccountId, Balances, ToAuthor<Runtime>>;
    type ResponseHandler = PolkadotXcm;
    type AssetTrap = PolkadotXcm;
    type AssetClaims = PolkadotXcm;
    type SubscriptionService = PolkadotXcm;
    type UniversalLocation = UniversalLocation;
    type AssetLocker = ();
    type AssetExchanger = ();
    type FeeManager = ();
    type MessageExporter = ();
    type PalletInstancesInfo = AllPalletsWithSystem;
    type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
    type UniversalAliases = Nothing;
    type CallDispatcher = RuntimeCall;
    type SafeCallFilter = ();
    type Aliasers = ();
}

/// No local origins on this chain are allowed to dispatch XCM sends/executions.
pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = (
    // Two routers - use UMP to communicate with the relay chain:
    cumulus_primitives_utility::ParentAsUmp<ParachainSystem, PolkadotXcm, ()>,
    // ..and XCMP to communicate with the sibling chains.
    XcmpQueue,
);

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
    pub ReachableDest: Option<MultiLocation> = Some(MultiLocation::parent());
}

parameter_types! {
    pub const MaxLockers: u32 = 8;
    pub const MaxRemoteLockConsumers: u32 = 0;
}

impl pallet_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmRouter = XcmRouter;
    type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmExecuteFilter = Everything;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmTeleportFilter = Everything;
    type XcmReserveTransferFilter = Nothing;
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;

    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
    // ^ Override for AdvertisedXcmVersion default
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;

    type UniversalLocation = UniversalLocation;
    type Currency = Balances;
    type CurrencyMatcher = ();
    type TrustedLockers = ();
    type SovereignAccountOf = ();
    type MaxLockers = MaxLockers;
    type WeightInfo = pallet_xcm::TestWeightInfo;
    type AdminOrigin = EnsureRoot<AccountId>;
    type MaxRemoteLockConsumers = MaxRemoteLockConsumers;
    type RemoteLockConsumerIdentifier = ();

    #[cfg(feature = "runtime-benchmarks")]
    type ReachableDest = ReachableDest;
}

impl cumulus_pallet_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl orml_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SovereignOrigin = EnsureRoot<AccountId>;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ChannelInfo = ParachainSystem;
    type VersionWrapper = ();
    type ControllerOrigin = EnsureRoot<AccountId>;
    type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
    type PriceForSiblingDelivery = NoPriceForMessageDelivery<ParaId>;
    type WeightInfo = ();
    type XcmpQueue = TransformOrigin<
        MessageQueue,
        CustomAggregateMessageOrigin<AggregateMessageOrigin>,
        ParaId,
        CustomParaIdToSibling,
    >;
    type MaxInboundSuspended = ConstU32<1000>;
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DmpSink = EnqueueWithOrigin<MessageQueue, RelayAggregate>;
    type WeightInfo = cumulus_pallet_dmp_queue::weights::SubstrateWeight<Self>;
}

parameter_types! {
    pub MessageQueueServiceWeight: Weight = Perbill::from_percent(35) * RuntimeBlockWeights::get().max_block;
    pub const MessageQueueMaxStale: u32 = 8;
    pub const MessageQueueHeapSize: u32 = 128 * 1048;
}

impl pallet_message_queue::Config for Runtime {
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
        xcm_builder::ProcessXcmMessage<
            AggregateMessageOrigin,
            xcm_executor::XcmExecutor<XcmConfig>,
            RuntimeCall,
        >,
        RuntimeCall,
        Runtime,
    >;
    type Size = u32;
    type QueueChangeHandler = CustomNarrowOriginToSibling<XcmpQueue, Runtime>;
    type QueuePausedQuery = CustomNarrowOriginToSibling<XcmpQueue, Runtime>;
    type HeapSize = MessageQueueHeapSize;
    type MaxStale = MessageQueueMaxStale;
    type ServiceWeight = MessageQueueServiceWeight;
}
