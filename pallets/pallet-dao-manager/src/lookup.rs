//! Custom account lookup implementation.
//!
//! ## Overview
//!
//!
//! This module implements the [`StaticLookup`] trait allowing for convenient lookup of a DAO's
//! AccountId from its DaoId.
//! This implementation abstracts on top of two lower level functions:
//! - `lookup_dao`: Used for accessing the storage and retrieving a DAO's AccountId.
//! - `lookup_address`: Used for converting from a `MultiAddress::Index` that contains a DaoId to this DAO's AccountId.

use crate::{Config, CoreByAccount, CoreStorage, Pallet};
use core::marker::PhantomData;
use frame_support::error::LookupError;
use sp_runtime::{traits::StaticLookup, MultiAddress};

impl<T: Config> Pallet<T> {
    /// Queries `CoreStorage` to retrieve the AccountId of a DAO.
    pub fn lookup_dao(dao_id: T::DaoId) -> Option<T::AccountId> {
        CoreStorage::<T>::get(dao_id).map(|dao| dao.account)
    }

    /// Matches `MultiAddress` to allow for a `MultiAddress::Index` containing a DaoId to be converted
    /// to it's derived AccountId.
    pub fn lookup_address(a: MultiAddress<T::AccountId, T::DaoId>) -> Option<T::AccountId> {
        match a {
            MultiAddress::Id(i) => Some(i),
            MultiAddress::Index(i) => Self::lookup_dao(i),
            _ => None,
        }
    }
}

/// StaticLookup implementor using MultiAddress::Index for looking up DAOs by id.
pub struct DaoLookup<T: Config>(PhantomData<T>);

impl<T: Config> StaticLookup for DaoLookup<T> {
    type Source = MultiAddress<T::AccountId, T::DaoId>;
    type Target = T::AccountId;

    fn lookup(a: Self::Source) -> Result<Self::Target, LookupError> {
        Pallet::<T>::lookup_address(a).ok_or(LookupError)
    }

    fn unlookup(a: Self::Target) -> Self::Source {
        match CoreByAccount::<T>::get(&a) {
            Some(dao_id) => MultiAddress::Index(dao_id),
            None => MultiAddress::Id(a),
        }
    }
}
