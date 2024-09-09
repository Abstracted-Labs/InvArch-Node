//! Dispatches calls internally, charging fees to the multisig account.
//!
//! ## Overview
//!
//! This module employs a custom `MultisigInternalOrigin` to ensure calls originate
//! from the multisig account itself, automating fee payments. The `dispatch_call` function
//! includes pre and post dispatch handling for streamlined fee management within the multisig context.

use crate::{
    fee_handling::{FeeAsset, MultisigFeeHandler},
    origin::{DaoOrigin, MultisigInternalOrigin},
    Config, Error,
};
use frame_support::{dispatch::GetDispatchInfo, pallet_prelude::*};

use sp_runtime::traits::Dispatchable;

/// Dispatch a call executing pre/post dispatch for proper fee handling.
pub fn dispatch_call<T: Config>(
    dao_id: <T as Config>::DaoId,
    fee_asset: &FeeAsset,
    call: <T as Config>::RuntimeCall,
) -> DispatchResultWithPostInfo
where
    T::AccountId: From<[u8; 32]>,
{
    // Create new custom origin as the multisig.
    let internal_origin = MultisigInternalOrigin::new(dao_id);
    let multisig_account = internal_origin.to_account_id();
    let origin = DaoOrigin::Multisig(internal_origin).into();

    let info = call.get_dispatch_info();
    let len = call.encode().len();

    // Execute pre dispatch using the multisig account instead of the extrinsic caller.
    let pre = <T::FeeCharger as MultisigFeeHandler<T>>::pre_dispatch(
        fee_asset,
        &multisig_account,
        &call,
        &info,
        len,
    )
    .map_err(|_| Error::<T>::CallFeePaymentFailed)?;

    let dispatch_result = call.dispatch(origin);

    let post = match dispatch_result {
        Ok(p) => p,
        Err(e) => e.post_info,
    };

    <T::FeeCharger as MultisigFeeHandler<T>>::post_dispatch(
        fee_asset,
        Some(pre),
        &info,
        &post,
        len,
        &dispatch_result.map(|_| ()).map_err(|e| e.error),
    )
    .map_err(|_| Error::<T>::CallFeePaymentFailed)?;

    dispatch_result
}
