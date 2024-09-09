//! Provides supporting types and traits for the staking pallet.
//!
//! ## Overview
//!
//! Primitives provides the foundational types and traits for a staking pallet.  
//!
//! ## Types overview:
//!
//! - `BalanceOf` - A type alias for the balance of a currency in the system.
//! - `DaoMetadata` - A struct that holds metadata for a DAO entity in the system.
//! - `DaoInfo` - A struct that holds information about a DAO entity, including its account ID and metadata.
//! - `RewardInfo` - A struct that holds information about rewards, including the balance for stakers and the DAO.
//! - `EraInfo` - A struct that holds information about a specific era, including rewards, staked balance, active stake, and locked balance.
//! - `DaoStakeInfo` - A struct that holds information about a DAO's stake, including the total balance,
//!    number of stakers, and whether a reward has been claimed.
//! - `EraStake` - A struct that holds information about the stake for a specific era.
//! - `StakerInfo` - A struct that holds information about a staker's stakes across different eras.
//! - `UnlockingChunk` - A struct that holds information about an unlocking chunk of balance.
//! - `UnbondingInfo` - A struct that holds information about unbonding chunks of balance.
//! - `AccountLedger` - A struct that holds information about an account's locked balance and unbonding information.

use codec::{Decode, Encode, FullCodec, HasCompact, MaxEncodedLen};
use cumulus_primitives_core::{AggregateMessageOrigin, MultiLocation, ParaId};
use frame_support::{
    pallet_prelude::Weight,
    traits::{Currency, ProcessMessage, QueueFootprint, QueuePausedQuery},
};
use pallet_message_queue::OnQueueChanged;
use scale_info::{prelude::marker::PhantomData, TypeInfo};
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, Zero},
    Perbill, RuntimeDebug,
};
use sp_std::{fmt::Debug, ops::Add, prelude::*};

pub use crate::pallet::*;
use crate::weights::WeightInfo;

/// The balance type of this pallet.
pub type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

const MAX_ASSUMED_VEC_LEN: u32 = 10;

/// Metadata for a dao entity in the system.
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct DaoMetadata<Name, Description, Image> {
    pub name: Name,
    pub description: Description,
    pub image: Image,
}

/// Information about a dao entity, including its account ID and metadata.
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct DaoInfo<AccountId, Metadata> {
    pub account: AccountId,
    pub metadata: Metadata,
}

/// Information about rewards, including the balance for stakers and the dao.
#[derive(PartialEq, Eq, Clone, Default, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct RewardInfo<Balance: HasCompact + MaxEncodedLen> {
    #[codec(compact)]
    pub(crate) stakers: Balance,
    #[codec(compact)]
    pub(crate) dao: Balance,
}

/// Information about a specific era, including rewards, staked balance, active stake, and locked balance.
#[derive(PartialEq, Eq, Clone, Default, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct EraInfo<Balance: HasCompact + MaxEncodedLen> {
    pub(crate) rewards: RewardInfo<Balance>,
    #[codec(compact)]
    pub(crate) staked: Balance,
    #[codec(compact)]
    pub(crate) active_stake: Balance,
    #[codec(compact)]
    pub(crate) locked: Balance,
}

/// Information about a dao's stake, including the total balance, number of stakers, and whether a reward has been claimed.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Default, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct DaoStakeInfo<Balance: HasCompact + MaxEncodedLen> {
    #[codec(compact)]
    pub(crate) total: Balance,
    #[codec(compact)]
    pub(crate) number_of_stakers: u32,
    pub(crate) reward_claimed: bool,
    pub(crate) active: bool,
}

/// Information about the stake for a specific era.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub(crate) struct EraStake<Balance: AtLeast32BitUnsigned + Copy + MaxEncodedLen> {
    #[codec(compact)]
    pub(crate) staked: Balance,
    #[codec(compact)]
    pub(crate) era: Era,
}

/// Information about a staker's stakes across different eras.
#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct StakerInfo<Balance: AtLeast32BitUnsigned + Copy + MaxEncodedLen> {
    pub(crate) stakes: Vec<EraStake<Balance>>,
}

