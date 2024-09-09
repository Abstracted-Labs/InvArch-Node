//! Custom Multisig Origin (`DaoOrigin`).
//!
//! ## Overview
//!
//! This module introduces a custom origin [`DaoOrigin`], enabling self-management for DAOs and
//! includes the [`ensure_multisig`] function to guarantee calls genuinely come from the multisig account.
//! This is an efficient approach considering that converting from DaoId to AccountId is a one-way operation,
//! so the origin brings the DaoId to dispatchable calls.
//! Converting to a `RawOrigin::Signed` origin for other calls is handled in the runtime.

use crate::{
    account_derivation::DaoAccountDerivation,
    pallet::{self, Origin, Pallet},
    Config,
};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{error::BadOrigin, pallet_prelude::RuntimeDebug};
use scale_info::TypeInfo;

/// Origin representing a dao by its id.
#[derive(PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, Clone, RuntimeDebug)]
pub enum DaoOrigin<T: pallet::Config> {
    Multisig(MultisigInternalOrigin<T>),
}

/// Internal origin for identifying the multisig DaoId.
#[derive(PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, Clone, RuntimeDebug)]
pub struct MultisigInternalOrigin<T: pallet::Config> {
    pub id: T::DaoId,
}

impl<T: pallet::Config> MultisigInternalOrigin<T>
where
    T::AccountId: From<[u8; 32]>,
{
    pub fn new(id: T::DaoId) -> Self {
        Self { id }
    }

    pub fn to_account_id(&self) -> T::AccountId {
        Pallet::<T>::derive_dao_account(self.id)
    }
}

/// Ensures the passed origin is a multisig, returning [`MultisigInternalOrigin`].
pub fn ensure_multisig<T: Config, OuterOrigin>(
    o: OuterOrigin,
) -> Result<MultisigInternalOrigin<T>, BadOrigin>
where
    OuterOrigin: Into<Result<pallet::Origin<T>, OuterOrigin>>,
{
    match o.into() {
        Ok(Origin::<T>::Multisig(internal)) => Ok(internal),
        _ => Err(BadOrigin),
    }
}
