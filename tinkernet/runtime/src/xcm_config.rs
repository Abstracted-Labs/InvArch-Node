use super::{
    AccountId, Balance, Balances, MessageQueue, ParachainInfo, ParachainSystem, PolkadotXcm,
    Runtime, RuntimeBlockWeights, RuntimeCall, RuntimeEvent, RuntimeOrigin, WeightToFee, XcmpQueue,
};
use crate::{
    assets::CORE_ASSET_ID, common_types::AssetId, constants::TreasuryAccount, AllPalletsWithSystem,
    AssetRegistry, Currencies, DealWithFees, UnknownTokens, Weight,
};
use codec::{Decode, Encode};
use cumulus_primitives_core::{AggregateMessageOrigin, ParaId};
use frame_support::{
    match_types, parameter_types,
    traits::{EnqueueWithOrigin, Everything, Get, Nothing, TransformOrigin},
};
use frame_system::EnsureRoot;
use orml_asset_registry::{AssetRegistryTrader, FixedRateAssetRegistryTrader};
use orml_traits::{
    location::AbsoluteReserveProvider, parameter_type_with_key, FixedConversionRateProvider,
    MultiCurrency,
};
pub use orml_xcm_support::{
    DepositToAlternative, IsNativeConcrete, MultiCurrencyAdapter, MultiNativeAsset,
};
use pallet_dao_staking::primitives::{
    CustomAggregateMessageOrigin, CustomMessageProcessor, CustomNarrowOriginToSibling,
    CustomParaIdToSibling,
};
use pallet_xcm::XcmPassthrough;
use polkadot_parachain::primitives::Sibling;
use polkadot_runtime_common::xcm_sender::NoPriceForMessageDelivery;
use scale_info::TypeInfo;
use sp_core::ConstU32;
use sp_runtime::{traits::Convert, Perbill};
use xcm::latest::prelude::*;
use xcm_builder::{
    AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom,
    AllowTopLevelPaidExecutionFrom, AllowUnpaidExecutionFrom, EnsureXcmOrigin, FixedWeightBounds,
    ParentIsPreset, RelayChainAsNative, SiblingParachainAsNative, SiblingParachainConvertsVia,
    SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation, TakeRevenue,
    TakeWeightCredit, UsingComponents,
};
use xcm_executor::XcmExecutor;
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
pub struct AssetLocation(pub MultiLocation);

pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

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

parameter_types! {
    pub SelfLocation: MultiLocation = MultiLocation::new(1, X1(Parachain(ParachainInfo::get().into())));
}

parameter_types! {
    pub const RelayNetwork: NetworkId = NetworkId::Kusama;
    pub const RelayLocation: MultiLocation = MultiLocation::parent();
    pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
    pub Ancestry: MultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
    pub UniversalLocation: InteriorMultiLocation = X2(GlobalConsensus(RelayNetwork::get()), Parachain(ParachainInfo::parachain_id().into()));
    pub const RelayAggregate: CustomAggregateMessageOrigin<AggregateMessageOrigin> = CustomAggregateMessageOrigin::Aggregate(AggregateMessageOrigin::Parent);
}

match_types! {
    pub type ParentOrParentsPlurality: impl Contains<MultiLocation> = {
        MultiLocation { parents: 1, interior: Here } |
        MultiLocation { parents: 1, interior: X1(Plurality { .. }) }
    };
}

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
    /// The amount of weight an XCM operation takes. This is a safe overestimate.
    pub const BaseXcmWeight: Weight = Weight::from_parts(100_000_000, 0);
    pub const MaxInstructions: u32 = 100;
    pub const MaxAssetsForTransfer: usize = 2;
}

pub struct CurrencyIdConvert;

impl Convert<AssetId, Option<MultiLocation>> for CurrencyIdConvert {
    fn convert(id: AssetId) -> Option<MultiLocation> {
        match id {
            CORE_ASSET_ID => Some(MultiLocation::new(
                1,
                X2(
                    Parachain(ParachainInfo::get().into()),
                    GeneralIndex(id.into()),
                ),
            )),
            _ => AssetRegistry::multilocation(&id).unwrap_or_default(),
        }
    }
}

impl Convert<MultiLocation, Option<AssetId>> for CurrencyIdConvert {
    fn convert(location: MultiLocation) -> Option<AssetId> {
        match location {
            MultiLocation {
                parents,
                interior: X2(Parachain(id), GeneralIndex(index)),
            } if parents == 1
                && ParaId::from(id) == ParachainInfo::get()
                && (index as u32) == CORE_ASSET_ID =>
            {
                // Handling native asset for this parachain
                Some(CORE_ASSET_ID)
            }
            // handle reanchor canonical location: https://github.com/paritytech/polkadot/pull/4470
            MultiLocation {
                parents: 0,
                interior: X1(GeneralIndex(index)),
            } if (index as u32) == CORE_ASSET_ID => Some(CORE_ASSET_ID),
            // delegate to asset-registry
            _ => AssetRegistry::location_to_asset_id(location),
        }
    }
}

impl Convert<MultiAsset, Option<AssetId>> for CurrencyIdConvert {
    fn convert(asset: MultiAsset) -> Option<AssetId> {
        if let MultiAsset {
            id: Concrete(location),
            ..
        } = asset
        {
            Self::convert(location)
        } else {
            None
        }
    }
}

