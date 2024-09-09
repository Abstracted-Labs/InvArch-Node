//! DAO creation and internal management.
//!
//! ## Overview
//!
//! This module handles the mechanics of creating multisigs or DAO's (OLD: referred to as "cores") and their lifecycle management. Key functions include:
//!
//! - `inner_create_dao`: Sets up a new dao, deriving its AccountId, distributing voting tokens, and handling creation fees.
//! - `inner_set_parameters`: Updates the DAO's operational rules.
//! - `is_asset_frozen`: Utility function for checking if a DAO's voting asset is frozen (can't be transferred by the owner).

use super::pallet::*;
use crate::{
    account_derivation::DaoAccountDerivation,
    fee_handling::{FeeAsset, FeeAssetNegativeImbalance, MultisigFeeHandler},
    origin::{ensure_multisig, DaoOrigin},
};
use frame_support::{
    pallet_prelude::*,
    traits::{
        fungibles::{Balanced, Mutate},
        tokens::{Fortitude, Precision, Preservation},
        Currency, ExistenceRequirement, WithdrawReasons,
    },
};
use frame_system::{ensure_signed, pallet_prelude::*};
use primitives::DaoInfo;
use sp_arithmetic::traits::{CheckedAdd, One};
use sp_runtime::Perbill;

pub type DaoIndexOf<T> = <T as Config>::DaoId;

pub type DaoMetadataOf<T> = BoundedVec<u8, <T as Config>::MaxMetadata>;

impl<T: Config> Pallet<T>
where
    Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
        From<<T as frame_system::Config>::RuntimeOrigin>,
    <T as frame_system::Config>::AccountId: From<[u8; 32]>,
{
    /// Inner function for the create_dao call.
    pub(crate) fn inner_create_dao(
        origin: OriginFor<T>,
        metadata: BoundedVec<u8, T::MaxMetadata>,
        minimum_support: Perbill,
        required_approval: Perbill,
        creation_fee_asset: FeeAsset,
    ) -> DispatchResult {
        NextCoreId::<T>::try_mutate(|next_id| -> DispatchResult {
            let creator = ensure_signed(origin)?;

            // Increment dao id counter
            let current_id = *next_id;
            *next_id = next_id
                .checked_add(&One::one())
                .ok_or(Error::<T>::NoAvailableDaoId)?;

            // Derive the account of this dao based on the dao id
            let dao_account = Self::derive_dao_account(current_id);

            // Mint base amount of voting token to the caller
            let seed_balance = <T as Config>::DaoSeedBalance::get();
            T::AssetsProvider::mint_into(current_id, &creator, seed_balance)?;

            // Build the structure of the new DAK
            // Tokens are set to frozen by default
            let info = DaoInfo {
                account: dao_account.clone(),
                metadata: metadata.clone(),
                minimum_support,
                required_approval,
                frozen_tokens: true,
            };

            // Charge creation fee from the caller
            T::FeeCharger::handle_creation_fee(match creation_fee_asset {
                FeeAsset::Native => {
                    FeeAssetNegativeImbalance::Native(<T as Config>::Currency::withdraw(
                        &creator,
                        T::DaoCreationFee::get(),
                        WithdrawReasons::TRANSACTION_PAYMENT,
                        ExistenceRequirement::KeepAlive,
                    )?)
                }

                FeeAsset::Relay => {
                    FeeAssetNegativeImbalance::Relay(<T as Config>::Tokens::withdraw(
                        T::RelayAssetId::get(),
                        &creator,
                        T::RelayDaoCreationFee::get(),
                        Precision::Exact,
                        Preservation::Protect,
                        Fortitude::Force,
                    )?)
                }
            });

            // Update dao storages
            CoreStorage::<T>::insert(current_id, info);
            CoreByAccount::<T>::insert(dao_account.clone(), current_id);

            Self::deposit_event(Event::DaoCreated {
                dao_account,
                metadata: metadata.to_vec(),
                dao_id: current_id,
                minimum_support,
                required_approval,
            });

            Ok(())
        })
    }

    /// Inner function for the set_parameters call.
    pub(crate) fn inner_set_parameters(
        origin: OriginFor<T>,
        metadata: Option<BoundedVec<u8, T::MaxMetadata>>,
        minimum_support: Option<Perbill>,
        required_approval: Option<Perbill>,
        frozen_tokens: Option<bool>,
    ) -> DispatchResult {
        let dao_origin = ensure_multisig::<T, OriginFor<T>>(origin)?;
        let dao_id = dao_origin.id;

        CoreStorage::<T>::try_mutate(dao_id, |dao| {
            let mut c = dao.take().ok_or(Error::<T>::DaoNotFound)?;

            if let Some(ms) = minimum_support {
                c.minimum_support = ms;
            }

            if let Some(ra) = required_approval {
                c.required_approval = ra;
            }

            if let Some(m) = metadata.clone() {
                c.metadata = m;
            }

            if let Some(f) = frozen_tokens {
                c.frozen_tokens = f;
            }

            *dao = Some(c);

            Self::deposit_event(Event::ParametersSet {
                dao_id,
                metadata: metadata.map(|m| m.to_vec()),
                minimum_support,
                required_approval,
                frozen_tokens,
            });

            Ok(())
        })
    }

    /// Checks if the voting asset is frozen.
    pub fn is_asset_frozen(dao_id: T::DaoId) -> Option<bool> {
        CoreStorage::<T>::get(dao_id).map(|c| c.frozen_tokens)
    }
}