impl<Balance: AtLeast32BitUnsigned + Copy + MaxEncodedLen> MaxEncodedLen for StakerInfo<Balance> {
    fn max_encoded_len() -> usize {
        codec::Compact(MAX_ASSUMED_VEC_LEN)
            .encoded_size()
            .saturating_add(
                (MAX_ASSUMED_VEC_LEN as usize)
                    .saturating_mul(EraStake::<Balance>::max_encoded_len()),
            )
    }
}

impl<Balance: AtLeast32BitUnsigned + Copy + MaxEncodedLen> StakerInfo<Balance> {
    pub(crate) fn is_empty(&self) -> bool {
        self.stakes.is_empty()
    }

    pub(crate) fn len(&self) -> u32 {
        self.stakes.len() as u32
    }

    /// Stakes the given value in the current era, mutates StakerInfo in-place.
    pub(crate) fn stake(&mut self, current_era: Era, value: Balance) -> Result<(), &str> {
        if let Some(era_stake) = self.stakes.last_mut() {
            if era_stake.era > current_era {
                return Err("Unexpected era");
            }

            let new_stake_value = era_stake.staked.saturating_add(value);

            if current_era == era_stake.era {
                *era_stake = EraStake {
                    staked: new_stake_value,
                    era: current_era,
                }
            } else {
                self.stakes.push(EraStake {
                    staked: new_stake_value,
                    era: current_era,
                })
            }
        } else {
            self.stakes.push(EraStake {
                staked: value,
                era: current_era,
            });
        }

        Ok(())
    }

    /// Unstakes the given value in the current era, mutates StakerInfo in-place.
    pub(crate) fn unstake(&mut self, current_era: Era, value: Balance) -> Result<(), &str> {
        if let Some(era_stake) = self.stakes.last_mut() {
            if era_stake.era > current_era {
                return Err("Unexpected era");
            }

            let new_stake_value = era_stake.staked.saturating_sub(value);
            if current_era == era_stake.era {
                *era_stake = EraStake {
                    staked: new_stake_value,
                    era: current_era,
                }
            } else {
                self.stakes.push(EraStake {
                    staked: new_stake_value,
                    era: current_era,
                })
            }

            if !self.stakes.is_empty() && self.stakes[0].staked.is_zero() {
                self.stakes.remove(0);
            }
        }

        Ok(())
    }

    /// Claims the stake for the current era, mutates StakerInfo in-place.  
    /// Returns the era and the staked balance.
    pub(crate) fn claim(&mut self) -> (Era, Balance) {
        if let Some(era_stake) = self.stakes.first() {
            let era_stake = *era_stake;

            // this checks if the last claim was from an older era or if the latest staking info is from
            // a newer era compared to the last claim, allowing the user to increase their stake while not losing
            // or messing with their stake from the previous eras.
            if self.stakes.len() == 1 || self.stakes[1].era > era_stake.era + 1 {
                self.stakes[0] = EraStake {
                    staked: era_stake.staked,
                    era: era_stake.era.saturating_add(1),
                }
            } else {
                self.stakes.remove(0);
            }

            if !self.stakes.is_empty() && self.stakes[0].staked.is_zero() {
                self.stakes.remove(0);
            }

            (era_stake.era, era_stake.staked)
        } else {
            (0, Zero::zero())
        }
    }

    /// Returns the latest staked balance.
    pub(crate) fn latest_staked_value(&self) -> Balance {
        self.stakes.last().map_or(Zero::zero(), |x| x.staked)
    }
}

/// A chunk of balance that is unlocking until a specific era.
#[derive(
    Clone, PartialEq, Eq, Copy, Encode, Decode, Default, RuntimeDebug, TypeInfo, MaxEncodedLen,
)]
pub(crate) struct UnlockingChunk<Balance: MaxEncodedLen> {
    #[codec(compact)]
    pub(crate) amount: Balance,
    #[codec(compact)]
    pub(crate) unlock_era: Era,
}

impl<Balance> UnlockingChunk<Balance>
where
    Balance: Add<Output = Balance> + Copy + MaxEncodedLen,
{
    /// Adds the given amount to the chunk's amount.
    pub(crate) fn add_amount(&mut self, amount: Balance) {
        self.amount = self.amount + amount
    }
}

/// Information about unbonding chunks of balance.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub(crate) struct UnbondingInfo<Balance: AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen> {
    pub(crate) unlocking_chunks: Vec<UnlockingChunk<Balance>>,
}

