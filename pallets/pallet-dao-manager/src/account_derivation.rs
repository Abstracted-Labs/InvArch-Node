//! Dao Account Derivation.
//!
//! ## Overview
//!
//! This module defines a method for generating account addresses, and how it's implemented within this
//! pallet. We use a custom derivation scheme to ensure that when a multisig is created, its AccountId
//! remains consistent across different parachains, promoting seamless interaction.
//!
//! ### The module contains:
//! - `DaoAccountDerivation` trait: The interface for our derivation method.
//! - Pallet implementation: The specific logic used to derive AccountIds.

use crate::{Config, Pallet};
use codec::{Compact, Encode};
use frame_support::traits::Get;
use sp_io::hashing::blake2_256;
use xcm::v3::{BodyId, BodyPart, Junction, Junctions};
/// Trait providing the XCM location and the derived account of a dao.
pub trait DaoAccountDerivation<T: Config> {
    /// Derives the dao's AccountId.
    fn derive_dao_account(dao_id: T::DaoId) -> T::AccountId;
    /// Specifies a dao's location.
    fn dao_location(dao_id: T::DaoId) -> Junctions;
}

impl<T: Config> DaoAccountDerivation<T> for Pallet<T>
where
    T::AccountId: From<[u8; 32]>,
{
    /// HashedDescription of the dao location from the perspective of a sibling chain.
    /// This derivation allows the local account address to match the account address in other parachains.
    /// Reference: https://github.com/paritytech/polkadot-sdk/blob/master/polkadot/xcm/xcm-builder/src/location_conversion.rs
    fn derive_dao_account(dao_id: T::DaoId) -> T::AccountId {
        blake2_256(
            &(
                b"SiblingChain",
                Compact::<u32>::from(T::ParaId::get()),
                (b"Body", BodyId::Index(dao_id.into()), BodyPart::Voice).encode(),
            )
                .encode(),
        )
        .into()
    }
    /// DAO location is defined as a plurality within the parachain.
    fn dao_location(dao_id: T::DaoId) -> Junctions {
        Junctions::X2(
            Junction::Parachain(T::ParaId::get()),
            Junction::Plurality {
                id: BodyId::Index(dao_id.into()),
                part: BodyPart::Voice,
            },
        )
    }
}
