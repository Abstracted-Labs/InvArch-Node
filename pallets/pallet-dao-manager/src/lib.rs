//! # Pallet dao_manager
//!
//! - [`Config`]
//! - [`Call`]
//! - [`Pallet`]
//!
//! ## Overview
//!
//! This pallet handles advanced virtual multisigs (DAOs).
//!
//! Lower level implementation details of this pallet's calls are contained in separate modules, each of them
//! containing their own docs.
//!
//! ### Pallet Functions
//!
//! - `create_dao` - Create a new dao
//! - `token_mint` - Mint the DAO's voting token to a target (called by a DAO origin)
//! - `token_burn` - Burn the DAO's voting token from a target (called by a DAO origin)
//! - `operate_multisig` - Create a new multisig proposal, auto-executing if caller passes execution threshold requirements
//! - `vote_multisig` - Vote on an existing multisig proposal, auto-executing if caller puts vote tally past execution threshold requirements
//! - `withdraw_vote_multisig` - Remove caller's vote from an existing multisig proposal
//! - `cancel_multisig_proposal` - Cancel an existing multisig proposal (called by a DAO origin)
//! - `set_parameters` - Change DAO parameters incl. voting thresholds and token freeze state (called by a DAO origin)

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod tests;

//pub mod migrations;

pub mod account_derivation;
pub mod dao_manager_core;
mod dispatch;
pub mod fee_handling;
mod lookup;
pub mod multisig;
pub mod origin;
pub mod voting;
pub mod weights;

