#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet as OcifStaking;
use core::ops::Add;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_support::{
    traits::{Get, OnFinalize, OnInitialize},
    BoundedVec,
};
use frame_system::{Pallet as System, RawOrigin};
use pallet_dao_manager::{
    account_derivation::DaoAccountDerivation,
    origin::{DaoOrigin, MultisigInternalOrigin},
};
use sp_runtime::traits::{Bounded, One};
use sp_std::vec;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn derive_account<T>(dao_id: <T as pallet_dao_manager::Config>::DaoId) -> T::AccountId
where
    T: pallet_dao_manager::Config,
    T::AccountId: From<[u8; 32]>,
{
    <pallet_dao_manager::Pallet<T> as DaoAccountDerivation<T>>::derive_dao_account(dao_id)
}

fn advance_to_era<T: Config>(n: Era) {
    while OcifStaking::<T>::current_era() < n {
        <OcifStaking<T> as OnFinalize<BlockNumberFor<T>>>::on_finalize(System::<T>::block_number());
        System::<T>::set_block_number(System::<T>::block_number() + One::one());
        <OcifStaking<T> as OnInitialize<BlockNumberFor<T>>>::on_initialize(
            System::<T>::block_number(),
        );
    }
}

fn mock_register<T: Config>() -> DispatchResultWithPostInfo
where
    Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
        From<<T as frame_system::Config>::RuntimeOrigin>,
    <T as frame_system::Config>::RuntimeOrigin: From<DaoOrigin<T>>,
    T::AccountId: From<[u8; 32]>,
{
    <T as Config>::Currency::make_free_balance_be(
        &derive_account::<T>(0u32.into()),
        T::RegisterDeposit::get() + T::RegisterDeposit::get(),
    );

    OcifStaking::<T>::register_dao(
        DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())).into(),
        vec![].try_into().unwrap(),
        vec![].try_into().unwrap(),
        vec![].try_into().unwrap(),
    )
}

fn mock_register_2<T: Config>() -> DispatchResultWithPostInfo
where
    Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
        From<<T as frame_system::Config>::RuntimeOrigin>,
    <T as frame_system::Config>::RuntimeOrigin: From<DaoOrigin<T>>,
    T::AccountId: From<[u8; 32]>,
{
    <T as Config>::Currency::make_free_balance_be(
        &derive_account::<T>(1u32.into()),
        T::RegisterDeposit::get() + T::RegisterDeposit::get(),
    );

    OcifStaking::<T>::register_dao(
        DaoOrigin::Multisig(MultisigInternalOrigin::new(1u32.into())).into(),
        vec![].try_into().unwrap(),
        vec![].try_into().unwrap(),
        vec![].try_into().unwrap(),
    )
}

fn mock_stake<T: Config>() -> DispatchResultWithPostInfo
where
    Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
        From<<T as frame_system::Config>::RuntimeOrigin>,
    <T as frame_system::Config>::RuntimeOrigin: From<DaoOrigin<T>>,
    T::AccountId: From<[u8; 32]>,
{
    <T as Config>::Currency::make_free_balance_be(
        &whitelisted_caller(),
        pallet::BalanceOf::<T>::max_value(),
    );

    OcifStaking::<T>::stake(
        RawOrigin::Signed(whitelisted_caller()).into(),
        0u32.into(),
        T::StakeThresholdForActiveDao::get() + T::StakeThresholdForActiveDao::get(),
    )
}

fn mock_unstake<T: Config>() -> DispatchResultWithPostInfo
where
    Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
        From<<T as frame_system::Config>::RuntimeOrigin>,
    <T as frame_system::Config>::RuntimeOrigin: From<DaoOrigin<T>>,
    T::AccountId: From<[u8; 32]>,
{
    OcifStaking::<T>::unstake(
        RawOrigin::Signed(whitelisted_caller()).into(),
        0u32.into(),
        T::StakeThresholdForActiveDao::get() + T::StakeThresholdForActiveDao::get(),
    )
}