impl<Balance: AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen> MaxEncodedLen
    for UnbondingInfo<Balance>
{
    fn max_encoded_len() -> usize {
        codec::Compact(MAX_ASSUMED_VEC_LEN)
            .encoded_size()
            .saturating_add(
                (MAX_ASSUMED_VEC_LEN as usize)
                    .saturating_mul(UnlockingChunk::<Balance>::max_encoded_len()),
            )
    }
}

impl<Balance> UnbondingInfo<Balance>
where
    Balance: AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen,
{
    pub(crate) fn len(&self) -> u32 {
        self.unlocking_chunks.len() as u32
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.unlocking_chunks.is_empty()
    }

    /// Returns the total amount of the unlocking chunks.
    pub(crate) fn sum(&self) -> Balance {
        self.unlocking_chunks
            .iter()
            .map(|chunk| chunk.amount)
            .reduce(|c1, c2| c1 + c2)
            .unwrap_or_default()
    }

    /// Adds the given chunk to the unbonding info.
    pub(crate) fn add(&mut self, chunk: UnlockingChunk<Balance>) {
        match self
            .unlocking_chunks
            .binary_search_by(|x| x.unlock_era.cmp(&chunk.unlock_era))
        {
            Ok(pos) => self.unlocking_chunks[pos].add_amount(chunk.amount),
            Err(pos) => self.unlocking_chunks.insert(pos, chunk),
        }
    }

    /// returns the chucks before and after a given era.
    pub(crate) fn partition(self, era: Era) -> (Self, Self) {
        let (matching_chunks, other_chunks): (
            Vec<UnlockingChunk<Balance>>,
            Vec<UnlockingChunk<Balance>>,
        ) = self
            .unlocking_chunks
            .iter()
            .partition(|chunk| chunk.unlock_era <= era);

        (
            Self {
                unlocking_chunks: matching_chunks,
            },
            Self {
                unlocking_chunks: other_chunks,
            },
        )
    }
}

/// Information about an account's locked balance and unbonding information.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Default, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct AccountLedger<Balance: AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen> {
    #[codec(compact)]
    pub(crate) locked: Balance,
    pub(crate) unbonding_info: UnbondingInfo<Balance>,
}

impl<Balance: AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen> AccountLedger<Balance> {
    pub(crate) fn is_empty(&self) -> bool {
        self.locked.is_zero() && self.unbonding_info.is_empty()
    }
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, Eq, PartialEq, TypeInfo, Debug)]
pub enum CustomAggregateMessageOrigin<XcmOrigin> {
    Aggregate(XcmOrigin),
    UnregisterMessageOrigin,
}

/// Custom Convert a sibling `ParaId` to an `AggregateMessageOrigin`.
pub struct CustomParaIdToSibling;
impl sp_runtime::traits::Convert<ParaId, CustomAggregateMessageOrigin<AggregateMessageOrigin>>
    for CustomParaIdToSibling
{
    fn convert(para_id: ParaId) -> CustomAggregateMessageOrigin<AggregateMessageOrigin> {
        CustomAggregateMessageOrigin::Aggregate(AggregateMessageOrigin::Sibling(para_id))
    }
}

pub struct CustomNarrowOriginToSibling<Inner, T>(PhantomData<(Inner, T)>);
impl<Inner: QueuePausedQuery<ParaId>, T: Config>
    QueuePausedQuery<CustomAggregateMessageOrigin<AggregateMessageOrigin>>
    for CustomNarrowOriginToSibling<Inner, T>
{
    fn is_paused(origin: &CustomAggregateMessageOrigin<AggregateMessageOrigin>) -> bool {
        match origin {
            CustomAggregateMessageOrigin::Aggregate(AggregateMessageOrigin::Sibling(id)) => {
                Inner::is_paused(id)
            }
            CustomAggregateMessageOrigin::Aggregate(_) => false,
            CustomAggregateMessageOrigin::UnregisterMessageOrigin => Pallet::<T>::is_halted(),
        }
    }
}

