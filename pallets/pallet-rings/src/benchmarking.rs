#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_support::{pallet_prelude::Weight, traits::Get, BoundedVec};
use frame_system::RawOrigin as SystemOrigin;
use pallet_dao_manager::origin::{DaoOrigin, MultisigInternalOrigin};
use sp_std::{ops::Div, prelude::*, vec};

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks! {
    where_clause {
      where
        Result<
            DaoOrigin<T>,
            <T as frame_system::Config>::RuntimeOrigin,
            >: From<<T as frame_system::Config>::RuntimeOrigin>,
    <T as frame_system::Config>::RuntimeOrigin: From<DaoOrigin<T>>,

    <T as pallet_dao_manager::Config>::DaoId: Into<u32>,

    [u8; 32]: From<<T as frame_system::Config>::AccountId>,

    T::AccountId: From<[u8; 32]>,
}

    set_maintenance_status {
        let chain = T::Chains::benchmark_mock();

    }: _(SystemOrigin::Root, chain.clone(), true)
        verify {
            assert_last_event::<T>(Event::ChainMaintenanceStatusChanged {
                chain,
                under_maintenance: true
            }.into());
        }

    send_call {
        let c in 0 .. T::MaxXCMCallLength::get();

        let call: BoundedVec<u8, T::MaxXCMCallLength> = vec![u8::MAX; c as usize].try_into().unwrap();
        let destination = T::Chains::benchmark_mock();
        let weight = Weight::from_parts(100_000_000u64, 10_000u64);
        let fee_asset: <<T as Config>::Chains as ChainList>::ChainAssets = T::Chains::benchmark_mock().get_main_asset();
        let fee: u128 = u128::MAX.div(4u128);

    }: _(DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())), destination.clone(), weight, fee_asset, fee, call.clone())
        verify {
            assert_last_event::<T>(Event::CallSent {
                sender: 0u32.into(),
                destination,
                call: call.to_vec(),
            }.into());
        }

    transfer_assets {
        let asset: <<T as Config>::Chains as ChainList>::ChainAssets = T::Chains::benchmark_mock().get_main_asset();
        let amount: u128 = u128::MAX.div(4u128);
        let to: T::AccountId = whitelisted_caller();

    }: _(DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())), asset.clone(), amount, to.clone(), asset.clone(), amount)
        verify {
            assert_last_event::<T>(Event::AssetsTransferred {
                chain: asset.clone().get_chain(),
                asset,
                amount,
                from: 0u32.into(),
                to,
            }.into());
        }

    bridge_assets {
        let asset: <<T as Config>::Chains as ChainList>::ChainAssets = T::Chains::benchmark_mock().get_main_asset();
        let amount: u128 = u128::MAX.div(4u128);
        let fee: u128 = amount.div(5u128);
        let to: Option<T::AccountId> = Some(whitelisted_caller());

    }: _(DaoOrigin::Multisig(MultisigInternalOrigin::new(0u32.into())), asset.clone(), asset.clone().get_chain(), fee, amount, to)
        verify {
            assert_last_event::<T>(Event::AssetsBridged {
                origin_chain_asset: asset,
                amount,
                from: 0u32.into(),
                to: whitelisted_caller(),
            }.into());
        }
}