benchmarks! {
    where_clause {
    where
        Result<
            DaoOrigin<T>,
            <T as frame_system::Config>::RuntimeOrigin,
            >: From<<T as frame_system::Config>::RuntimeOrigin>,
    <T as frame_system::Config>::RuntimeOrigin: From<DaoOrigin<T>>,
    T::AccountId: From<[u8; 32]>,

}

    register_dao {
        let n in 0 .. T::MaxNameLength::get();
        let d in 0 .. T::MaxDescriptionLength::get();
        let i in 0 .. T::MaxImageUrlLength::get();

        let name: BoundedVec<u8, T::MaxNameLength> = vec![u8::MAX; n as usize].try_into().unwrap();
        let description: BoundedVec<u8, T::MaxDescriptionLength> = vec![u8::MAX; d as usize].try_into().unwrap();
        let image: BoundedVec<u8, T::MaxImageUrlLength> = vec![u8::MAX; i as usize].try_into().unwrap();

        <T as Config>::Currency::make_free_balance_be(&derive_account::<T>(0u32.into()), T::RegisterDeposit::get() + T::RegisterDeposit::get());
    }: _(DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())), name, description, image)
    verify {
        assert_last_event::<T>(Event::<T>::DaoRegistered {
            dao: 0u32.into()
        }.into());
    }

    change_dao_metadata {
        let n in 0 .. T::MaxNameLength::get();
        let d in 0 .. T::MaxDescriptionLength::get();
        let i in 0 .. T::MaxImageUrlLength::get();

        let name: BoundedVec<u8, T::MaxNameLength> = vec![u8::MAX; n as usize].try_into().unwrap();
        let description: BoundedVec<u8, T::MaxDescriptionLength> = vec![u8::MAX; d as usize].try_into().unwrap();
        let image: BoundedVec<u8, T::MaxImageUrlLength> = vec![u8::MAX; i as usize].try_into().unwrap();

        mock_register().unwrap();

    }: _(DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())), name.clone(), description.clone(), image.clone())
        verify {
            assert_last_event::<T>(Event::<T>::MetadataChanged {
                dao: 0u32.into(),
                old_metadata: DaoMetadata {
                    name: vec![],
                    description: vec![],
                    image: vec![]
                },
                new_metadata: DaoMetadata {
                    name: name.to_vec(),
                    description: description.to_vec(),
                    image: image.to_vec()
                }
            }.into());
        }

    unregister_dao {
        mock_register().unwrap();

    }: _(DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())))
    verify {
        assert_last_event::<T>(Event::<T>::DaoUnregistered {
            dao: 0u32.into()
        }.into());
    }

    stake {
        mock_register().unwrap();

        let staker = whitelisted_caller();
        let amount = T::StakeThresholdForActiveDao::get() + T::StakeThresholdForActiveDao::get();

        <T as Config>::Currency::make_free_balance_be(&staker, pallet::BalanceOf::<T>::max_value());
    }: _(RawOrigin::Signed(staker.clone()), 0u32.into(), amount)
    verify {
        assert_last_event::<T>(Event::<T>::Staked {
            staker,
            dao: 0u32.into(),
            amount
        }.into());
    }

    unstake {
        mock_register().unwrap();
        mock_stake().unwrap();

        let staker: T::AccountId = whitelisted_caller();
        let amount = T::StakeThresholdForActiveDao::get() + T::StakeThresholdForActiveDao::get();

    }: _(RawOrigin::Signed(staker.clone()), 0u32.into(), amount)
        verify {
            assert_last_event::<T>(Event::<T>::Unstaked {
                staker,
                dao: 0u32.into(),
                amount
            }.into());
        }

    withdraw_unstaked {
        mock_register().unwrap();
        mock_stake().unwrap();
        mock_unstake().unwrap();
        advance_to_era::<T>(T::UnbondingPeriod::get().add(1));

        let staker: T::AccountId = whitelisted_caller();
        let amount = T::StakeThresholdForActiveDao::get() + T::StakeThresholdForActiveDao::get();

    }: _(RawOrigin::Signed(staker.clone()))
        verify {
            assert_last_event::<T>(Event::<T>::Withdrawn {
                staker,
                amount
            }.into());
        }

    staker_claim_rewards {
        mock_register().unwrap();
        mock_stake().unwrap();
        advance_to_era::<T>(One::one());

        let staker: T::AccountId = whitelisted_caller();
        let amount = T::StakeThresholdForActiveDao::get() + T::StakeThresholdForActiveDao::get();

        let dao_stake_info = OcifStaking::<T>::dao_stake_info::<<T as pallet_dao_manager::Config>::DaoId, Era>(0u32.into(), 0u32).unwrap();
        let era_info = OcifStaking::<T>::general_era_info::<Era>(0u32).unwrap();

        let (_, reward) = OcifStaking::<T>::dao_stakers_split(&dao_stake_info, &era_info);

    }: _(RawOrigin::Signed(staker.clone()), 0u32.into())
        verify {
            assert_last_event::<T>(Event::<T>::StakerClaimed {
                staker,
                dao: 0u32.into(),
                era: 0u32.into(),
                amount: reward
            }.into());
        }

    dao_claim_rewards {
        mock_register().unwrap();
        mock_stake().unwrap();
        advance_to_era::<T>(One::one());

        let staker: T::AccountId = whitelisted_caller();
        let amount = T::StakeThresholdForActiveDao::get() + T::StakeThresholdForActiveDao::get();

        let dao_stake_info = OcifStaking::<T>::dao_stake_info::<<T as pallet_dao_manager::Config>::DaoId, Era>(0u32.into(), 0u32).unwrap();
        let era_info = OcifStaking::<T>::general_era_info::<Era>(0u32).unwrap();

        let (reward, _) = OcifStaking::<T>::dao_stakers_split(&dao_stake_info, &era_info);

    }: _(DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())), 0u32.into(), 0u32.into())
        verify {
            assert_last_event::<T>(Event::<T>::DaoClaimed {
                dao: 0u32.into(),
                destination_account: derive_account::<T>(0u32.into()),
                era: 0u32.into(),
                amount: reward
            }.into());
        }

    halt_unhalt_pallet {}: _(RawOrigin::Root, true)
        verify {
            assert_last_event::<T>(Event::<T>::HaltChanged {
                is_halted: true
            }.into());
        }

    move_stake {
        mock_register().unwrap();
        mock_register_2().unwrap();

        mock_stake().unwrap();

        let staker: T::AccountId = whitelisted_caller();
        let amount = T::StakeThresholdForActiveDao::get() + T::StakeThresholdForActiveDao::get();
    }: _(RawOrigin::Signed(staker.clone()), 0u32.into(), amount, 1u32.into())
        verify {
            assert_last_event::<T>(Event::<T>::StakeMoved {
                staker,
                from_dao: 0u32.into(),
                amount,
                to_dao: 1u32.into()
            }.into());
        }
}
