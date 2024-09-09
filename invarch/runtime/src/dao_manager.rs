use crate::{
    balances::DealWithFees, common_types::CommonId, AccountId, Balance, Balances, CoreAssets,
    ParachainInfo, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, TransactionByteFee, UNIT,
};
use codec::{Decode, Encode};
use frame_support::{
    parameter_types,
    traits::{
        fungibles::{Balanced, Credit, Inspect, Unbalanced},
        Contains, Currency, OnUnbalanced,
    },
    weights::ConstantMultiplier,
};
use pallet_dao_manager::fee_handling::{FeeAsset, FeeAssetNegativeImbalance, MultisigFeeHandler};
use pallet_transaction_payment::ChargeTransactionPayment;
use scale_info::TypeInfo;
use sp_core::ConstU32;
use sp_runtime::traits::{One, SignedExtension, Zero};

parameter_types! {
    pub const MaxMetadata: u32 = 10000;
    pub const MaxCallers: u32 = 10000;
    pub const DaoSeedBalance: Balance = 1000000u128;
    pub const DaoCreationFee: Balance = UNIT * 1000;

    pub const RelayDaoCreationFee: Balance = UNIT;
    pub const MaxCallSize: u32 = 50 * 1024;

    pub ParaId: u32 = ParachainInfo::parachain_id().into();

    pub const NoId: () = ();
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

    type Tokens = NoTokens;
    type RelayAssetId = NoId;
    type RelayDaoCreationFee = RelayDaoCreationFee;

    type MaxCallSize = MaxCallSize;

    type ParaId = ParaId;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
}

pub struct NoTokens;

impl Inspect<AccountId> for NoTokens {
    type AssetId = ();
    type Balance = u128;

    fn total_issuance(_asset: Self::AssetId) -> Self::Balance {
        Zero::zero()
    }

    fn minimum_balance(_asset: Self::AssetId) -> Self::Balance {
        Zero::zero()
    }

    fn total_balance(_asset: Self::AssetId, _who: &AccountId) -> Self::Balance {
        Zero::zero()
    }

    fn balance(_asset: Self::AssetId, _who: &AccountId) -> Self::Balance {
        Zero::zero()
    }

    fn reducible_balance(
        _asset: Self::AssetId,
        _who: &AccountId,
        _preservation: frame_support::traits::tokens::Preservation,
        _force: frame_support::traits::tokens::Fortitude,
    ) -> Self::Balance {
        Zero::zero()
    }

    fn can_deposit(
        _asset: Self::AssetId,
        _who: &AccountId,
        _amount: Self::Balance,
        _provenance: frame_support::traits::tokens::Provenance,
    ) -> frame_support::traits::tokens::DepositConsequence {
        frame_support::traits::tokens::DepositConsequence::UnknownAsset
    }

    fn can_withdraw(
        _asset: Self::AssetId,
        _who: &AccountId,
        _amount: Self::Balance,
    ) -> frame_support::traits::tokens::WithdrawConsequence<Self::Balance> {
        frame_support::traits::tokens::WithdrawConsequence::UnknownAsset
    }

    fn asset_exists(_asset: Self::AssetId) -> bool {
        false
    }

    fn active_issuance(_asset: Self::AssetId) -> Self::Balance {
        Zero::zero()
    }
}

impl Unbalanced<AccountId> for NoTokens {
    fn handle_dust(_dust: frame_support::traits::fungibles::Dust<AccountId, Self>) {}

    fn write_balance(
        _asset: Self::AssetId,
        _who: &AccountId,
        _amount: Self::Balance,
    ) -> Result<Option<Self::Balance>, sp_runtime::DispatchError> {
        Err(sp_runtime::DispatchError::Token(
            sp_runtime::TokenError::UnknownAsset,
        ))
    }

    fn set_total_issuance(_asset: Self::AssetId, _amount: Self::Balance) {}
}

pub struct NoHandle;
impl frame_support::traits::tokens::fungibles::HandleImbalanceDrop<(), u128> for NoHandle {
    fn handle(_asset: (), _amount: u128) {}
}

impl Balanced<AccountId> for NoTokens {
    type OnDropCredit = NoHandle;
    type OnDropDebt = NoHandle;
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo, Debug)]
pub struct FeeCharger;

impl MultisigFeeHandler<Runtime> for FeeCharger {
    type Pre =
        <pallet_transaction_payment::ChargeTransactionPayment<Runtime> as SignedExtension>::Pre;

    fn pre_dispatch(
        fee_asset: &FeeAsset,
        who: &AccountId,
        call: &RuntimeCall,
        info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Result<Self::Pre, frame_support::unsigned::TransactionValidityError> {
        match fee_asset {
            FeeAsset::Native => ChargeTransactionPayment::<Runtime>::from(Zero::zero())
                .pre_dispatch(who, call, info, len),

            FeeAsset::Relay => Err(frame_support::unsigned::TransactionValidityError::Invalid(
                sp_runtime::transaction_validity::InvalidTransaction::Payment,
            )),
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
            FeeAsset::Native => ChargeTransactionPayment::<Runtime>::post_dispatch(
                pre, info, post_info, len, result,
            ),

            FeeAsset::Relay => Err(frame_support::unsigned::TransactionValidityError::Invalid(
                sp_runtime::transaction_validity::InvalidTransaction::Payment,
            )),
        }
    }

    fn handle_creation_fee(
        imbalance: FeeAssetNegativeImbalance<
            <Balances as Currency<AccountId>>::NegativeImbalance,
            Credit<AccountId, NoTokens>,
        >,
    ) {
        match imbalance {
            FeeAssetNegativeImbalance::Native(imb) => DealWithFees::on_unbalanced(imb),

            FeeAssetNegativeImbalance::Relay(_) => {}
        }
    }
}

orml_traits::parameter_type_with_key! {
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
    orml_traits::currency::OnTransfer<
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
impl orml_traits::Happened<(AccountId, <Runtime as pallet_dao_manager::Config>::DaoId)>
    for HandleNewMembers
{
    fn happened((member, dao_id): &(AccountId, <Runtime as pallet_dao_manager::Config>::DaoId)) {
        crate::INV4::add_member(dao_id, member)
    }
}

pub struct HandleRemovedMembers;
impl orml_traits::Happened<(AccountId, <Runtime as pallet_dao_manager::Config>::DaoId)>
    for HandleRemovedMembers
{
    fn happened((member, dao_id): &(AccountId, <Runtime as pallet_dao_manager::Config>::DaoId)) {
        crate::INV4::remove_member(dao_id, member)
    }
}

pub struct INV4TokenHooks;
impl
    orml_traits::currency::MutationHooks<
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

impl orml_tokens::Config for Runtime {
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
