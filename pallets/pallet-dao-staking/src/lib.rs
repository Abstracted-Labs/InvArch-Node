//! # DAO Staking pallet
//! A pallet for allowing DAOs to be staked towards.
//!
//! ## Overview
//!
//! This pallet provides functionality to allow 2 sets of entities to participate in distribution of tokens
//! available in a predefined pot account.
//! The tokens provided to the pot account are to be handled by the Runtime,
//! either directly or with the assistance of another pallet.
//!
//! The 2 entity sets will be referred to in code as DAOs and Stakers:
//!
//! ### DAOs
//! DAOs are virtual accounts that have an ID used to derive their own account address,
//! their task in the process is to register themselves and have Stakers lock tokens in favor of a specific DAO.
//! DAOs receive their fraction of the pot rewards based on the total amount staked towards them by Stakers,
//! however, a DAO must have total stake above the defined threshold (making it `active`), otherwise they won't be entitled to rewards.
//!
//! ### Stakers
//! Stakers are any account existing on the chain, their task is to lock tokens in favor of a DAO.
//! Unlike DAOs, Stakers get their fraction of the rewards based on their own stake and regardless of
//! the `active` state of the DAO they staked towards.
//!
//! ## Relevant runtime configs
//!
//! * `BlocksPerEra` - Defines how many blocks constitute an era.
//! * `RegisterDeposit` - Defines the deposit amount for a DAO to register in the system.
//! * `MaxStakersPerDao` - Defines the maximum amount of Stakers allowed staking simultaneously towards the same DAO.
//! * `MinimumStakingAmount` - Defines the minimum amount a Staker has to stake to participate.
//! * `UnbondingPeriod` - Defines the period, in eras, that it takes to unbond a stake.
//! * `RewardRatio` - Defines the ratio of balance from the pot to distribute to DAOs and Stakers, respectively.
//! * `StakeThresholdForActiveDao` - Defines the threshold of stake a DAO needs to surpass to become active.
//!
//! **Example Runtime implementation can be found in [src/testing/mock.rs](./src/testing/mock.rs)**
//!
//! ## Dispatchable Functions
//!
//! * `register_dao` - Registers a DAO in the system.
//! * `unregister_dao` - Unregisters a DAO from the system, starting the unbonding period for the Stakers.
//! * `change_dao_metadata` - Changes the metadata tied to a DAO.
//! * `stake` - Stakes tokens towards a DAO.
//! * `unstake` - Unstakes tokens from a DAO and starts the unbonding period for those tokens.
//! * `withdraw_unstaked` - Withdraws tokens that have already been through the unbonding period.
//! * `staker_claim_rewards` - Claims rewards available for a Staker.
//! * `dao_claim_rewards` - Claims rewards available for a DAO.
//! * `halt_unhalt_pallet` - Allows Root to trigger a halt of the system, eras will stop counting and rewards won't be distributed.
//!
//! [`Call`]: ./enum.Call.html
//! [`Config`]: ./trait.Config.html

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    dispatch::{Pays, PostDispatchInfo},
    ensure,
    pallet_prelude::*,
    traits::{
        Currency, ExistenceRequirement, Get, HandleMessage, Imbalance, LockIdentifier,
        LockableCurrency, OnUnbalanced, ProcessMessage, QueuePausedQuery, ReservableCurrency,
        WithdrawReasons,
    },
    weights::{Weight, WeightToFee},
    BoundedSlice, PalletId,
};
use frame_system::{ensure_signed, pallet_prelude::*};
use sp_runtime::{
    traits::{AccountIdConversion, Saturating, Zero},
    Perbill,
};
use sp_std::{
    convert::{From, TryInto},
    vec::Vec,
};

pub mod primitives;
use primitives::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod testing;
pub mod weights;

pub use weights::WeightInfo;