impl<Inner: OnQueueChanged<ParaId>, T: Config>
    OnQueueChanged<CustomAggregateMessageOrigin<AggregateMessageOrigin>>
    for CustomNarrowOriginToSibling<Inner, T>
{
    fn on_queue_changed(
        origin: CustomAggregateMessageOrigin<AggregateMessageOrigin>,
        fp: QueueFootprint,
    ) {
        match origin {
            CustomAggregateMessageOrigin::Aggregate(AggregateMessageOrigin::Sibling(id)) => {
                Inner::on_queue_changed(id, fp)
            }
            CustomAggregateMessageOrigin::Aggregate(_) => (),
            CustomAggregateMessageOrigin::UnregisterMessageOrigin => (),
        }
    }
}

pub struct CustomMessageProcessor<Origin, XcmOrigin, XcmProcessor, C, T>(
    PhantomData<(Origin, XcmOrigin, XcmProcessor, C, T)>,
);

impl<Origin, XcmOrigin, XcmProcessor, Call, T> ProcessMessage
    for CustomMessageProcessor<Origin, XcmOrigin, XcmProcessor, Call, T>
where
    Origin: Into<CustomAggregateMessageOrigin<XcmOrigin>>
        + FullCodec
        + MaxEncodedLen
        + Clone
        + Eq
        + PartialEq
        + TypeInfo
        + Debug,
    XcmOrigin:
        Into<MultiLocation> + FullCodec + MaxEncodedLen + Clone + Eq + PartialEq + TypeInfo + Debug,
    XcmProcessor: ProcessMessage<Origin = XcmOrigin>,
    T: Config,
{
    type Origin = Origin;
    fn process_message(
        message: &[u8],
        _origin: Self::Origin,
        meter: &mut frame_support::weights::WeightMeter,
        _id: &mut [u8; 32],
    ) -> Result<bool, frame_support::traits::ProcessMessageError> {
        match _origin.into() {
            CustomAggregateMessageOrigin::Aggregate(o) => {
                XcmProcessor::process_message(message, o, meter, _id)
            }
            CustomAggregateMessageOrigin::UnregisterMessageOrigin => {
                let call: UnregisterMessage<T> = UnregisterMessage::<T>::decode(&mut &message[..])
                    .map_err(|_| frame_support::traits::ProcessMessageError::Corrupt)?;

                let unstake_weight = <T as Config>::WeightInfo::unstake();

                let meter_limit = meter.limit();

                let thirdy_of_limit = Perbill::from_percent(30) * meter_limit;

                let meter_remaining = meter.remaining();

                let min_desired = {
                    // if a third of the proofsize is > 1/2 MB then we use a 1/2 MB for the proofsize weight.
                    if thirdy_of_limit.proof_size() >= 524288 {
                        Weight::from_parts(thirdy_of_limit.ref_time(), 524288)
                    } else {
                        thirdy_of_limit
                    }
                };

                // only use less than 30% of all the weight the message queue can provide.
                if !meter_remaining.all_gte(Perbill::from_percent(70) * meter_limit) {
                    return Err(frame_support::traits::ProcessMessageError::Yield);
                }

                let max_calls = {
                    match min_desired.checked_div_per_component(&unstake_weight) {
                        Some(x) if x > 0 => x.min(100),
                        _ => return Err(frame_support::traits::ProcessMessageError::Yield),
                    }
                };

                let max_weight = max_calls * unstake_weight;

                let chunk_result = crate::pallet::Pallet::<T>::process_dao_unregistration_shard(
                    call.stakers_to_unstake,
                    call.dao_id,
                    call.era,
                    max_calls,
                );

                match chunk_result {
                    Ok(weight) => {
                        if let Some(actual_weight) = weight.actual_weight {
                            meter.try_consume(actual_weight).map_err(|_| {
                                frame_support::traits::ProcessMessageError::Overweight(
                                    actual_weight,
                                )
                            })?;
                        } else {
                            meter.try_consume(max_weight).map_err(|_| {
                                frame_support::traits::ProcessMessageError::Overweight(max_weight)
                            })?;
                        }
                        Ok(true)
                    }
                    Err(_) => {
                        meter.try_consume(max_weight).map_err(|_| {
                            frame_support::traits::ProcessMessageError::Overweight(max_weight)
                        })?;
                        Ok(false)
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, Default, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct UnregisterMessage<T: Config> {
    pub(crate) dao_id: T::DaoId,
    pub(crate) era: Era,
    pub(crate) stakers_to_unstake: u32,
}
