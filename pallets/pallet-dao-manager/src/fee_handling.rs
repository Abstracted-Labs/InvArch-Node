//! MultisigFeeHandler trait.
//!
//! ## Overview
//!
//! Defines how transaction fees are charged to the multisig account.
//! This trait requires proper runtime implementation to allow the usage of native or non-native assets.

use crate::Config;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
    traits::{fungibles::Credit, Currency},
    unsigned::TransactionValidityError,
};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{DispatchInfoOf, PostDispatchInfoOf},
    DispatchResult,
};

/// Represents the asset to be used by the multisig for paying transaction fees.
///
/// This enum defines the assets that can be used to pay for transaction fees.
#[derive(Clone, TypeInfo, Encode, Decode, MaxEncodedLen, Debug, PartialEq, Eq)]
pub enum FeeAsset {
    Native,
    Relay,
}

/// Represents a potential negative asset balance incurred during fee payment operations
/// within a multisig context.
///
/// This enum handles imbalances in either the native token or
/// a relay chain asset used for fees.
///
/// - `Native(NativeNegativeImbalance)`: Indicates a deficit balance in the chain's native asset.
/// - `Relay(RelayNegativeImbalance)`: Indicates a deficit balance in an asset originating on the relay chain.
///
/// This enum plays a role in resolving deficit balances in the `MultisigFeeHandler` trait.
pub enum FeeAssetNegativeImbalance<NativeNegativeImbalance, RelayNegativeImbalance> {
    Native(NativeNegativeImbalance),
    Relay(RelayNegativeImbalance),
}

/// Fee handler trait.
///
/// This should be implemented properly in the runtime to account for native and non-native assets.
pub trait MultisigFeeHandler<T: Config> {
    /// Type returned by `pre_dispatch` - implementation dependent.
    type Pre;

    /// Checks if the fee can be paid using the selected asset.
    fn pre_dispatch(
        asset: &FeeAsset,
        who: &T::AccountId,
        call: &<T as Config>::RuntimeCall,
        info: &DispatchInfoOf<<T as Config>::RuntimeCall>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError>;

    /// Charges the call dispatching fee from the multisig directly.
    fn post_dispatch(
        asset: &FeeAsset,
        pre: Option<Self::Pre>,
        info: &DispatchInfoOf<<T as Config>::RuntimeCall>,
        post_info: &PostDispatchInfoOf<<T as Config>::RuntimeCall>,
        len: usize,
        result: &DispatchResult,
    ) -> Result<(), TransactionValidityError>;

    /// Charges the fee for creating the dao (multisig).
    fn handle_creation_fee(
        imbalance: FeeAssetNegativeImbalance<
            <T::Currency as Currency<T::AccountId>>::NegativeImbalance,
            Credit<T::AccountId, T::Tokens>,
        >,
    );
}