pub struct AccountIdToMultiLocation;
impl Convert<AccountId, MultiLocation> for AccountIdToMultiLocation {
    fn convert(account: AccountId) -> MultiLocation {
        X1(AccountId32 {
            network: None,
            id: account.into(),
        })
        .into()
    }
}

parameter_types! {
    pub TNKRMultiLocation: MultiLocation = MultiLocation::new(0, X1(GeneralIndex(CORE_ASSET_ID.into())));
}

pub struct FeePerSecondProvider;
impl FixedConversionRateProvider for FeePerSecondProvider {
    fn get_fee_per_second(location: &MultiLocation) -> Option<u128> {
        AssetRegistry::fetch_metadata_by_location(location)?
            .additional
            .fee_per_second
    }
}

pub struct ToTreasury;
impl TakeRevenue for ToTreasury {
    fn take_revenue(revenue: MultiAsset) {
        if let MultiAsset {
            id: xcm::latest::AssetId::Concrete(id),
            fun: Fungibility::Fungible(amount),
        } = revenue
        {
            if let Some(currency_id) = CurrencyIdConvert::convert(id) {
                let _ = Currencies::deposit(currency_id, &TreasuryAccount::get(), amount);
            }
        }
    }
}

pub type Trader = (
    UsingComponents<WeightToFee, TNKRMultiLocation, AccountId, Balances, DealWithFees>,
    AssetRegistryTrader<FixedRateAssetRegistryTrader<FeePerSecondProvider>, ToTreasury>,
);

pub struct XcmConfig;

impl xcm_executor::Config for XcmConfig {
    type RuntimeCall = RuntimeCall;
    type XcmSender = XcmRouter;
    // How to withdraw and deposit an asset.
    type AssetTransactor = LocalAssetTransactor;
    type OriginConverter = XcmOriginToTransactDispatchOrigin;
    type IsReserve = MultiNativeAsset<AbsoluteReserveProvider>;
    type IsTeleporter = (); // disabled
    type Barrier = Barrier;
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type Trader = Trader;
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
    type MaxAssetsIntoHolding = ConstU32<8>;
    type UniversalAliases = Nothing;
    type CallDispatcher = RuntimeCall;
    type SafeCallFilter = ();
    type Aliasers = ();
}

impl cumulus_pallet_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ChannelInfo = ParachainSystem;
    type VersionWrapper = PolkadotXcm;
    type ControllerOrigin = EnsureRoot<AccountId>;
    type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
    type WeightInfo = cumulus_pallet_xcmp_queue::weights::SubstrateWeight<Runtime>;
    type PriceForSiblingDelivery = NoPriceForMessageDelivery<ParaId>;
    type XcmpQueue = TransformOrigin<
        MessageQueue,
        CustomAggregateMessageOrigin<AggregateMessageOrigin>,
        ParaId,
        CustomParaIdToSibling,
    >;
    type MaxInboundSuspended = ConstU32<1000>;
}

// Deprecated
impl cumulus_pallet_dmp_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DmpSink = EnqueueWithOrigin<MessageQueue, RelayAggregate>;
    type WeightInfo = cumulus_pallet_dmp_queue::weights::SubstrateWeight<Self>;
}

parameter_type_with_key! {
      pub ParachainMinFee: |_location: MultiLocation| -> Option<u128> {
            None
      };
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

impl orml_xtokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type CurrencyId = AssetId;
    type CurrencyIdConvert = CurrencyIdConvert;
    type AccountIdToMultiLocation = AccountIdToMultiLocation;
    type SelfLocation = SelfLocation;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type BaseXcmWeight = BaseXcmWeight;
    type MaxAssetsForTransfer = MaxAssetsForTransfer;
    type MinXcmFee = ParachainMinFee;
    type MultiLocationsFilter = Everything;
    type ReserveProvider = AbsoluteReserveProvider;

    type UniversalLocation = UniversalLocation;
}

impl orml_unknown_tokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl orml_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SovereignOrigin = EnsureRoot<AccountId>;
}

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
      pub ReachableDest: Option<MultiLocation> = Some(MultiLocation::parent());
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
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;

    type UniversalLocation = UniversalLocation;
    type Currency = Balances;
    type CurrencyMatcher = ();
    type TrustedLockers = ();
    type SovereignAccountOf = ();
    type MaxLockers = ConstU32<8>;
    type WeightInfo = pallet_xcm::TestWeightInfo;
    type AdminOrigin = EnsureRoot<AccountId>;
    type MaxRemoteLockConsumers = ConstU32<0>;
    type RemoteLockConsumerIdentifier = ();
}

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = (
    // Two routers - use UMP to communicate with the relay chain:
    cumulus_primitives_utility::ParentAsUmp<ParachainSystem, PolkadotXcm, ()>,
    // ..and XCMP to communicate with the sibling chains.
    XcmpQueue,
);

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

pub type LocalAssetTransactor = MultiCurrencyAdapter<
    Currencies,
    UnknownTokens,
    IsNativeConcrete<AssetId, CurrencyIdConvert>,
    AccountId,
    LocationToAccountId,
    AssetId,
    CurrencyIdConvert,
    DepositToAlternative<TreasuryAccount, Currencies, AssetId, AccountId, Balance>,
>;
