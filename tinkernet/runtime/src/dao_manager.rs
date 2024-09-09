use crate::{
    assets::{RelayAssetId, KSM_ASSET_ID},
    common_types::{AssetId, CommonId},
    constants::currency::UNIT,
    fee_handling::DealWithKSMFees,
    AccountId, Balance, Balances, CoreAssets, DealWithFees, ParachainInfo, Runtime, RuntimeCall,
    RuntimeEvent, RuntimeOrigin, Tokens, TransactionByteFee,
};
use codec::{Decode, Encode};
use frame_support::{
    parameter_types,
    traits::{fungibles::Credit, Contains, Currency, OnUnbalanced},
    weights::ConstantMultiplier,
};
use pallet_asset_tx_payment::ChargeAssetTxPayment;
use pallet_dao_manager::fee_handling::{FeeAsset, FeeAssetNegativeImbalance, MultisigFeeHandler};
use scale_info::TypeInfo;
use sp_core::ConstU32;
use sp_runtime::traits::{One, SignedExtension, Zero};

parameter_types! {
    pub const MaxMetadata: u32 = 10000;
    pub const MaxCallers: u32 = 10000;
    pub const DaoSeedBalance: Balance = 1000000u128;
    pub const DaoCreationFee: Balance = UNIT * 100;

    pub const KSMCoreCreationFee: Balance = UNIT;
    pub const MaxCallSize: u32 = 50 * 1024;

    pub ParaId: u32 = ParachainInfo::parachain_id().into();
}

impl pallet_dao_manager::Config for Runtime {
    type MaxMetadata = MaxMetadata;
    type DaoId = CommonId;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type RuntimeCall = RuntimeCall;
    type MaxCallers = MaxCallers;
    type DaoSeedBalance = DaoSeedBalance;
    type AssetsProvider = CoreAssets;
    type RuntimeOrigin = RuntimeOrigin;
    type DaoCreationFee = DaoCreationFee;
    type FeeCharger = FeeCharger;
    type WeightInfo = pallet_dao_manager::weights::SubstrateWeight<Runtime>;

    type Tokens = Tokens;
    type RelayAssetId = RelayAssetId;
    type RelayDaoCreationFee = KSMCoreCreationFee;

    type MaxCallSize = MaxCallSize;

    type ParaId = ParaId;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo, Debug)]
pub struct FeeCharger;

impl MultisigFeeHandler<Runtime> for FeeCharger {
    type Pre = (
        // tip
        Balance,
        // who paid the fee
        AccountId,
        // imbalance resulting from withdrawing the fee
        pallet_asset_tx_payment::InitialPayment<Runtime>,
        // asset_id for the transaction payment
        Option<AssetId>,
    );

    fn pre_dispatch(
        fee_asset: &FeeAsset,
        who: &AccountId,
        call: &RuntimeCall,
        info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Result<Self::Pre, frame_support::unsigned::TransactionValidityError> {
        match fee_asset {
            FeeAsset::Native => ChargeAssetTxPayment::<Runtime>::from(Zero::zero(), None)
                .pre_dispatch(who, call, info, len),

            FeeAsset::Relay => {
                ChargeAssetTxPayment::<Runtime>::from(Zero::zero(), Some(KSM_ASSET_ID))
                    .pre_dispatch(who, call, info, len)
            }
        }
    }

    fn post_dispatch(
        fee_asset: &FeeAsset,
        pre: Option<Self::Pre>,
        info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        post_info: &sp_runtime::traits::PostDispatchInfoOf<RuntimeCall>,
        len: usize,
        result: &sp_runtime::DispatchResult,
    ) -> Result<(), frame_support::unsigned::TransactionValidityError> {
        match fee_asset {
            FeeAsset::Native => {
                ChargeAssetTxPayment::<Runtime>::post_dispatch(pre, info, post_info, len, result)
            }

            FeeAsset::Relay => {
                ChargeAssetTxPayment::<Runtime>::post_dispatch(pre, info, post_info, len, result)
            }
        }
    }

    fn handle_creation_fee(
        imbalance: FeeAssetNegativeImbalance<
            <Balances as Currency<AccountId>>::NegativeImbalance,
            Credit<AccountId, Tokens>,
        >,
    ) {
        match imbalance {
            FeeAssetNegativeImbalance::Native(imb) => DealWithFees::on_unbalanced(imb),

            FeeAssetNegativeImbalance::Relay(imb) => DealWithKSMFees::on_unbalanced(imb),
        }
    }
}

orml_traits2::parameter_type_with_key! {
    pub DaoExistentialDeposits: |_currency_id: <Runtime as pallet_dao_manager::Config>::DaoId| -> Balance {
        Balance::one()
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
        <Runtime as pallet_dao_manager::Config>::DaoId,
        Balance,
    > for DisallowIfFrozen
{
    fn on_transfer(
        currency_id: <Runtime as pallet_dao_manager::Config>::DaoId,
        _from: &AccountId,
        _to: &AccountId,
        _amount: Balance,
    ) -> sp_runtime::DispatchResult {
        if let Some(true) = crate::INV4::is_asset_frozen(currency_id) {
            Err(sp_runtime::DispatchError::Token(
                sp_runtime::TokenError::Frozen,
            ))
        } else {
            Ok(())
        }
    }
}

pub struct HandleNewMembers;
impl orml_traits2::Happened<(AccountId, <Runtime as pallet_dao_manager::Config>::DaoId)>
    for HandleNewMembers
{
    fn happened((member, dao_id): &(AccountId, <Runtime as pallet_dao_manager::Config>::DaoId)) {
        crate::INV4::add_member(dao_id, member)
    }
}

pub struct HandleRemovedMembers;
impl orml_traits2::Happened<(AccountId, <Runtime as pallet_dao_manager::Config>::DaoId)>
    for HandleRemovedMembers
{
    fn happened((member, dao_id): &(AccountId, <Runtime as pallet_dao_manager::Config>::DaoId)) {
        crate::INV4::remove_member(dao_id, member)
    }
}

pub struct INV4TokenHooks;
impl
    orml_traits2::currency::MutationHooks<
        AccountId,
        <Runtime as pallet_dao_manager::Config>::DaoId,
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

impl orml_tokens2::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = i128;
    type CurrencyId = <Runtime as pallet_dao_manager::Config>::DaoId;
    type WeightInfo = ();
    type ExistentialDeposits = DaoExistentialDeposits;
    type MaxLocks = ConstU32<0u32>;
    type MaxReserves = ConstU32<0u32>;
    type DustRemovalWhitelist = DaoDustRemovalWhitelist;
    type ReserveIdentifier = [u8; 8];
    type CurrencyHooks = INV4TokenHooks;
}
