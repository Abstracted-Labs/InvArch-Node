use crate::{
    assets::RelayAssetId,
    common_types::AssetId,
    constants::{StakingPotAccount, TreasuryAccount},
    AccountId, Balance, Runtime, RuntimeCall, RuntimeEvent, Tokens,
};
use codec::{Decode, Encode};
use frame_support::traits::{
    fungible::Inspect as FungibleInspect,
    fungibles::{Balanced, Credit},
    tokens::{Fortitude, Precision, Preservation, WithdrawConsequence},
    Contains, OnUnbalanced,
};
use orml_tokens::CurrencyAdapter;
use pallet_asset_tx_payment::OnChargeAssetTransaction;
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{DispatchInfoOf, One, PostDispatchInfoOf, Zero},
    transaction_validity::{InvalidTransaction, TransactionValidityError},
};

pub struct KSMEnabledPallets;
impl Contains<RuntimeCall> for KSMEnabledPallets {
    fn contains(t: &RuntimeCall) -> bool {
        matches!(
            t,
            // We want users and DAOs to be able to operate multisigs using KSM.
            RuntimeCall::INV4(_)
                // We want DAOs to be able to operate XCMultisigs using KSM.
                | RuntimeCall::Rings(_)
                // These next 3 are needed to manage the KSM itself using KSM as the fee token.
                | RuntimeCall::Tokens(_)
                | RuntimeCall::XTokens(_)
                | RuntimeCall::Currencies(_)
        )
    }
}

impl pallet_asset_tx_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Fungibles = Tokens;
    type OnChargeAssetTransaction = FilteredTransactionCharger;
}

pub struct TnkrToKsm;
impl TnkrToKsm {
    pub fn to_asset_balance(balance: Balance) -> Balance {
        balance.saturating_div(20u128)
    }
}

pub struct FilteredTransactionCharger;
impl OnChargeAssetTransaction<Runtime> for FilteredTransactionCharger {
    type AssetId = AssetId;
    type Balance = Balance;
    type LiquidityInfo = Credit<AccountId, Tokens>;

    fn withdraw_fee(
        who: &AccountId,
        call: &RuntimeCall,
        _dispatch_info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        asset_id: AssetId,
        fee: Balance,
        _tip: Balance,
    ) -> Result<Credit<AccountId, Tokens>, frame_support::unsigned::TransactionValidityError> {
        if KSMEnabledPallets::contains(call) && asset_id == 1u32 {
            let min_converted_fee = if fee.is_zero() {
                Zero::zero()
            } else {
                One::one()
            };

            let fee = TnkrToKsm::to_asset_balance(fee).max(min_converted_fee);

            let can_withdraw = CurrencyAdapter::<Runtime, RelayAssetId>::can_withdraw(who, fee);

            if !matches!(can_withdraw, WithdrawConsequence::Success) {
                return Err(InvalidTransaction::Payment.into());
            }

            <Tokens as Balanced<AccountId>>::withdraw(
                asset_id,
                who,
                fee,
                Precision::Exact,
                Preservation::Expendable,
                Fortitude::Force,
            )
            .map_err(|_| TransactionValidityError::from(InvalidTransaction::Payment))
        } else {
            Err(TransactionValidityError::from(InvalidTransaction::Payment))
        }
    }

    fn correct_and_deposit_fee(
        who: &AccountId,
        _dispatch_info: &DispatchInfoOf<RuntimeCall>,
        _post_info: &PostDispatchInfoOf<RuntimeCall>,
        corrected_fee: Balance,
        _tip: Balance,
        paid: Credit<AccountId, Tokens>,
    ) -> Result<(u128, u128), TransactionValidityError> {
        let min_converted_fee = if corrected_fee.is_zero() {
            Zero::zero()
        } else {
            One::one()
        };

        let corrected_fee = TnkrToKsm::to_asset_balance(corrected_fee).max(min_converted_fee);

        let (final_fee, refund) = paid.split(corrected_fee);

        let _ = <Tokens as Balanced<AccountId>>::resolve(who, refund);

        DealWithKSMFees::on_unbalanced(final_fee);

        Ok((Zero::zero(), Zero::zero()))
    }
}

pub struct DealWithKSMFees;
impl OnUnbalanced<Credit<AccountId, Tokens>> for DealWithKSMFees {
    fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = Credit<AccountId, Tokens>>) {
        if let Some(mut fees) = fees_then_tips.next() {
            if let Some(tips) = fees_then_tips.next() {
                // Merge with fee, for now we send everything to the treasury
                let _ = fees.subsume(tips);
            }

            Self::on_unbalanced(fees);
        }
    }

    fn on_unbalanced(amount: Credit<AccountId, Tokens>) {
        let total: u128 = 100u128;
        let amount1 = amount.peek().saturating_mul(50u128) / total;
        let (to_collators, to_treasury) = amount.split(amount1);

        let _ = <Tokens as Balanced<AccountId>>::resolve(&TreasuryAccount::get(), to_treasury);

        let _ = <Tokens as Balanced<AccountId>>::resolve(&StakingPotAccount::get(), to_collators);
    }
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
pub struct ChargerExtra {
    #[codec(compact)]
    pub tip: Balance,
    pub asset_id: Option<AssetId>,
}
