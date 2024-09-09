#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::{
    fee_handling::FeeAsset,
    multisig::MAX_SIZE,
    origin::{DaoOrigin, MultisigInternalOrigin},
    voting::{Tally, Vote},
    BalanceOf,
};
use core::convert::TryFrom;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::{
    dispatch::PostDispatchInfo,
    pallet_prelude::DispatchResultWithPostInfo,
    traits::{Currency, Get},
    BoundedBTreeMap, BoundedVec,
};
use frame_system::RawOrigin as SystemOrigin;
use sp_runtime::{
    traits::{Bounded, Hash, Zero},
    DispatchError, DispatchErrorWithPostInfo, Perbill,
};
use sp_std::{
    collections::btree_map::BTreeMap, convert::TryInto, iter::Sum, ops::Div, prelude::*, vec,
};

use crate::Pallet as dao_manager;

const SEED: u32 = 0;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn perbill_one() -> Perbill {
    Perbill::one()
}

fn derive_account<T>(dao_id: T::DaoId) -> T::AccountId
where
    T: Config,
    T::AccountId: From<[u8; 32]>,
{
    dao_manager::<T>::derive_dao_account(dao_id)
}

fn mock_dao<T: Config>() -> DispatchResultWithPostInfo
where
    Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
        From<<T as frame_system::Config>::RuntimeOrigin>,
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance:
        Sum,
    T::AccountId: From<[u8; 32]>,
{
    T::Currency::make_free_balance_be(
        &whitelisted_caller(),
        T::DaoCreationFee::get() + T::DaoCreationFee::get(),
    );

    dao_manager::<T>::create_dao(
        SystemOrigin::Signed(whitelisted_caller()).into(),
        vec![].try_into().unwrap(),
        perbill_one(),
        perbill_one(),
        FeeAsset::Native,
    )
}

fn mock_mint<T: Config>() -> Result<(), DispatchError>
where
    Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
        From<<T as frame_system::Config>::RuntimeOrigin>,
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance:
        Sum,
    <T as frame_system::Config>::RuntimeOrigin: From<DaoOrigin<T>>,
    T::AccountId: From<[u8; 32]>,
{
    dao_manager::<T>::token_mint(
        DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())).into(),
        BalanceOf::<T>::max_value().div(4u32.into()),
        account("target", 0, SEED),
    )
}

fn mock_mint_2<T: Config>() -> Result<(), DispatchError>
where
    Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
        From<<T as frame_system::Config>::RuntimeOrigin>,
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance:
        Sum,
    <T as frame_system::Config>::RuntimeOrigin: From<DaoOrigin<T>>,
    T::AccountId: From<[u8; 32]>,
{
    dao_manager::<T>::token_mint(
        DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())).into(),
        BalanceOf::<T>::max_value().div(4u32.into()),
        account("target1", 1, SEED + 1),
    )
}

fn mock_call<T: Config>() -> Result<PostDispatchInfo, DispatchErrorWithPostInfo<PostDispatchInfo>>
where
    Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
        From<<T as frame_system::Config>::RuntimeOrigin>,
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance:
        Sum,
    <T as frame_system::Config>::RuntimeOrigin: From<DaoOrigin<T>>,
    T::AccountId: From<[u8; 32]>,
{
    dao_manager::<T>::operate_multisig(
        SystemOrigin::Signed(whitelisted_caller()).into(),
        0u32.into(),
        None,
        FeeAsset::Native,
        Box::new(frame_system::Call::<T>::remark { remark: vec![0] }.into()),
    )
}

fn mock_vote<T: Config>() -> Result<PostDispatchInfo, DispatchErrorWithPostInfo<PostDispatchInfo>>
where
    Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
        From<<T as frame_system::Config>::RuntimeOrigin>,
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance:
        Sum,
    <T as frame_system::Config>::RuntimeOrigin: From<DaoOrigin<T>>,
    T::AccountId: From<[u8; 32]>,
{
    let call: <T as Config>::RuntimeCall =
        frame_system::Call::<T>::remark { remark: vec![0] }.into();
    let call_hash = <<T as frame_system::Config>::Hashing as Hash>::hash_of(&call.clone());

    dao_manager::<T>::vote_multisig(
        SystemOrigin::Signed(account("target", 0, SEED)).into(),
        0u32.into(),
        call_hash,
        true,
    )
}