/// Staking lock identifier.
const LOCK_ID: LockIdentifier = *b"ocif-stk";

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use pallet_dao_manager::{
        origin::{ensure_multisig, DaoOrigin},
        DaoAccountDerivation,
    };

    use pallet_message_queue::{self};

    use super::*;

    /// The balance type of this pallet.
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    #[pallet::pallet]
    pub struct Pallet<T>(PhantomData<T>);

    /// The opaque token type for an imbalance. This is returned by unbalanced operations and must be dealt with.
    type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
        <T as frame_system::Config>::AccountId,
    >>::NegativeImbalance;

    /// The dao metadata type of this pallet.
    pub type DaoMetadataOf<T> = DaoMetadata<
        BoundedVec<u8, <T as Config>::MaxNameLength>,
        BoundedVec<u8, <T as Config>::MaxDescriptionLength>,
        BoundedVec<u8, <T as Config>::MaxImageUrlLength>,
    >;

    /// The dao information type, containing a dao's AccountId and DaoMetadataOf.
    pub type DaoInfoOf<T> = DaoInfo<<T as frame_system::Config>::AccountId, DaoMetadataOf<T>>;

    /// Alias type for the era identifier type.
    pub type Era = u32;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_dao_manager::Config + pallet_message_queue::Config
    {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The currency used in staking.
        type Currency: LockableCurrency<Self::AccountId, Moment = BlockNumberFor<Self>>
            + ReservableCurrency<Self::AccountId>;

        // type DaoId: Parameter
        //     + Member
        //     + AtLeast32BitUnsigned
        //     + Default
        //     + Copy
        //     + Display
        //     + MaxEncodedLen
        //     + Clone
        //     + From<<Self as pallet_dao_manager::Config>::DaoId>;

        /// Number of blocks per era.
        #[pallet::constant]
        type BlocksPerEra: Get<BlockNumberFor<Self>>;

        /// Deposit amount that will be reserved as part of new dao registration.
        #[pallet::constant]
        type RegisterDeposit: Get<BalanceOf<Self>>;

        /// Maximum number of unique stakers per dao.
        #[pallet::constant]
        type MaxStakersPerDao: Get<u32>;

        /// Minimum amount user must have staked on a dao.
        /// User can stake less if they already have the minimum staking amount staked.
        #[pallet::constant]
        type MinimumStakingAmount: Get<BalanceOf<Self>>;

        /// Account Identifier from which the internal Pot is generated.
        #[pallet::constant]
        type PotId: Get<PalletId>;

        /// The minimum amount required to keep an account open.
        #[pallet::constant]
        type ExistentialDeposit: Get<BalanceOf<Self>>;

        /// Max number of unlocking chunks per account Id <-> dao Id pairing.
        /// If value is zero, unlocking becomes impossible.
        #[pallet::constant]
        type MaxUnlocking: Get<u32>;

        /// Number of eras that need to pass until unstaked value can be withdrawn.
        /// When set to `0`, it's equal to having no unbonding period.
        #[pallet::constant]
        type UnbondingPeriod: Get<u32>;

        /// Max number of unique `EraStake` values that can exist for a `(staker, dao)` pairing.
        ///
        /// When stakers claims rewards, they will either keep the number of `EraStake` values the same or they will reduce them by one.
        /// Stakers cannot add an additional `EraStake` value by calling `bond&stake` or `unbond&unstake` if they've reached the max number of values.
        ///
        /// This ensures that history doesn't grow indefinitely - if there are too many chunks, stakers should first claim their former rewards
        /// before adding additional `EraStake` values.
        #[pallet::constant]
        type MaxEraStakeValues: Get<u32>;

        /// Reward ratio of the pot to be distributed between the dao and stakers, respectively.
        #[pallet::constant]
        type RewardRatio: Get<(u32, u32)>;

        /// Threshold of staked tokens necessary for a dao to become active.
        #[pallet::constant]
        type StakeThresholdForActiveDao: Get<BalanceOf<Self>>;

        /// Maximum length of a dao's name.
        #[pallet::constant]
        type MaxNameLength: Get<u32>;

        /// Maximum length of a dao's description.
        #[pallet::constant]
        type MaxDescriptionLength: Get<u32>;

        /// Maximum length of a dao's image URL.
        #[pallet::constant]
        type MaxImageUrlLength: Get<u32>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;

        /// Message queue interface.
        type StakingMessage: HandleMessage;

        /// Weight to fee conversion provider, from pallet_transaction_payment.
        type WeightToFee: WeightToFee<Balance = BalanceOf<Self>>;

        /// Fee charghing interface.
        type OnUnbalanced: OnUnbalanced<NegativeImbalanceOf<Self>>;
    }

    /// General information about the staker.
    #[pallet::storage]
    #[pallet::getter(fn ledger)]
    pub type Ledger<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, AccountLedger<BalanceOf<T>>, ValueQuery>;

    /// The current era index.
    #[pallet::storage]
    #[pallet::getter(fn current_era)]
    pub type CurrentEra<T> = StorageValue<_, Era, ValueQuery>;

    /// Accumulator for block rewards during an era. It is reset at every new era.
    #[pallet::storage]
    #[pallet::getter(fn reward_accumulator)]
    pub type RewardAccumulator<T> = StorageValue<_, RewardInfo<BalanceOf<T>>, ValueQuery>;

    /// Stores the block number of when the next era starts.
    #[pallet::storage]
    #[pallet::getter(fn next_era_starting_block)]
    pub type NextEraStartingBlock<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Simple map where DaoId points to the respective dao information.
    #[pallet::storage]
    #[pallet::getter(fn dao_info)]
    pub(crate) type RegisteredCore<T: Config> =
        StorageMap<_, Blake2_128Concat, T::DaoId, DaoInfoOf<T>>;

    /// General information about an era.
    #[pallet::storage]
    #[pallet::getter(fn general_era_info)]
    pub type GeneralEraInfo<T: Config> = StorageMap<_, Twox64Concat, Era, EraInfo<BalanceOf<T>>>;

    /// Staking information about a dao in a particular era.
    #[pallet::storage]
    #[pallet::getter(fn dao_stake_info)]
    pub type CoreEraStake<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::DaoId,
        Twox64Concat,
        Era,
        DaoStakeInfo<BalanceOf<T>>,
    >;

    /// Info about staker's stakes on a particular dao.
    #[pallet::storage]
    #[pallet::getter(fn staker_info)]
    pub type GeneralStakerInfo<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::DaoId,
        Blake2_128Concat,
        T::AccountId,
        StakerInfo<BalanceOf<T>>,
        ValueQuery,
    >;

    /// Denotes whether the pallet is halted (disabled).
    #[pallet::storage]
    #[pallet::getter(fn is_halted)]
    pub type Halted<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// Placeholder for the dao being unregistered and its stake info.
    #[pallet::storage]
    #[pallet::getter(fn dao_unregistering_staker_info)]
    pub type UnregisteredCoreStakeInfo<T: Config> =
        StorageMap<_, Blake2_128Concat, T::DaoId, DaoStakeInfo<BalanceOf<T>>, OptionQuery>;

    /// Placeholder for the dao being unregistered and its stakers.
    #[pallet::storage]
    #[pallet::getter(fn dao_unregistering_staker_list)]
    pub type UnregisteredCoreStakers<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::DaoId,
        BoundedVec<T::AccountId, T::MaxStakersPerDao>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(crate) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Account has staked funds to a dao.
        Staked {
            staker: T::AccountId,
            dao: T::DaoId,
            amount: BalanceOf<T>,
        },

        /// Account has unstaked funds from a dao.
        Unstaked {
            staker: T::AccountId,
            dao: T::DaoId,
            amount: BalanceOf<T>,
        },

        /// Account has withdrawn unbonded funds.
        Withdrawn {
            staker: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// New dao registered for staking.
        DaoRegistered { dao: T::DaoId },

        /// DAO unregistered.
        DaoUnregistered { dao: T::DaoId },

        /// Beginning of a new era.
        NewEra { era: u32 },

        /// Staker claimed rewards.
        StakerClaimed {
            staker: T::AccountId,
            dao: T::DaoId,
            era: u32,
            amount: BalanceOf<T>,
        },

        /// Rewards claimed for dao.
        DaoClaimed {
            dao: T::DaoId,
            destination_account: T::AccountId,
            era: u32,
            amount: BalanceOf<T>,
        },

        /// Halt status changed.
        HaltChanged { is_halted: bool },

        /// DAO metadata changed.
        MetadataChanged {
            dao: T::DaoId,
            old_metadata: DaoMetadata<Vec<u8>, Vec<u8>, Vec<u8>>,
            new_metadata: DaoMetadata<Vec<u8>, Vec<u8>, Vec<u8>>,
        },

        /// Staker moved an amount of stake to another dao.
        StakeMoved {
            staker: T::AccountId,
            from_dao: T::DaoId,
            to_dao: T::DaoId,
            amount: BalanceOf<T>,
        },
        /// DAO is being unregistered.
        DaoUnregistrationQueueStarted { dao: T::DaoId },
        /// DAO ungregistration chunk was processed.
        DaoUnregistrationChunksProcessed {
            dao: T::DaoId,
            accounts_processed_in_this_chunk: u64,
            accounts_left: u64,
        },
        /// Sharded execution of the dao unregistration process finished.
        DaoUnregistrationQueueFinished { dao: T::DaoId },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Staking nothing.
        StakingNothing,
        /// Attempted to stake less than the minimum amount.
        InsufficientBalance,
        /// Maximum number of stakers reached.
        MaxStakersReached,
        /// DAO not found.
        DaoNotFound,
        /// No stake available for withdrawal.
        NoStakeAvailable,
        /// Unclaimed rewards available.
        UnclaimedRewardsAvailable,
        /// Unstaking nothing.
        UnstakingNothing,
        /// Nothing available for withdrawal.
        NothingToWithdraw,
        /// DAO already registered.
        DaoAlreadyRegistered,
        /// Unknown rewards for era.
        UnknownEraReward,
        /// Unexpected stake info for era.
        UnexpectedStakeInfoEra,
        /// Too many unlocking chunks.
        TooManyUnlockingChunks,
        /// Reward already claimed.
        RewardAlreadyClaimed,
        /// Incorrect era.
        IncorrectEra,
        /// Too many era stake values.
        TooManyEraStakeValues,
        /// Not a staker.
        NotAStaker,
        /// No permission.
        NoPermission,
        /// Name exceeds maximum length.
        MaxNameExceeded,
        /// Description exceeds maximum length.
        MaxDescriptionExceeded,
        /// Image URL exceeds maximum length.
        MaxImageExceeded,
        /// DAO not registered.
        NotRegistered,
        /// Halted.
        Halted,
        /// No halt change.
        NoHaltChange,
        /// Attempted to move stake to the same dao.
        MoveStakeToSameDao,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            // If the pallet is halted we don't process a new era.
            if Self::is_halted() {
                return T::DbWeight::get().reads(1);
            }

            let previous_era = Self::current_era();
            let next_era_starting_block = Self::next_era_starting_block();

            // Process a new era if past the start block of next era or if this is the first ever era.
            if now >= next_era_starting_block || previous_era.is_zero() {
                let blocks_per_era = T::BlocksPerEra::get();
                let next_era = previous_era + 1;
                CurrentEra::<T>::put(next_era);

                NextEraStartingBlock::<T>::put(now + blocks_per_era);

                let reward = RewardAccumulator::<T>::take();
                let (consumed_weight, new_active_stake) = Self::rotate_staking_info(previous_era);
                Self::reward_balance_snapshot(previous_era, reward, new_active_stake);

                Self::deposit_event(Event::<T>::NewEra { era: next_era });

                consumed_weight + T::DbWeight::get().reads_writes(5, 3)
            } else {
                T::DbWeight::get().reads(3)
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
            From<<T as frame_system::Config>::RuntimeOrigin>,
        T::AccountId: From<[u8; 32]>,
    {
        /// Used to register dao for staking.
        ///
        /// The origin has to be the dao origin.
        ///
        /// As part of this call, `RegisterDeposit` will be reserved from the dao account.
        ///
        /// - `name`: Name of the dao.
        /// - `description`: Description of the dao.
        /// - `image`: Image URL of the dao.
        #[pallet::call_index(0)]
        #[pallet::weight(
            <T as Config>::WeightInfo::register_dao(
                name.len() as u32,
                description.len() as u32,
                image.len() as u32
            )
        )]
        pub fn register_dao(
            origin: OriginFor<T>,
            name: BoundedVec<u8, T::MaxNameLength>,
            description: BoundedVec<u8, T::MaxDescriptionLength>,
            image: BoundedVec<u8, T::MaxImageUrlLength>,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_not_halted()?;

            let dao = ensure_multisig::<T, OriginFor<T>>(origin)?;
            let dao_account = dao.to_account_id();
            let dao_id = dao.id;

            ensure!(
                !RegisteredCore::<T>::contains_key(dao_id),
                Error::<T>::DaoAlreadyRegistered,
            );

            let metadata: DaoMetadataOf<T> = DaoMetadata {
                name,
                description,
                image,
            };

            <T as pallet::Config>::Currency::reserve(&dao_account, T::RegisterDeposit::get())?;

            RegisteredCore::<T>::insert(
                dao_id,
                DaoInfo {
                    account: dao_account,
                    metadata,
                },
            );

            Self::deposit_event(Event::<T>::DaoRegistered { dao: dao_id });

            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Unregister existing dao for staking, making it ineligible for rewards from current era onwards and
        /// starts the unbonding period for the stakers.
        ///
        /// The origin has to be the dao origin.
        ///
        /// Deposit is returned to the dao account.
        ///
        /// - `dao_id`: Id of the dao to be unregistered.
        #[pallet::call_index(1)]
        #[pallet::weight(
            <T as Config>::WeightInfo::unregister_dao()
        )]
        pub fn unregister_dao(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            Self::ensure_not_halted()?;

            let dao = ensure_multisig::<T, OriginFor<T>>(origin.clone())?;
            let dao_account = dao.to_account_id();
            let dao_id = dao.id;

            ensure!(
                RegisteredCore::<T>::get(dao_id).is_some(),
                Error::<T>::NotRegistered
            );

            let current_era = Self::current_era();

            let all_stakers: BoundedVec<T::AccountId, T::MaxStakersPerDao> =
                BoundedVec::truncate_from(
                    GeneralStakerInfo::<T>::iter_key_prefix(dao_id).collect::<Vec<T::AccountId>>(),
                );

            let all_fee = <T as Config>::WeightToFee::weight_to_fee(
                &(all_stakers.len() as u32 * <T as Config>::WeightInfo::unstake()),
            );

            UnregisteredCoreStakers::<T>::insert(dao_id, all_stakers);

            let mut dao_stake_info = Self::dao_stake_info(dao_id, current_era).unwrap_or_default();
            UnregisteredCoreStakeInfo::<T>::insert(dao_id, dao_stake_info.clone());
            GeneralEraInfo::<T>::mutate(current_era, |value| {
                if let Some(x) = value {
                    x.staked = x.staked.saturating_sub(dao_stake_info.total);
                }
            });
            dao_stake_info.total = Zero::zero();
            CoreEraStake::<T>::insert(dao_id, current_era, dao_stake_info.clone());

            let reserve_deposit = T::RegisterDeposit::get();
            <T as Config>::Currency::unreserve(&dao_account, reserve_deposit);

            T::OnUnbalanced::on_unbalanced(<T as Config>::Currency::withdraw(
                &dao_account,
                reserve_deposit.min(all_fee),
                WithdrawReasons::TRANSACTION_PAYMENT,
                ExistenceRequirement::KeepAlive,
            )?);

            RegisteredCore::<T>::remove(dao_id);

            let total_stakers = dao_stake_info.number_of_stakers;

            let message = primitives::UnregisterMessage::<T> {
                dao_id,
                era: current_era,
                stakers_to_unstake: total_stakers,
            }
            .encode();

            T::StakingMessage::handle_message(BoundedSlice::truncate_from(message.as_slice()));

            Self::deposit_event(Event::<T>::DaoUnregistrationQueueStarted { dao: dao_id });

            Self::deposit_event(Event::<T>::DaoUnregistered { dao: dao_id });

            Ok(Some(<T as Config>::WeightInfo::unregister_dao()).into())
        }

        /// Used to change the metadata of a dao.
        ///
        /// The origin has to be the dao origin.
        ///
        /// - `name`: Name of the dao.
        /// - `description`: Description of the dao.
        /// - `image`: Image URL of the dao.
        #[pallet::call_index(2)]
        #[pallet::weight(
            <T as Config>::WeightInfo::change_dao_metadata(
                name.len() as u32,
                description.len() as u32,
                image.len() as u32
            )
        )]
        pub fn change_dao_metadata(
            origin: OriginFor<T>,
            name: BoundedVec<u8, T::MaxNameLength>,
            description: BoundedVec<u8, T::MaxDescriptionLength>,
            image: BoundedVec<u8, T::MaxImageUrlLength>,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_not_halted()?;

            let dao_origin = ensure_multisig::<T, OriginFor<T>>(origin)?;
            let dao_id = dao_origin.id;

            RegisteredCore::<T>::try_mutate(dao_id, |dao| {
                let mut new_dao = dao.take().ok_or(Error::<T>::NotRegistered)?;

                let new_metadata: DaoMetadataOf<T> = DaoMetadata {
                    name: name.clone(),
                    description: description.clone(),
                    image: image.clone(),
                };

                let old_metadata = new_dao.metadata;

                new_dao.metadata = new_metadata;

                *dao = Some(new_dao);

                Self::deposit_event(Event::<T>::MetadataChanged {
                    dao: dao_id,
                    old_metadata: DaoMetadata {
                        name: old_metadata.name.into_inner(),
                        description: old_metadata.description.into_inner(),
                        image: old_metadata.image.into_inner(),
                    },
                    new_metadata: DaoMetadata {
                        name: name.to_vec(),
                        description: description.to_vec(),
                        image: image.to_vec(),
                    },
                });

                Ok(().into())
            })
        }

        /// Lock up and stake balance of the origin account.
        ///
        /// `value` must be more than the `minimum_stake` specified by `MinimumStakingAmount`
        /// unless account already has bonded value equal or more than 'minimum_stake'.
        ///
        /// The dispatch origin for this call must be _Signed_ by the staker's account.
        ///
        /// - `dao_id`: Id of the dao to stake towards.
        /// - `value`: Amount to stake.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::stake())]
        pub fn stake(
            origin: OriginFor<T>,
            dao_id: T::DaoId,
            #[pallet::compact] value: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_not_halted()?;

            let staker = ensure_signed(origin)?;

            ensure!(Self::dao_info(dao_id).is_some(), Error::<T>::NotRegistered);

            let mut ledger = Self::ledger(&staker);
            let available_balance = Self::available_staking_balance(&staker, &ledger);
            let value_to_stake = value.min(available_balance);

            ensure!(value_to_stake > Zero::zero(), Error::<T>::StakingNothing);

            let current_era = Self::current_era();
            let mut staking_info = Self::dao_stake_info(dao_id, current_era).unwrap_or_default();
            let mut staker_info = Self::staker_info(dao_id, &staker);

            Self::internal_stake(
                &mut staker_info,
                &mut staking_info,
                value_to_stake,
                current_era,
            )?;

            ledger.locked = ledger.locked.saturating_add(value_to_stake);

            GeneralEraInfo::<T>::mutate(current_era, |value| {
                if let Some(x) = value {
                    x.staked = x.staked.saturating_add(value_to_stake);
                    x.locked = x.locked.saturating_add(value_to_stake);
                }
            });

            Self::update_ledger(&staker, ledger);
            Self::update_staker_info(&staker, dao_id, staker_info);
            CoreEraStake::<T>::insert(dao_id, current_era, staking_info);

            Self::deposit_event(Event::<T>::Staked {
                staker,
                dao: dao_id,
                amount: value_to_stake,
            });
            Ok(().into())
        }

        /// Start unbonding process and unstake balance from the dao.
        ///
        /// The unstaked amount will no longer be eligible for rewards but still won't be unlocked.
        /// User needs to wait for the unbonding period to finish before being able to withdraw
        /// the funds via `withdraw_unstaked` call.
        ///
        /// In case remaining staked balance is below minimum staking amount,
        /// entire stake will be unstaked.
        ///
        /// - `dao_id`: Id of the dao to unstake from.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::unstake())]
        pub fn unstake(
            origin: OriginFor<T>,
            dao_id: T::DaoId,
            #[pallet::compact] value: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_not_halted()?;

            let staker = ensure_signed(origin)?;

            ensure!(value > Zero::zero(), Error::<T>::UnstakingNothing);
            ensure!(Self::dao_info(dao_id).is_some(), Error::<T>::NotRegistered);

            let current_era = Self::current_era();
            let mut staker_info = Self::staker_info(dao_id, &staker);
            let mut dao_stake_info = Self::dao_stake_info(dao_id, current_era).unwrap_or_default();

            let value_to_unstake =
                Self::internal_unstake(&mut staker_info, &mut dao_stake_info, value, current_era)?;

            let mut ledger = Self::ledger(&staker);
            ledger.unbonding_info.add(UnlockingChunk {
                amount: value_to_unstake,
                unlock_era: current_era + T::UnbondingPeriod::get(),
            });

            ensure!(
                ledger.unbonding_info.len() <= T::MaxUnlocking::get(),
                Error::<T>::TooManyUnlockingChunks
            );

            Self::update_ledger(&staker, ledger);

            GeneralEraInfo::<T>::mutate(current_era, |value| {
                if let Some(x) = value {
                    x.staked = x.staked.saturating_sub(value_to_unstake);
                }
            });
            Self::update_staker_info(&staker, dao_id, staker_info);
            CoreEraStake::<T>::insert(dao_id, current_era, dao_stake_info);

            Self::deposit_event(Event::<T>::Unstaked {
                staker,
                dao: dao_id,
                amount: value_to_unstake,
            });

            Ok(().into())
        }

        /// Withdraw all funds that have completed the unbonding process.
        ///
        /// If there are unbonding chunks which will be fully unbonded in future eras,
        /// they will remain and can be withdrawn later.
        ///
        /// The dispatch origin for this call must be _Signed_ by the staker's account.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_unstaked())]
        pub fn withdraw_unstaked(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            Self::ensure_not_halted()?;

            let staker = ensure_signed(origin)?;

            let mut ledger = Self::ledger(&staker);
            let current_era = Self::current_era();

            let (valid_chunks, future_chunks) = ledger.unbonding_info.partition(current_era);
            let withdraw_amount = valid_chunks.sum();

            ensure!(!withdraw_amount.is_zero(), Error::<T>::NothingToWithdraw);

            ledger.locked = ledger.locked.saturating_sub(withdraw_amount);
            ledger.unbonding_info = future_chunks;

            Self::update_ledger(&staker, ledger);
            GeneralEraInfo::<T>::mutate(current_era, |value| {
                if let Some(x) = value {
                    x.locked = x.locked.saturating_sub(withdraw_amount)
                }
            });

            Self::deposit_event(Event::<T>::Withdrawn {
                staker,
                amount: withdraw_amount,
            });

            Ok(().into())
        }

        /// Claim the staker's rewards.
        ///
        /// In case reward cannot be claimed or was already claimed, an error is raised.
        ///
        /// The dispatch origin for this call must be _Signed_ by the staker's account.
        ///
        /// - `dao_id`: Id of the dao to claim rewards from.
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::staker_claim_rewards())]
        pub fn staker_claim_rewards(
            origin: OriginFor<T>,
            dao_id: T::DaoId,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_not_halted()?;

            let staker = ensure_signed(origin)?;

            let mut staker_info = Self::staker_info(dao_id, &staker);
            let (era, staked) = staker_info.claim();
            ensure!(staked > Zero::zero(), Error::<T>::NoStakeAvailable);

            let current_era = Self::current_era();
            ensure!(era < current_era, Error::<T>::IncorrectEra);

            let staking_info = Self::dao_stake_info(dao_id, era).unwrap_or_default();

            let mut staker_reward = Zero::zero();

            if staking_info.total > Zero::zero() {
                let reward_and_stake =
                    Self::general_era_info(era).ok_or(Error::<T>::UnknownEraReward)?;

                let (_, stakers_joint_reward) =
                    Self::dao_stakers_split(&staking_info, &reward_and_stake);
                staker_reward =
                    Perbill::from_rational(staked, staking_info.total) * stakers_joint_reward;

                let reward_imbalance = <T as pallet::Config>::Currency::withdraw(
                    &Self::account_id(),
                    staker_reward,
                    WithdrawReasons::TRANSFER,
                    ExistenceRequirement::AllowDeath,
                )?;

                <T as pallet::Config>::Currency::resolve_creating(&staker, reward_imbalance);
                Self::update_staker_info(&staker, dao_id, staker_info);
            }

            Self::deposit_event(Event::<T>::StakerClaimed {
                staker,
                dao: dao_id,
                era,
                amount: staker_reward,
            });

            Ok(().into())
        }

        /// Claim dao reward for the specified era.
        ///
        /// In case reward cannot be claimed or was already claimed, an error is raised.
        ///
        /// - `dao_id`: Id of the dao to claim rewards from.
        /// - `era`: Era for which rewards are to be claimed.
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::dao_claim_rewards())]
        pub fn dao_claim_rewards(
            origin: OriginFor<T>,
            dao_id: T::DaoId,
            #[pallet::compact] era: Era,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_not_halted()?;

            ensure_signed(origin)?;

            let current_era = Self::current_era();
            ensure!(era < current_era, Error::<T>::IncorrectEra);

            let mut dao_stake_info = Self::dao_stake_info(dao_id, era).unwrap_or_default();
            ensure!(
                !dao_stake_info.reward_claimed,
                Error::<T>::RewardAlreadyClaimed,
            );
            ensure!(
                dao_stake_info.total > Zero::zero(),
                Error::<T>::NoStakeAvailable,
            );

            let reward_and_stake =
                Self::general_era_info(era).ok_or(Error::<T>::UnknownEraReward)?;

            let (reward, _) = Self::dao_stakers_split(&dao_stake_info, &reward_and_stake);

            let reward_imbalance = <T as pallet::Config>::Currency::withdraw(
                &Self::account_id(),
                reward,
                WithdrawReasons::TRANSFER,
                ExistenceRequirement::AllowDeath,
            )?;

            let dao_account =
                <pallet_dao_manager::Pallet<T> as DaoAccountDerivation<T>>::derive_dao_account(
                    dao_id,
                );

            <T as pallet::Config>::Currency::resolve_creating(&dao_account, reward_imbalance);
            Self::deposit_event(Event::<T>::DaoClaimed {
                dao: dao_id,
                destination_account: dao_account,
                era,
                amount: reward,
            });

            dao_stake_info.reward_claimed = true;
            CoreEraStake::<T>::insert(dao_id, era, dao_stake_info);

            Ok(().into())
        }

        /// Halt or unhalt the pallet.
        ///
        /// The dispatch origin for this call must be _Root_.
        ///
        /// - `halt`: `true` to halt, `false` to unhalt.
        #[pallet::call_index(8)]
        #[pallet::weight((<T as Config>::WeightInfo::halt_unhalt_pallet(), Pays::No))]
        pub fn halt_unhalt_pallet(origin: OriginFor<T>, halt: bool) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let is_halted = Self::is_halted();

            ensure!(is_halted ^ halt, Error::<T>::NoHaltChange);

            Self::internal_halt_unhalt(halt);

            Self::deposit_event(Event::<T>::HaltChanged { is_halted: halt });

            Ok(().into())
        }

        /// Move stake from one dao to another.
        ///
        /// The dispatch origin for this call must be _Signed_ by the staker's account.
        ///
        /// - `from_dao`: Id of the dao to move stake from.
        /// - `amount`: Amount to move.
        /// - `to_dao`: Id of the dao to move stake to.
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::move_stake())]
        pub fn move_stake(
            origin: OriginFor<T>,
            from_dao: T::DaoId,
            #[pallet::compact] amount: BalanceOf<T>,
            to_dao: T::DaoId,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_not_halted()?;

            let staker = ensure_signed(origin)?;

            ensure!(from_dao != to_dao, Error::<T>::MoveStakeToSameDao);
            ensure!(Self::dao_info(to_dao).is_some(), Error::<T>::NotRegistered);

            let current_era = Self::current_era();
            let mut from_staker_info = Self::staker_info(from_dao, &staker);
            let mut from_dao_info = Self::dao_stake_info(from_dao, current_era).unwrap_or_default();

            let unstaked_amount = Self::internal_unstake(
                &mut from_staker_info,
                &mut from_dao_info,
                amount,
                current_era,
            )?;

            let mut to_staker_info = Self::staker_info(to_dao, &staker);
            let mut to_dao_info = Self::dao_stake_info(to_dao, current_era).unwrap_or_default();

            Self::internal_stake(
                &mut to_staker_info,
                &mut to_dao_info,
                unstaked_amount,
                current_era,
            )?;

            CoreEraStake::<T>::insert(from_dao, current_era, from_dao_info);
            Self::update_staker_info(&staker, from_dao, from_staker_info);

            CoreEraStake::<T>::insert(to_dao, current_era, to_dao_info);
            Self::update_staker_info(&staker, to_dao, to_staker_info);

            Self::deposit_event(Event::<T>::StakeMoved {
                staker,
                from_dao,
                to_dao,
                amount: unstaked_amount,
            });

            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Internal function responsible for validating a stake and updating in-place
        /// both the staker's and dao staking info.
        fn internal_stake(
            staker_info: &mut StakerInfo<BalanceOf<T>>,
            staking_info: &mut DaoStakeInfo<BalanceOf<T>>,
            amount: BalanceOf<T>,
            current_era: Era,
        ) -> Result<(), Error<T>> {
            ensure!(
                !staker_info.latest_staked_value().is_zero()
                    || staking_info.number_of_stakers < T::MaxStakersPerDao::get(),
                Error::<T>::MaxStakersReached
            );
            if staker_info.latest_staked_value().is_zero() {
                staking_info.number_of_stakers = staking_info.number_of_stakers.saturating_add(1);
            }

            staker_info
                .stake(current_era, amount)
                .map_err(|_| Error::<T>::UnexpectedStakeInfoEra)?;
            ensure!(
                staker_info.len() < T::MaxEraStakeValues::get(),
                Error::<T>::TooManyEraStakeValues
            );
            ensure!(
                staker_info.latest_staked_value() >= T::MinimumStakingAmount::get(),
                Error::<T>::InsufficientBalance,
            );

            let new_total = staking_info.total.saturating_add(amount);

            staking_info.total = new_total;

            Ok(())
        }

        /// Internal function responsible for validating an unstake and updating in-place
        /// both the staker's and dao staking info.
        fn internal_unstake(
            staker_info: &mut StakerInfo<BalanceOf<T>>,
            dao_stake_info: &mut DaoStakeInfo<BalanceOf<T>>,
            amount: BalanceOf<T>,
            current_era: Era,
        ) -> Result<BalanceOf<T>, Error<T>> {
            let staked_value = staker_info.latest_staked_value();
            ensure!(staked_value > Zero::zero(), Error::<T>::NoStakeAvailable);

            let remaining = staked_value.saturating_sub(amount);
            let value_to_unstake = if remaining < T::MinimumStakingAmount::get() {
                dao_stake_info.number_of_stakers =
                    dao_stake_info.number_of_stakers.saturating_sub(1);
                staked_value
            } else {
                amount
            };

            let new_total = dao_stake_info.total.saturating_sub(value_to_unstake);

            dao_stake_info.total = new_total;

            ensure!(
                value_to_unstake > Zero::zero(),
                Error::<T>::UnstakingNothing
            );

            staker_info
                .unstake(current_era, value_to_unstake)
                .map_err(|_| Error::<T>::UnexpectedStakeInfoEra)?;
            ensure!(
                staker_info.len() < T::MaxEraStakeValues::get(),
                Error::<T>::TooManyEraStakeValues
            );

            Ok(value_to_unstake)
        }

        pub(crate) fn account_id() -> T::AccountId {
            T::PotId::get().into_account_truncating()
        }

        /// Update the ledger for a staker. This will also update the stash lock.
        /// This lock will lock the entire funds except paying for further transactions.
        fn update_ledger(staker: &T::AccountId, ledger: AccountLedger<BalanceOf<T>>) {
            if ledger.is_empty() {
                Ledger::<T>::remove(staker);
                <T as pallet::Config>::Currency::remove_lock(LOCK_ID, staker);
            } else {
                <T as pallet::Config>::Currency::set_lock(
                    LOCK_ID,
                    staker,
                    ledger.locked,
                    WithdrawReasons::all(),
                );
                Ledger::<T>::insert(staker, ledger);
            }
        }

        /// The block rewards are accumulated on the pallet's account during an era.
        /// This function takes a snapshot of the pallet's balance accrued during current era
        /// and stores it for future distribution
        ///
        /// This is called just at the beginning of an era.
        fn reward_balance_snapshot(
            era: Era,
            rewards: RewardInfo<BalanceOf<T>>,
            new_active_stake: BalanceOf<T>,
        ) {
            let mut era_info = Self::general_era_info(era).unwrap_or_default();

            GeneralEraInfo::<T>::insert(
                era + 1,
                EraInfo {
                    rewards: Default::default(),
                    staked: era_info.staked,
                    active_stake: new_active_stake,
                    locked: era_info.locked,
                },
            );

            era_info.rewards = rewards;
            era_info.active_stake = new_active_stake;

            GeneralEraInfo::<T>::insert(era, era_info);
        }

        /// Adds `stakers` and `DAOs` rewards to the reward pool.
        ///
        /// - `inflation`: Total inflation for the era.
        pub fn rewards(inflation: NegativeImbalanceOf<T>) {
            let (dao_part, stakers_part) = <T as Config>::RewardRatio::get();

            let (dao, stakers) = inflation.ration(dao_part, stakers_part);

            RewardAccumulator::<T>::mutate(|accumulated_reward| {
                accumulated_reward.dao = accumulated_reward.dao.saturating_add(dao.peek());
                accumulated_reward.stakers =
                    accumulated_reward.stakers.saturating_add(stakers.peek());
            });

            <T as pallet::Config>::Currency::resolve_creating(
                &Self::account_id(),
                stakers.merge(dao),
            );
        }

        /// Updates staker info for a dao.
        fn update_staker_info(
            staker: &T::AccountId,
            dao_id: T::DaoId,
            staker_info: StakerInfo<BalanceOf<T>>,
        ) {
            if staker_info.is_empty() {
                GeneralStakerInfo::<T>::remove(dao_id, staker)
            } else {
                GeneralStakerInfo::<T>::insert(dao_id, staker, staker_info)
            }
        }

        /// Returns available staking balance for the potential staker.
        fn available_staking_balance(
            staker: &T::AccountId,
            ledger: &AccountLedger<BalanceOf<T>>,
        ) -> BalanceOf<T> {
            let free_balance = <T as pallet::Config>::Currency::free_balance(staker)
                .saturating_sub(<T as pallet::Config>::ExistentialDeposit::get());

            free_balance.saturating_sub(ledger.locked)
        }

        /// Returns total value locked by staking.
        ///
        /// Note that this can differ from _total staked value_ since some funds might be undergoing the unbonding period.
        pub fn tvl() -> BalanceOf<T> {
            let current_era = Self::current_era();
            if let Some(era_info) = Self::general_era_info(current_era) {
                era_info.locked
            } else {
                Zero::zero()
            }
        }

        /// Calculate reward split between dao and stakers.
        ///
        /// Returns (DAO reward, joint stakers reward)
        pub(crate) fn dao_stakers_split(
            dao_info: &DaoStakeInfo<BalanceOf<T>>,
            era_info: &EraInfo<BalanceOf<T>>,
        ) -> (BalanceOf<T>, BalanceOf<T>) {
            let dao_stake_portion = if dao_info.active {
                Perbill::from_rational(dao_info.total, era_info.active_stake)
            } else {
                Perbill::zero()
            };
            let stakers_stake_portion = Perbill::from_rational(dao_info.total, era_info.staked);

            let dao_reward_part = dao_stake_portion * era_info.rewards.dao;
            let stakers_joint_reward = stakers_stake_portion * era_info.rewards.stakers;

            (dao_reward_part, stakers_joint_reward)
        }

        /// Used to copy all `DaoStakeInfo` from the ending era over to the next era.
        fn rotate_staking_info(current_era: Era) -> (Weight, BalanceOf<T>) {
            let next_era = current_era + 1;

            let mut consumed_weight = Weight::zero();

            let mut new_active_stake: BalanceOf<T> = Zero::zero();

            for dao_id in RegisteredCore::<T>::iter_keys() {
                consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));

                if let Some(mut staking_info) = Self::dao_stake_info(dao_id, current_era) {
                    if staking_info.total >= <T as Config>::StakeThresholdForActiveDao::get() {
                        staking_info.active = true;
                        new_active_stake += staking_info.total;
                    } else {
                        staking_info.active = false;
                    }

                    staking_info.reward_claimed = false;
                    CoreEraStake::<T>::insert(dao_id, next_era, staking_info);

                    consumed_weight =
                        consumed_weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
                } else {
                    consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));
                }
            }

            (consumed_weight, new_active_stake)
        }

        /// Sets the halt state of the pallet.
        pub fn internal_halt_unhalt(halt: bool) {
            Halted::<T>::put(halt);
        }

        /// Ensure the pallet is not halted.
        pub fn ensure_not_halted() -> Result<(), Error<T>> {
            if Self::is_halted() {
                Err(Error::<T>::Halted)
            } else {
                Ok(())
            }
        }

        /// Sharded execution of the dao unregistration process.
        ///
        /// This function is called by the [`ProcessMessage`] trait implemented in [`primitives::ProcessUnregistrationMessages`]
        pub(crate) fn process_dao_unregistration_shard(
            stakers: u32,
            dao_id: T::DaoId,
            start_era: Era,
            chunk_size: u64,
        ) -> DispatchResultWithPostInfo {
            let mut staker_info_prefix =
                Self::dao_unregistering_staker_list(dao_id).unwrap_or_default();

            let mut corrected_staker_length_fee = Zero::zero();

            let mut dao_stake_info =
                Self::dao_unregistering_staker_info(dao_id).unwrap_or_default();

            let mut unsteked_count: u64 = 0;
            while let Some(staker) = staker_info_prefix.pop() {
                let mut staker_info = Self::staker_info(dao_id, &staker);

                let latest_staked_value = staker_info.latest_staked_value();

                if let Ok(value_to_unstake) = Self::internal_unstake(
                    &mut staker_info,
                    &mut dao_stake_info,
                    latest_staked_value,
                    start_era,
                ) {
                    UnregisteredCoreStakeInfo::<T>::insert(dao_id, dao_stake_info.clone());
                    let mut ledger = Self::ledger(&staker);
                    ledger.unbonding_info.add(UnlockingChunk {
                        amount: value_to_unstake,
                        unlock_era: start_era + T::UnbondingPeriod::get(),
                    });

                    ensure!(
                        ledger.unbonding_info.len() <= T::MaxUnlocking::get(),
                        Error::<T>::TooManyUnlockingChunks
                    );

                    Self::update_ledger(&staker, ledger);

                    Self::update_staker_info(&staker, dao_id, staker_info);

                    Self::deposit_event(Event::<T>::Unstaked {
                        staker: staker.clone(),
                        dao: dao_id,
                        amount: value_to_unstake,
                    });
                    corrected_staker_length_fee += <T as Config>::WeightInfo::unstake();
                } else {
                    // if the staker has moved or already unstaked `internal_unstake` will do one read and return err.
                    corrected_staker_length_fee += T::DbWeight::get().reads(1);
                }

                unsteked_count += 1;

                if unsteked_count >= chunk_size {
                    let total_remaning_stakers = stakers.saturating_sub(unsteked_count as u32);
                    let message: Vec<u8> = primitives::UnregisterMessage::<T> {
                        dao_id,
                        stakers_to_unstake: total_remaning_stakers,
                        era: start_era,
                    }
                    .encode();

                    T::StakingMessage::handle_message(BoundedSlice::truncate_from(
                        message.as_slice(),
                    ));

                    Self::deposit_event(Event::<T>::DaoUnregistrationChunksProcessed {
                        dao: dao_id,
                        accounts_processed_in_this_chunk: unsteked_count,
                        accounts_left: total_remaning_stakers as u64,
                    });
                    UnregisteredCoreStakeInfo::<T>::insert(dao_id, dao_stake_info.clone());
                    UnregisteredCoreStakers::<T>::insert(dao_id, staker_info_prefix);

                    return Ok(Some(corrected_staker_length_fee).into());
                }
            }

            Self::deposit_event(Event::<T>::DaoUnregistrationQueueFinished { dao: dao_id });
            UnregisteredCoreStakers::<T>::remove(dao_id);
            UnregisteredCoreStakeInfo::<T>::remove(dao_id);

            Ok(Some(corrected_staker_length_fee).into())
        }
    }
}

impl<T: Config> QueuePausedQuery<T> for Pallet<T> {
    fn is_paused(_origin: &T) -> bool {
        Pallet::<T>::is_halted()
    }
}

pub type MessageOriginOf<T> =
    <<T as pallet_message_queue::Config>::MessageProcessor as ProcessMessage>::Origin;
