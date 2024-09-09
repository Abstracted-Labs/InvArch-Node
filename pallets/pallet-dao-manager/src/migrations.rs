use super::*;
use frame_support::{
    dispatch::GetStorageVersion,
    traits::{Get, OnRuntimeUpgrade},
    weights::Weight,
};
use log::{info, warn};

pub mod v1 {

    use super::*;

    pub fn clear_storages<T: Config>() {
        let _ = frame_support::migration::clear_storage_prefix(b"INV4", b"", b"", None, None);
    }

    pub struct MigrateToV1<T>(sp_std::marker::PhantomData<T>);
    impl<T: Config> OnRuntimeUpgrade for MigrateToV1<T> {
        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, &'static str> {
            frame_support::ensure!(
                Pallet::<T>::current_storage_version() == 0,
                "Required v0 before upgrading to v1"
            );

            Ok(Default::default())
        }

        fn on_runtime_upgrade() -> Weight {
            let current = Pallet::<T>::current_storage_version();

            if current == 1 {
                clear_storages::<T>();

                current.put::<Pallet<T>>();

                info!("v1 applied successfully");
                T::DbWeight::get().reads_writes(0, 1)
            } else {
                warn!("Skipping v1, should be removed");
                T::DbWeight::get().reads(1)
            }
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), &'static str> {
            frame_support::ensure!(
                Pallet::<T>::on_chain_storage_version() == 1,
                "v1 not applied"
            );

            Ok(())
        }
    }
}

pub mod v2 {
    use super::*;
    use codec::{Decode, Encode};
    use frame_support::{
        pallet_prelude::ValueQuery, storage_alias, Blake2_128Concat, Twox64Concat,
    };

    #[derive(Default, Encode, Decode)]
    pub struct AccountData<Balance> {
        pub free: Balance,
        pub reserved: Balance,
        pub frozen: Balance,
    }

    #[storage_alias]
    pub type Accounts<T: crate::Config + frame_system::Config + orml_tokens2::Config> =
        StorageDoubleMap<
            orml_tokens2::Pallet<T>,
            Blake2_128Concat,
            <T as frame_system::Config>::AccountId,
            Twox64Concat,
            <T as crate::Config>::DaoId,
            AccountData<u128>,
            ValueQuery,
        >;

    pub fn fill_dao_owners<T: Config + orml_tokens2::Config>() {
        Accounts::<T>::iter_keys()
            .for_each(|(member, dao_id)| CoreMembers::<T>::insert(dao_id, member, ()));
    }

    pub struct MigrateToV2<T>(sp_std::marker::PhantomData<T>);
    impl<T: Config + orml_tokens2::Config> OnRuntimeUpgrade for MigrateToV2<T> {
        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, &'static str> {
            frame_support::ensure!(
                Pallet::<T>::current_storage_version() == 1,
                "Required v1 before upgrading to v2"
            );

            Ok(Default::default())
        }

        fn on_runtime_upgrade() -> Weight {
            let current = Pallet::<T>::current_storage_version();

            if current == 2 {
                fill_dao_owners::<T>();

                current.put::<Pallet<T>>();

                info!("v2 applied successfully");
                T::DbWeight::get().reads_writes(0, 1)
            } else {
                warn!("Skipping v1, should be removed");
                T::DbWeight::get().reads(1)
            }
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), &'static str> {
            frame_support::ensure!(
                Pallet::<T>::on_chain_storage_version() == 2,
                "v2 not applied"
            );

            Ok(())
        }
    }
}