benchmarks! {

    where_clause {
      where
        Result<
                DaoOrigin<T>,
            <T as frame_system::Config>::RuntimeOrigin,
            >: From<<T as frame_system::Config>::RuntimeOrigin>,
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance: Sum,
    <T as frame_system::Config>::RuntimeOrigin: From<DaoOrigin<T>>,
    T::AccountId: From<[u8; 32]>,
}

    create_dao {
        let m in 0 .. T::MaxMetadata::get();

        let metadata: BoundedVec<u8, T::MaxMetadata> = vec![u8::MAX; m as usize].try_into().unwrap();
        let caller = whitelisted_caller();
        let minimum_support = perbill_one();
        let required_approval = perbill_one();
        let creation_fee_asset = FeeAsset::Native;

        T::Currency::make_free_balance_be(&caller, T::DaoCreationFee::get() + T::DaoCreationFee::get());
    }: _(SystemOrigin::Signed(caller.clone()), metadata.clone(), minimum_support, required_approval, creation_fee_asset)
        verify {
            assert_last_event::<T>(Event::DaoCreated {
                dao_account: derive_account::<T>(0u32.into()),
                dao_id: 0u32.into(),
                metadata: metadata.to_vec(),
                minimum_support,
                required_approval
            }.into());
        }

    set_parameters {
        let m in 0 .. T::MaxMetadata::get();

        mock_dao().unwrap();

        let metadata: Option<BoundedVec<u8, T::MaxMetadata>> = Some(vec![u8::MAX; m as usize].try_into().unwrap());
        let minimum_support = Some(perbill_one());
        let required_approval = Some(perbill_one());
        let frozen_tokens = Some(true);

    }: _(DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())), metadata.clone(), minimum_support, required_approval, frozen_tokens)
        verify {
            assert_last_event::<T>(Event::ParametersSet {
                dao_id: 0u32.into(),
                metadata: metadata.map(|m| m.to_vec()),
                minimum_support,
                required_approval,
                frozen_tokens
            }.into());
        }

    token_mint {
        mock_dao().unwrap();

        let amount = BalanceOf::<T>::max_value().div(2u32.into());
        let target: T::AccountId = account("target", 0, SEED);

    }: _(DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())), amount, target.clone())
        verify {
            assert_last_event::<T>(Event::Minted {
                dao_id: 0u32.into(),
                target,
                amount
            }.into());
        }

    token_burn {
        mock_dao().unwrap();
        mock_mint().unwrap();

        let amount = BalanceOf::<T>::max_value().div(4u32.into());
        let target: T::AccountId = account("target", 0, SEED);

    }: _(DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())), amount, target.clone())
        verify {
            assert_last_event::<T>(Event::Burned {
                dao_id: 0u32.into(),
                target,
                amount
            }.into());
        }

    operate_multisig {
        let m in 0 .. T::MaxMetadata::get();
        let z in 0 .. (MAX_SIZE - 10);

        mock_dao().unwrap();
        mock_mint().unwrap();

        let call: <T as Config>::RuntimeCall = frame_system::Call::<T>::remark {
            remark: vec![0; z as usize]
        }.into();

        let metadata: BoundedVec<u8, T::MaxMetadata> = vec![u8::MAX; m as usize].try_into().unwrap();
        let caller: T::AccountId = whitelisted_caller();
        let dao_id: T::DaoId = 0u32.into();
        let call_hash = <<T as frame_system::Config>::Hashing as Hash>::hash_of(&call.clone());
        let fee_asset = FeeAsset::Native;

    }: _(SystemOrigin::Signed(caller.clone()), dao_id, Some(metadata), fee_asset, Box::new(call.clone()))
        verify {
            assert_last_event::<T>(Event::MultisigVoteStarted {
                dao_id,
                executor_account: derive_account::<T>(dao_id),
                voter: caller,
                votes_added: Vote::Aye(T::DaoSeedBalance::get()),
                call_hash,
            }.into());
        }

    vote_multisig {
        mock_dao().unwrap();
        mock_mint().unwrap();
        mock_mint_2().unwrap();
        mock_call().unwrap();

        let caller: T::AccountId = account("target", 0, SEED);
        let dao_id: T::DaoId = 0u32.into();
        let call: <T as Config>::RuntimeCall = frame_system::Call::<T>::remark {
            remark: vec![0]
        }.into();
        let call_hash = <<T as frame_system::Config>::Hashing as Hash>::hash_of(&call.clone());

    }: _(SystemOrigin::Signed(caller.clone()), dao_id, call_hash, true)
        verify {
            assert_last_event::<T>(Event::MultisigVoteAdded {
                dao_id,
                executor_account: derive_account::<T>(dao_id),
                voter: caller.clone(),
                votes_added:  Vote::Aye(BalanceOf::<T>::max_value().div(4u32.into())),
                current_votes: Tally::<T>::from_parts(
                    (BalanceOf::<T>::max_value().div(4u32.into()) + T::DaoSeedBalance::get()).into(),
                    Zero::zero(),
                    BoundedBTreeMap::try_from(BTreeMap::from([(
                        whitelisted_caller(),
                        Vote::Aye(T::DaoSeedBalance::get()),
                    ),
                                                              (caller, Vote::Aye(BalanceOf::<T>::max_value().div(4u32.into())))
                    ])).unwrap()
                ),
                call_hash,
            }.into());
        }

    withdraw_vote_multisig {
        mock_dao().unwrap();
        mock_mint().unwrap();
        mock_mint_2().unwrap();
        mock_call().unwrap();
        mock_vote().unwrap();

        let caller: T::AccountId = account("target", 0, SEED);
        let dao_id: T::DaoId = 0u32.into();
        let call: <T as Config>::RuntimeCall = frame_system::Call::<T>::remark {
            remark: vec![0]
        }.into();
        let call_hash = <<T as frame_system::Config>::Hashing as Hash>::hash_of(&call.clone());

    }: _(SystemOrigin::Signed(caller.clone()), dao_id, call_hash)
        verify {
            assert_last_event::<T>(Event::MultisigVoteWithdrawn {
                dao_id,
                executor_account: derive_account::<T>(dao_id),
                voter: caller,
                votes_removed: Vote::Aye(BalanceOf::<T>::max_value().div(4u32.into())),
                call_hash,
            }.into());
        }

    cancel_multisig_proposal {
        mock_dao().unwrap();
        mock_mint().unwrap();
        mock_mint_2().unwrap();
        mock_call().unwrap();

        let caller: T::AccountId = account("target", 0, SEED);
        let dao_id: T::DaoId = 0u32.into();
        let call: <T as Config>::RuntimeCall = frame_system::Call::<T>::remark {
            remark: vec![0]
        }.into();
        let call_hash = <<T as frame_system::Config>::Hashing as Hash>::hash_of(&call.clone());

    }: _(DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())), call_hash)
        verify {
            assert_last_event::<T>(Event::MultisigCanceled {
                dao_id,
                call_hash,
            }.into());
        }
}
