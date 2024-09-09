//! Voting Mechanism.
//!
//! ## Overview
//!
//! This module provides a weighted voting [`Tally`] implementation used for managing the multisig's proposals.
//! Members each have a balance in voting tokens and this balance differentiate their voting power
//! as every vote utilizes the entire `power` of the said member.
//! This empowers decision-making where certain members possess greater influence.

use crate::{origin::DaoOrigin, BalanceOf, Config, CoreStorage, Error, Multisig, Pallet};
use codec::{Decode, Encode, HasCompact, MaxEncodedLen};
use core::marker::PhantomData;
use frame_support::{
    pallet_prelude::{Member, RuntimeDebug},
    traits::{fungibles::Inspect, PollStatus, VoteTally},
    BoundedBTreeMap, CloneNoBound, EqNoBound, Parameter, PartialEqNoBound, RuntimeDebugNoBound,
};
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{One, Zero},
    DispatchError, Perbill,
};
use sp_std::vec::Vec;

pub type Votes<T> = BalanceOf<T>;
pub type Dao<T> = <T as Config>::DaoId;

/// Aggregated votes for an ongoing poll by members of a dao.
#[derive(
    CloneNoBound,
    PartialEqNoBound,
    EqNoBound,
    RuntimeDebugNoBound,
    TypeInfo,
    Encode,
    Decode,
    MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct Tally<T: Config> {
    pub ayes: Votes<T>,
    pub nays: Votes<T>,
    pub records: BoundedBTreeMap<T::AccountId, Vote<Votes<T>>, T::MaxCallers>,
    dummy: PhantomData<T>,
}

impl<T: Config> Tally<T> {
    /// Allows for building a `Tally` manually.
    pub fn from_parts(
        ayes: Votes<T>,
        nays: Votes<T>,
        records: BoundedBTreeMap<T::AccountId, Vote<Votes<T>>, T::MaxCallers>,
    ) -> Self {
        Tally {
            ayes,
            nays,
            records,
            dummy: PhantomData,
        }
    }

    /// Check if a vote is valid and add the member's total voting token balance to the tally.
    pub fn process_vote(
        &mut self,
        account: T::AccountId,
        maybe_vote: Option<Vote<Votes<T>>>,
    ) -> Result<Vote<Votes<T>>, DispatchError> {
        let votes = if let Some(vote) = maybe_vote {
            self.records
                .try_insert(account, vote)
                .map_err(|_| Error::<T>::MaxCallersExceeded)?;
            vote
        } else {
            self.records.remove(&account).ok_or(Error::<T>::NotAVoter)?
        };

        let (ayes, nays) = self.records.values().fold(
            (Zero::zero(), Zero::zero()),
            |(mut ayes, mut nays): (Votes<T>, Votes<T>), vote| {
                match vote {
                    Vote::Aye(v) => ayes += *v,
                    Vote::Nay(v) => nays += *v,
                };
                (ayes, nays)
            },
        );

        self.ayes = ayes;
        self.nays = nays;

        Ok(votes)
    }
}

impl<T: Config> VoteTally<Votes<T>, Dao<T>> for Tally<T> {
    fn new(_: Dao<T>) -> Self {
        Self {
            ayes: Zero::zero(),
            nays: Zero::zero(),
            records: BoundedBTreeMap::default(),
            dummy: PhantomData,
        }
    }

    fn ayes(&self, _: Dao<T>) -> Votes<T> {
        self.ayes
    }

    fn support(&self, class: Dao<T>) -> Perbill {
        Perbill::from_rational(self.ayes, T::AssetsProvider::total_issuance(class))
    }

    fn approval(&self, _: Dao<T>) -> Perbill {
        Perbill::from_rational(
            self.ayes,
            <Votes<T> as One>::one().max(self.ayes + self.nays),
        )
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn unanimity(_: Dao<T>) -> Self {
        todo!()
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn rejection(_: Dao<T>) -> Self {
        todo!()
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn from_requirements(_: Perbill, _: Perbill, _: Dao<T>) -> Self {
        todo!()
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn setup(_: Dao<T>, _: Perbill) {
        todo!()
    }
}

pub trait CustomPolling<Tally> {
    type Index: Ord + PartialOrd + Copy + MaxEncodedLen;
    type Votes: Parameter + Ord + PartialOrd + Copy + HasCompact + MaxEncodedLen;
    type Class: Parameter + Member + Ord + PartialOrd + MaxEncodedLen;
    type Moment;

    // Provides a vec of values that `T` may take.
    fn classes() -> Vec<Self::Class>;

    /// `Some` if the referendum `index` can be voted on, along with the tally and class of
    /// referendum.
    ///
    /// Don't use this if you might mutate - use `try_access_poll` instead.
    fn as_ongoing(class: Self::Class, index: Self::Index) -> Option<(Tally, Self::Class)>;

    fn access_poll<R>(
        class: Self::Class,
        index: Self::Index,
        f: impl FnOnce(PollStatus<&mut Tally, Self::Moment, Self::Class>) -> R,
    ) -> R;

    fn try_access_poll<R>(
        class: Self::Class,
        index: Self::Index,
        f: impl FnOnce(PollStatus<&mut Tally, Self::Moment, Self::Class>) -> Result<R, DispatchError>,
    ) -> Result<R, DispatchError>;
}

impl<T: Config> CustomPolling<Tally<T>> for Pallet<T> {
    type Index = T::Hash;
    type Votes = Votes<T>;
    type Moment = BlockNumberFor<T>;
    type Class = T::DaoId;

    fn classes() -> Vec<Self::Class> {
        CoreStorage::<T>::iter_keys().collect()
    }

    fn access_poll<R>(
        class: Self::Class,
        index: Self::Index,
        f: impl FnOnce(PollStatus<&mut Tally<T>, BlockNumberFor<T>, T::DaoId>) -> R,
    ) -> R {
        match Multisig::<T>::get(class, index) {
            Some(mut m) => {
                let result = f(PollStatus::Ongoing(&mut m.tally, class));
                Multisig::<T>::insert(class, index, m);
                result
            }
            _ => f(PollStatus::None),
        }
    }

    fn try_access_poll<R>(
        class: Self::Class,
        index: Self::Index,
        f: impl FnOnce(
            PollStatus<&mut Tally<T>, BlockNumberFor<T>, T::DaoId>,
        ) -> Result<R, DispatchError>,
    ) -> Result<R, DispatchError> {
        match Multisig::<T>::get(class, index) {
            Some(mut m) => {
                let result = f(PollStatus::Ongoing(&mut m.tally, class))?;
                Multisig::<T>::insert(class, index, m);
                Ok(result)
            }
            _ => f(PollStatus::None),
        }
    }

    fn as_ongoing(class: Self::Class, index: Self::Index) -> Option<(Tally<T>, T::DaoId)> {
        Multisig::<T>::get(class, index).map(|m| (m.tally, class))
    }
}

/// Represents a proposal vote within a multisig context.
///
/// This is both the vote and how many voting tokens it carries.
#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum Vote<Votes> {
    Aye(Votes),
    Nay(Votes),
}

/// Type alias for [`Vote`] with [`BalanceOf`].
pub type VoteRecord<T> = Vote<Votes<T>>;

impl<T: Config> Pallet<T>
where
    Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
        From<<T as frame_system::Config>::RuntimeOrigin>,
{
    /// Returns the minimum support and required approval thresholds of a dao.
    pub fn minimum_support_and_required_approval(dao_id: T::DaoId) -> Option<(Perbill, Perbill)> {
        CoreStorage::<T>::get(dao_id).map(|dao| (dao.minimum_support, dao.required_approval))
    }
}