pub use account_derivation::DaoAccountDerivation;
use fee_handling::FeeAsset;
pub use lookup::DaoLookup;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use core::iter::Sum;

    use crate::{
        fee_handling::MultisigFeeHandler,
        voting::{Tally, VoteRecord},
    };

    use super::*;
    use codec::FullCodec;
    use frame_support::{
        dispatch::{GetDispatchInfo, Pays, PostDispatchInfo},
        pallet_prelude::*,
        traits::{
            fungibles,
            fungibles::{Balanced, Inspect},
            Currency, Get, GetCallMetadata, ReservableCurrency,
        },
        transactional,
        weights::WeightToFee,
        Parameter,
    };
    use frame_system::{pallet_prelude::*, RawOrigin};
    use primitives::DaoInfo;
    use scale_info::prelude::fmt::Display;
    use sp_runtime::{
        traits::{AtLeast32BitUnsigned, Dispatchable, Member},
        Perbill,
    };
    use sp_std::{boxed::Box, convert::TryInto, vec::Vec};

    pub use super::{dao_manager_core, multisig};

    use crate::origin::DaoOrigin;

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    pub type DaoInfoOf<T> =
        DaoInfo<<T as frame_system::Config>::AccountId, dao_manager_core::DaoMetadataOf<T>>;

    pub type CallOf<T> = <T as Config>::RuntimeCall;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {
        /// Runtime event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Integer id type for the dao id
        type DaoId: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + Display
            + MaxEncodedLen
            + Clone
            + Into<u32>;

        /// Currency type
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// The overarching call type
        type RuntimeCall: Parameter
            + Dispatchable<
                Info = frame_support::dispatch::DispatchInfo,
                RuntimeOrigin = <Self as pallet::Config>::RuntimeOrigin,
                PostInfo = PostDispatchInfo,
            > + GetDispatchInfo
            + From<frame_system::Call<Self>>
            + GetCallMetadata
            + FullCodec;

        /// The maximum numbers of caller accounts on a single multisig proposal
        #[pallet::constant]
        type MaxCallers: Get<u32>;

        /// The maximum length of the dao metadata and the metadata of multisig proposals
        #[pallet::constant]
        type MaxMetadata: Get<u32>;

        /// The outer `Origin` type.
        type RuntimeOrigin: From<Origin<Self>>
            + From<<Self as frame_system::Config>::RuntimeOrigin>
            + From<RawOrigin<<Self as frame_system::Config>::AccountId>>;

        /// Base voting token balance to give callers when creating a DAO
        #[pallet::constant]
        type DaoSeedBalance: Get<BalanceOf<Self>>;

        /// Fee for creating a dao in the native token
        #[pallet::constant]
        type DaoCreationFee: Get<BalanceOf<Self>>;

        /// Fee for creating a dao in the relay token
        #[pallet::constant]
        type RelayDaoCreationFee: Get<
            <<Self as Config>::Tokens as Inspect<<Self as frame_system::Config>::AccountId>>::Balance,
        >;

        /// Relay token asset id in the runtime
        #[pallet::constant]
        type RelayAssetId: Get<<<Self as Config>::Tokens as Inspect<<Self as frame_system::Config>::AccountId>>::AssetId>;

        /// Provider of assets functionality for the voting tokens
        type AssetsProvider: fungibles::Inspect<Self::AccountId, Balance = BalanceOf<Self>, AssetId = Self::DaoId>
            + fungibles::Mutate<Self::AccountId, AssetId = Self::DaoId>;

        /// Provider of balance tokens in the runtime
        type Tokens: Balanced<Self::AccountId> + Inspect<Self::AccountId>;

        /// Implementation of the fee handler for both dao creation fee and multisig call fees
        type FeeCharger: MultisigFeeHandler<Self>;

        /// ParaId of the parachain, to be used for deriving the dao account id
        type ParaId: Get<u32>;

        /// Maximum size of a multisig proposal call
        #[pallet::constant]
        type MaxCallSize: Get<u32>;

        /// Weight info for dispatchable calls
        type WeightInfo: WeightInfo;

        /// Byte to fee conversion provider, from pallet_transaction_payment.
        type LengthToFee: WeightToFee<Balance = BalanceOf<Self>>;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    /// The custom dao origin.
    #[pallet::origin]
    pub type Origin<T> = DaoOrigin<T>;

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// Next available DAO ID.
    #[pallet::storage]
    #[pallet::getter(fn next_dao_id)]
    pub type NextCoreId<T: Config> = StorageValue<_, T::DaoId, ValueQuery>;

    /// DAO info storage.
    #[pallet::storage]
    #[pallet::getter(fn dao_storage)]
    pub type CoreStorage<T: Config> = StorageMap<_, Blake2_128Concat, T::DaoId, DaoInfoOf<T>>;

    /// Mapping of account id -> dao id.
    #[pallet::storage]
    #[pallet::getter(fn dao_by_account)]
    pub type CoreByAccount<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::DaoId>;

    /// Details of a multisig call.
    ///
    /// Key: (Dao ID, call hash)
    #[pallet::storage]
    #[pallet::getter(fn multisig)]
    pub type Multisig<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::DaoId,
        Blake2_128Concat,
        T::Hash,
        crate::multisig::MultisigOperationOf<T>,
    >;

    /// Stores a list of members for each DAO.
    /// This storage should be always handled by the runtime and mutated by CoreAssets hooks.
    // We make this a StorageDoubleMap so we don't have to bound the list.
    #[pallet::storage]
    #[pallet::getter(fn dao_members)]
    pub type CoreMembers<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::DaoId, Blake2_128Concat, T::AccountId, ()>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(crate) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A dao was created
        DaoCreated {
            dao_account: T::AccountId,
            dao_id: T::DaoId,
            metadata: Vec<u8>,
            minimum_support: Perbill,
            required_approval: Perbill,
        },

        /// A dao had parameters changed
        ParametersSet {
            dao_id: T::DaoId,
            metadata: Option<Vec<u8>>,
            minimum_support: Option<Perbill>,
            required_approval: Option<Perbill>,
            frozen_tokens: Option<bool>,
        },

        /// A dao's voting token was minted
        Minted {
            dao_id: T::DaoId,
            target: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// A dao's voting token was burned
        Burned {
            dao_id: T::DaoId,
            target: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// A multisig proposal has started, it needs more votes to pass
        MultisigVoteStarted {
            dao_id: T::DaoId,
            executor_account: T::AccountId,
            voter: T::AccountId,
            votes_added: VoteRecord<T>,
            call_hash: T::Hash,
        },

        /// A vote was added to an existing multisig proposal
        MultisigVoteAdded {
            dao_id: T::DaoId,
            executor_account: T::AccountId,
            voter: T::AccountId,
            votes_added: VoteRecord<T>,
            current_votes: Tally<T>,
            call_hash: T::Hash,
        },

        /// A vote was removed from an existing multisig proposal
        MultisigVoteWithdrawn {
            dao_id: T::DaoId,
            executor_account: T::AccountId,
            voter: T::AccountId,
            votes_removed: VoteRecord<T>,
            call_hash: T::Hash,
        },

        /// A multisig proposal passed and it's call was executed
        MultisigExecuted {
            dao_id: T::DaoId,
            executor_account: T::AccountId,
            voter: T::AccountId,
            call_hash: T::Hash,
            call: CallOf<T>,
            result: DispatchResult,
        },

        /// A multisig proposal was cancelled
        MultisigCanceled {
            dao_id: T::DaoId,
            call_hash: T::Hash,
        },
    }

    /// Errors for dao_manager pallet
    #[pallet::error]
    pub enum Error<T> {
        /// No available DAO ID
        NoAvailableDaoId,
        /// DAO not found
        DaoNotFound,
        /// The caller has no permissions in the DAO
        NoPermission,
        /// Maximum metadata length exceeded
        MaxMetadataExceeded,
        /// Maximum amount of callers exceeded
        MaxCallersExceeded,
        /// Multisig call not found
        MultisigCallNotFound,
        /// Failed to decode stored multisig call
        FailedDecodingCall,
        /// Multisig proposal already exists and is being voted on
        MultisigCallAlreadyExists,
        /// Cannot withdraw a vote on a multisig transaction you have not voted on
        NotAVoter,
        /// Failed to extract metadata from a call
        CallHasTooFewBytes,
        /// Incomplete vote cleanup
        IncompleteVoteCleanup,
        /// Multisig fee payment failed, probably due to lack of funds to pay for fees
        CallFeePaymentFailed,
        /// Call is too long
        MaxCallLengthExceeded,
    }

    /// Dispatch functions
    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        Result<
            DaoOrigin<T>,
            <T as frame_system::Config>::RuntimeOrigin,
        >: From<<T as frame_system::Config>::RuntimeOrigin>,
        <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance: Sum,
    <T as frame_system::Config>::AccountId: From<[u8; 32]>,
    {
        /// Create a new DAO
        /// - `metadata`: Arbitrary byte vec to be attached to the dao info
        /// - `minimum_support`: Minimum amount of positive votes out of total token supply required to approve a proposal
        /// - `required_approval`: Minimum amount of positive votes out of current positive + negative votes required to approve a proposal
        /// - `creation_fee_asset`: Token to be used to pay the dao creation fee
        #[pallet::call_index(0)]
        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::create_dao(metadata.len() as u32))]
        pub fn create_dao(
            owner: OriginFor<T>,
            metadata: BoundedVec<u8, T::MaxMetadata>,
            minimum_support: Perbill,
            required_approval: Perbill,
            creation_fee_asset: FeeAsset,
        ) -> DispatchResultWithPostInfo {
            Pallet::<T>::inner_create_dao(
                owner,
                metadata,
                minimum_support,
                required_approval,
                creation_fee_asset,
            )?;

            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Mint the dao's voting token to a target (called by a dao origin)
        /// - `amount`: Balance amount
        /// - `target`: Account receiving the minted tokens
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::token_mint())]
        pub fn token_mint(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            target: T::AccountId,
        ) -> DispatchResult {
            Pallet::<T>::inner_token_mint(origin, amount, target)
        }

        /// Burn the dao's voting token from a target (called by a dao origin)
        /// - `amount`: Balance amount
        /// - `target`: Account having tokens burned
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::token_burn())]
        pub fn token_burn(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            target: T::AccountId,
        ) -> DispatchResult {
            Pallet::<T>::inner_token_burn(origin, amount, target)
        }

        /// Create a new multisig proposal, auto-executing if caller passes execution threshold requirements
        /// Fees are calculated using the length of the metadata and the call
        /// The proposed call's weight is used internally to charge the multisig instead of the user proposing the call
        /// - `dao_id`: Id of the dao to propose the call in
        /// - `metadata`: Arbitrary byte vec to be attached to the proposal
        /// - `fee_asset`: Token to be used by the multisig to pay for call fees
        /// - `call`: The actual call to be proposed
        #[pallet::call_index(3)]
        #[pallet::weight(
            <T as Config>::WeightInfo::operate_multisig(
                metadata.clone().map(|m| m.len()).unwrap_or(0) as u32,
                call.using_encoded(|c| c.len() as u32)
            )
        )]
        pub fn operate_multisig(
            caller: OriginFor<T>,
            dao_id: T::DaoId,
            metadata: Option<BoundedVec<u8, T::MaxMetadata>>,
            fee_asset: FeeAsset,
            call: Box<<T as pallet::Config>::RuntimeCall>,
        ) -> DispatchResultWithPostInfo {
            Pallet::<T>::inner_operate_multisig(caller, dao_id, metadata, fee_asset, call)
        }

        /// Vote on an existing multisig proposal, auto-executing if caller puts vote tally past execution threshold requirements
        /// - `dao_id`: Id of the dao where the proposal is
        /// - `call_hash`: Hash of the call identifying the proposal
        /// - `aye`: Wheter or not to vote positively
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::vote_multisig())]
        pub fn vote_multisig(
            caller: OriginFor<T>,
            dao_id: T::DaoId,
            call_hash: T::Hash,
            aye: bool,
        ) -> DispatchResultWithPostInfo {
            Pallet::<T>::inner_vote_multisig(caller, dao_id, call_hash, aye)
        }

        /// Remove caller's vote from an existing multisig proposal
        /// - `dao_id`: Id of the dao where the proposal is
        /// - `call_hash`: Hash of the call identifying the proposal
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_vote_multisig())]
        pub fn withdraw_vote_multisig(
            caller: OriginFor<T>,
            dao_id: T::DaoId,
            call_hash: T::Hash,
        ) -> DispatchResultWithPostInfo {
            Pallet::<T>::inner_withdraw_vote_multisig(caller, dao_id, call_hash)
        }

        /// Cancel an existing multisig proposal (called by a dao origin)
        /// - `call_hash`: Hash of the call identifying the proposal
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::cancel_multisig_proposal())]
        pub fn cancel_multisig_proposal(
            caller: OriginFor<T>,
            call_hash: T::Hash,
        ) -> DispatchResultWithPostInfo {
            Pallet::<T>::inner_cancel_multisig_proposal(caller, call_hash)
        }

        /// Change dao parameters incl. voting thresholds and token freeze state (called by a dao origin)
        /// - `metadata`: Arbitrary byte vec to be attached to the dao info
        /// - `minimum_support`: Minimum amount of positive votes out of total token supply required to approve a proposal
        /// - `required_approval`: Minimum amount of positive votes out of current positive + negative votes required to approve a proposal
        /// - `frozen_tokens`: Wheter or not the dao's voting token should be transferable by the holders
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::set_parameters(
            metadata.clone().map(|m| m.len()).unwrap_or(0) as u32
        ))]
        pub fn set_parameters(
            origin: OriginFor<T>,
            metadata: Option<BoundedVec<u8, T::MaxMetadata>>,
            minimum_support: Option<Perbill>,
            required_approval: Option<Perbill>,
            frozen_tokens: Option<bool>,
        ) -> DispatchResult {
            Pallet::<T>::inner_set_parameters(origin, metadata, minimum_support, required_approval, frozen_tokens)
        }
    }
}
