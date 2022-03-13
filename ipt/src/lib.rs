#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
    pallet_prelude::*,
    traits::{Currency as FSCurrency, Get},
    Parameter,
};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{AtLeast32BitUnsigned, Member};
use sp_std::vec::Vec;

pub use pallet::*;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AssetDetails<Balance, AccountId> {
    owner: AccountId,
    /// The total supply across all accounts.
    supply: Balance,
    /// The balance deposited for this asset. This pays for the data stored here.
    deposit: Balance,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct MultisigOperation<AccountId, Signers, Call> {
    signers: Signers,
    include_original_caller: Option<AccountId>,
    actual_call: Call,
}

#[frame_support::pallet]
pub mod pallet {
    use core::iter::Sum;
    use frame_system::RawOrigin;
    use primitives::utils::multi_account_id;
    use sp_std::convert::TryInto;

    use super::*;
    use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
    use scale_info::prelude::fmt::Display;
    use sp_core::blake2_256;
    use sp_runtime::traits::CheckedSub;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The IPS Pallet Events
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// Currency
        type Currency: FSCurrency<Self::AccountId>;
        /// The units in which we record balances.
        type Balance: Member
            + Parameter
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + MaxEncodedLen
            + TypeInfo
            + Sum<Self::Balance>;

        /// The IPS ID type
        type IptId: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + Display
            + MaxEncodedLen;

        /// The overarching call type.
        type Call: Parameter
            + Dispatchable<Origin = Self::Origin, PostInfo = PostDispatchInfo>
            + GetDispatchInfo
            + From<frame_system::Call<Self>>
            + MaxEncodedLen;

        /// The maximum numbers of caller accounts on a single Multisig call
        #[pallet::constant]
        type MaxCallers: Get<u32> + MaxEncodedLen;

        #[pallet::constant]
        type ExistentialDeposit: Get<Self::Balance>;
    }

    pub type BalanceOf<T> =
        <<T as Config>::Currency as FSCurrency<<T as frame_system::Config>::AccountId>>::Balance;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn multisig)]
    /// Details of a multisig call.
    pub type Multisig<T: Config> =
        StorageMap<_, Blake2_128Concat, (T::IptId, [u8; 32]), MultisigOperationOf<T>>;

    pub type MultisigOperationOf<T> = MultisigOperation<
        <T as frame_system::Config>::AccountId,
        BoundedVec<<T as frame_system::Config>::AccountId, <T as Config>::MaxCallers>,
        <T as pallet::Config>::Call,
    >;

    #[pallet::storage]
    #[pallet::getter(fn ipt)]
    /// Details of an asset.
    pub type Ipt<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::IptId,
        AssetDetails<<T as pallet::Config>::Balance, T::AccountId>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn balance)]
    /// The holdings of a specific account for a specific asset.
    pub type Balance<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::IptId,
        Blake2_128Concat,
        T::AccountId,
        <T as pallet::Config>::Balance,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(fn deposit_event)]
    pub enum Event<T: Config> {
        Minted(T::IptId, T::AccountId, <T as pallet::Config>::Balance),
    }

    /// Errors for IPF pallet
    #[pallet::error]
    pub enum Error<T> {
        IptDoesntExist,
        NoPermission,
        NotEnoughAmount,
        TooManySignatories,
        UnexistentBalance,
        MultisigOperationUninitialized,
        MaxMetadataExceeded,
    }

    /// Dispatch functions
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(100_000)] // TODO: Set correct weight
        pub fn mint(
            owner: OriginFor<T>,
            ips_id: T::IptId,
            amount: <T as pallet::Config>::Balance,
            target: T::AccountId,
        ) -> DispatchResult {
            let owner = ensure_signed(owner)?;

            let ipt = Ipt::<T>::get(ips_id).ok_or(Error::<T>::IptDoesntExist)?;

            ensure!(owner == ipt.owner, Error::<T>::NoPermission);

            Pallet::<T>::internal_mint(target, ips_id, amount)?;

            Self::deposit_event(Event::Minted(ips_id, owner, amount));

            Ok(())
        }

        #[pallet::weight(100_000)]
        pub fn as_multi(
            owner: OriginFor<T>,
            include_caller: bool,
            ips_id: T::IptId,
            call: <T as pallet::Config>::Call,
        ) -> DispatchResultWithPostInfo {
            let owner = ensure_signed(owner)?;
            let ipt = Ipt::<T>::get(ips_id).ok_or(Error::<T>::IptDoesntExist)?;

            //  ensure!(owner == ipt.owner, Error::<T>::NoPermission);

            let total_per_2 = ipt.supply / 2u32.into();

            let owner_balance =
                Balance::<T>::get(ips_id, owner.clone()).ok_or(Error::<T>::NoPermission)?;

            if owner_balance > total_per_2 {
                call.dispatch(
                    RawOrigin::Signed(multi_account_id::<T, T::IptId>(
                        ips_id,
                        if include_caller { Some(owner) } else { None },
                    ))
                    .into(),
                )?;
            } else {
                Multisig::<T>::insert(
                    (ips_id, blake2_256(&call.encode())),
                    MultisigOperation {
                        signers: vec![owner.clone()]
                            .try_into()
                            .map_err(|_| Error::<T>::TooManySignatories)?,
                        include_original_caller: if include_caller { Some(owner) } else { None },
                        actual_call: call,
                    },
                );
            }

            Ok(().into())
        }

        #[pallet::weight(100_000)]
        pub fn approve_as_multi(
            owner: OriginFor<T>,
            ips_id: T::IptId,
            call_hash: [u8; 32],
        ) -> DispatchResultWithPostInfo {
            Multisig::<T>::try_mutate_exists((ips_id, call_hash), |data| {
                let owner = ensure_signed(owner)?;

                let ipt = Ipt::<T>::get(ips_id).ok_or(Error::<T>::IptDoesntExist)?;

                let mut old_data = data
                    .take()
                    .ok_or(Error::<T>::MultisigOperationUninitialized)?;

                let voter_balance =
                    Balance::<T>::get(ips_id, owner.clone()).ok_or(Error::<T>::NoPermission)?;

                let total_in_operation: T::Balance = old_data
                    .signers
                    .clone()
                    .into_iter()
                    .map(|voter| -> Option<T::Balance> { Balance::<T>::get(ips_id, voter) })
                    .collect::<Option<Vec<T::Balance>>>()
                    .ok_or(Error::<T>::NoPermission)?
                    .into_iter()
                    .sum();

                let total_per_2 = ipt.supply / 2u32.into();

                if (total_in_operation + voter_balance) > total_per_2 {
                    old_data.actual_call.dispatch(
                        RawOrigin::Signed(multi_account_id::<T, T::IptId>(
                            ips_id,
                            old_data.include_original_caller,
                        ))
                        .into(),
                    )?;
                } else {
                    old_data.signers = {
                        let mut v = old_data.signers.to_vec();
                        v.push(owner);
                        v.try_into().map_err(|_| Error::<T>::MaxMetadataExceeded)?
                    };
                    *data = Some(old_data);
                }

                Ok(().into())
            })
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

    impl<T: Config> Pallet<T> {
        pub fn create(
            owner: T::AccountId,
            ips_id: T::IptId,
            endowed_accounts: Vec<(T::AccountId, <T as pallet::Config>::Balance)>,
        ) {
            Ipt::<T>::insert(
                ips_id,
                AssetDetails {
                    owner,
                    supply: endowed_accounts
                        .clone()
                        .into_iter()
                        .map(|(_, balance)| balance)
                        .sum(),
                    deposit: Default::default(),
                },
            );

            endowed_accounts
                .iter()
                .for_each(|(account, balance)| Balance::<T>::insert(ips_id, account, balance));
        }

        pub fn internal_mint(
            target: T::AccountId,
            ips_id: T::IptId,
            amount: <T as pallet::Config>::Balance,
        ) -> DispatchResult {
            Ipt::<T>::try_mutate(ips_id, |ipt| -> DispatchResult {
                Balance::<T>::try_mutate(ips_id, target, |balance| -> DispatchResult {
                    let old_balance = balance.take().unwrap_or_default();
                    *balance = Some(old_balance + amount);

                    let mut old_ipt = ipt.take().ok_or(Error::<T>::IptDoesntExist)?;
                    old_ipt.supply += amount;
                    *ipt = Some(old_ipt);

                    Ok(())
                })
            })
        }

        pub fn internal_burn(
            target: T::AccountId,
            ips_id: T::IptId,
            amount: <T as pallet::Config>::Balance,
        ) -> DispatchResult {
            Ipt::<T>::try_mutate(ips_id, |ipt| -> DispatchResult {
                Balance::<T>::try_mutate(ips_id, target, |balance| -> DispatchResult {
                    let old_balance = balance.take().ok_or(Error::<T>::IptDoesntExist)?;
                    *balance = Some(
                        old_balance
                            .checked_sub(&amount)
                            .ok_or(Error::<T>::NotEnoughAmount)?,
                    );

                    let old_ipt = ipt.take().ok_or(Error::<T>::IptDoesntExist)?;
                    old_ipt
                        .supply
                        .checked_sub(&amount)
                        .ok_or(Error::<T>::NotEnoughAmount)?;
                    *ipt = Some(old_ipt);

                    Ok(())
                })
            })
        }
    }
}
