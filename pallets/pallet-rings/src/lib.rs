//! # Rings Pallet
//!
//! - [`Config`]
//! - [`Call`]
//! - [`Pallet`]
//!
//! ## Overview
//!
//! This pallet provides a XCM abstraction layer for DAO Management, allowing them to manage assets easily across multiple chains.
//!
//! The module [`traits`] contains traits that provide an abstraction on top of XCM [`MultiLocation`] and has to be correctly implemented in the runtime.
//!
//! ## Dispatchable Functions
//!
//! - `set_maintenance_status` - Sets the maintenance status of a chain, requires the origin to be authorized as a `MaintenanceOrigin`.
//! - `send_call` - Allows a DAO to send a XCM call to a destination chain.
//! - `transfer_assets` - Allows a DAO to transfer fungible assets to another account in the destination chain.
//! - `bridge_assets` - Allows a DAO to bridge fungible assets to another chain having either a third party account or
//!    the DAO account as beneficiary in the destination chain.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::Get;
use sp_std::convert::TryInto;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod tests;

mod traits;
pub mod weights;

pub use pallet::*;
pub use traits::{ChainAssetsList, ChainList};
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::OriginFor;
    use pallet_dao_manager::origin::{ensure_multisig, DaoOrigin};
    use sp_std::{vec, vec::Vec};
    use xcm::{
        v3::{prelude::*, MultiAsset, Weight, WildMultiAsset},
        DoubleEncoded,
    };

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_dao_manager::Config + pallet_xcm::Config
    {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Higher level type providing an abstraction over a chain's asset and location.
        type Chains: ChainList;

        /// Max length of an XCM call.
        #[pallet::constant]
        type MaxXCMCallLength: Get<u32>;

        /// Origin that can set maintenance status.
        type MaintenanceOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// Maps chain's and their maintenance status.
    #[pallet::storage]
    #[pallet::getter(fn is_under_maintenance)]
    pub type ChainsUnderMaintenance<T: Config> =
        StorageMap<_, Blake2_128Concat, MultiLocation, bool>;

    #[pallet::error]
    pub enum Error<T> {
        /// Failed to send XCM.
        SendingFailed,
        /// Weight exceeds `MaxXCMCallLength`.
        WeightTooHigh,
        /// Failed to calculate XCM fee.
        FailedToCalculateXcmFee,
        /// Failed to reanchor asset.
        FailedToReanchorAsset,
        /// Failed to invert location.
        FailedToInvertLocation,
        /// Asset is not supported in the destination chain.
        DifferentChains,
        /// Chain is under maintenance.
        ChainUnderMaintenance,
    }

    #[pallet::event]
    #[pallet::generate_deposit(fn deposit_event)]
    pub enum Event<T: Config> {
        /// A XCM call was sent.
        CallSent {
            sender: <T as pallet_dao_manager::Config>::DaoId,
            destination: <T as pallet::Config>::Chains,
            call: Vec<u8>,
        },

        /// Assets were transferred.
        AssetsTransferred {
            chain: <<<T as pallet::Config>::Chains as ChainList>::ChainAssets as ChainAssetsList>::Chains,
            asset: <<T as pallet::Config>::Chains as ChainList>::ChainAssets,
            amount: u128,
            from: <T as pallet_dao_manager::Config>::DaoId,
            to: <T as frame_system::Config>::AccountId,
        },

        /// Assets were bridged.
        AssetsBridged {
            origin_chain_asset: <<T as pallet::Config>::Chains as ChainList>::ChainAssets,
            amount: u128,
            from: <T as pallet_dao_manager::Config>::DaoId,
            to: Option<<T as frame_system::Config>::AccountId>,
        },

        /// A Chain's maintenance status changed.
        ChainMaintenanceStatusChanged {
            chain: <T as Config>::Chains,
            under_maintenance: bool,
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        Result<DaoOrigin<T>, <T as frame_system::Config>::RuntimeOrigin>:
            From<<T as frame_system::Config>::RuntimeOrigin>,

        <T as pallet_dao_manager::Config>::DaoId: Into<u32>,

        [u8; 32]: From<<T as frame_system::Config>::AccountId>,
        <T as frame_system::Config>::AccountId: From<[u8; 32]>,
    {
        /// Set the maintenance status of a chain.
        ///
        /// The origin has to be `MaintenanceOrigin`.
        ///
        /// - `chain`: referred chain.
        /// - `under_maintenance`: maintenance status.
        #[pallet::call_index(0)]
        #[pallet::weight((<T as Config>::WeightInfo::set_maintenance_status(), Pays::No))]
        pub fn set_maintenance_status(
            origin: OriginFor<T>,
            chain: <T as Config>::Chains,
            under_maintenance: bool,
        ) -> DispatchResult {
            T::MaintenanceOrigin::ensure_origin(origin)?;

            ChainsUnderMaintenance::<T>::insert(chain.get_location(), under_maintenance);

            Self::deposit_event(Event::<T>::ChainMaintenanceStatusChanged {
                chain,
                under_maintenance,
            });

            Ok(())
        }

        /// Send a XCM call to a destination chain.
        ///
        /// The origin has to be a dao.
        ///
        /// - `destination`: destination chain.
        /// - `weight`: weight of the call.
        /// - `fee_asset`: asset used to pay the fee.
        /// - `fee`: fee amount.
        /// - `call`: XCM call.
        #[pallet::call_index(1)]
        #[pallet::weight(
            <T as Config>::WeightInfo::send_call(call.len() as u32)
        )]
        pub fn send_call(
            origin: OriginFor<T>,
            destination: <T as pallet::Config>::Chains,
            weight: Weight,
            fee_asset: <<T as pallet::Config>::Chains as ChainList>::ChainAssets,
            fee: u128,
            call: BoundedVec<u8, T::MaxXCMCallLength>,
        ) -> DispatchResult {
            let dao = ensure_multisig::<T, OriginFor<T>>(origin)?;
            let dao_id = dao.id.into();

            let dest = destination.get_location();

            ensure!(
                !Self::is_under_maintenance(dest).unwrap_or(false),
                Error::<T>::ChainUnderMaintenance
            );

            let descend_interior = Junction::Plurality {
                id: BodyId::Index(dao_id),
                part: BodyPart::Voice,
            };

            let fee_asset_location = fee_asset.get_asset_location();

            let mut dao_multilocation: MultiLocation = MultiLocation {
                parents: 1,
                interior: Junctions::X2(
                    Junction::Parachain(<T as pallet_dao_manager::Config>::ParaId::get()),
                    descend_interior,
                ),
            };

            mutate_if_relay(&mut dao_multilocation, &dest);

            let fee_multiasset = MultiAsset {
                id: AssetId::Concrete(fee_asset_location),
                fun: Fungibility::Fungible(fee),
            };

            let message = Xcm(vec![
                Instruction::WithdrawAsset(fee_multiasset.clone().into()),
                Instruction::BuyExecution {
                    fees: fee_multiasset,
                    weight_limit: WeightLimit::Unlimited,
                },
                Instruction::Transact {
                    origin_kind: OriginKind::SovereignAccount,
                    require_weight_at_most: weight,
                    call: <DoubleEncoded<_> as From<Vec<u8>>>::from(call.clone().to_vec()),
                },
                Instruction::RefundSurplus,
                Instruction::DepositAsset {
                    assets: MultiAssetFilter::Wild(WildMultiAsset::AllCounted(1)),
                    beneficiary: dao_multilocation,
                },
            ]);

            pallet_xcm::Pallet::<T>::send_xcm(descend_interior, dest, message)
                .map_err(|_| Error::<T>::SendingFailed)?;

            Self::deposit_event(Event::CallSent {
                sender: dao.id,
                destination,
                call: call.to_vec(),
            });

            Ok(())
        }

        /// Transfer fungible assets to another account in the destination chain.
        ///
        /// Both asset and fee_asset have to be in the same chain.
        ///
        /// The origin has to be a dao.
        ///
        /// - `asset`: asset to transfer.
        /// - `amount`: amount to transfer.
        /// - `to`: account receiving the asset.
        /// - `fee_asset`: asset used to pay the fee.
        /// - `fee`: fee amount.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::transfer_assets())]
        pub fn transfer_assets(
            origin: OriginFor<T>,
            asset: <<T as pallet::Config>::Chains as ChainList>::ChainAssets,
            amount: u128,
            to: <T as frame_system::Config>::AccountId,
            fee_asset: <<T as pallet::Config>::Chains as ChainList>::ChainAssets,
            fee: u128,
        ) -> DispatchResult {
            let dao = ensure_multisig::<T, OriginFor<T>>(origin)?;
            let dao_id = dao.id.into();

            let chain = asset.get_chain();
            let dest = chain.get_location();

            ensure!(
                !Self::is_under_maintenance(dest).unwrap_or(false),
                Error::<T>::ChainUnderMaintenance
            );

            ensure!(chain == fee_asset.get_chain(), Error::<T>::DifferentChains);

            let descend_interior = Junction::Plurality {
                id: BodyId::Index(dao_id),
                part: BodyPart::Voice,
            };

            let asset_location = asset.get_asset_location();

            let multi_asset = MultiAsset {
                id: AssetId::Concrete(asset_location),
                fun: Fungibility::Fungible(amount),
            };

            let beneficiary: MultiLocation = MultiLocation {
                parents: 0,
                interior: Junctions::X1(Junction::AccountId32 {
                    network: None,
                    id: to.clone().into(),
                }),
            };

            let mut dao_multilocation: MultiLocation = MultiLocation {
                parents: 1,
                interior: Junctions::X2(
                    Junction::Parachain(<T as pallet_dao_manager::Config>::ParaId::get()),
                    descend_interior,
                ),
            };

            mutate_if_relay(&mut dao_multilocation, &dest);

            let fee_multiasset = MultiAsset {
                id: AssetId::Concrete(fee_asset.get_asset_location()),
                fun: Fungibility::Fungible(fee),
            };

            let message = Xcm(vec![
                // Pay execution fees
                Instruction::WithdrawAsset(fee_multiasset.clone().into()),
                Instruction::BuyExecution {
                    fees: fee_multiasset,
                    weight_limit: WeightLimit::Unlimited,
                },
                // Actual transfer instruction
                Instruction::TransferAsset {
                    assets: multi_asset.into(),
                    beneficiary,
                },
                // Refund unused fees
                Instruction::RefundSurplus,
                Instruction::DepositAsset {
                    assets: MultiAssetFilter::Wild(WildMultiAsset::AllCounted(1)),
                    beneficiary: dao_multilocation,
                },
            ]);

            pallet_xcm::Pallet::<T>::send_xcm(descend_interior, dest, message)
                .map_err(|_| Error::<T>::SendingFailed)?;

            Self::deposit_event(Event::AssetsTransferred {
                chain,
                asset,
                amount,
                from: dao.id,
                to,
            });

            Ok(())
        }

        /// Bridge fungible assets to another chain.
        ///
        /// The origin has to be a dao.
        ///
        /// - `asset`: asset to bridge and the chain to bridge from.
        /// - `destination`: destination chain.
        /// - `fee`: fee amount.
        /// - `amount`: amount to bridge.
        /// - `to`: account receiving the asset, None defaults to dao account.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::bridge_assets())]
        pub fn bridge_assets(
            origin: OriginFor<T>,
            asset: <<T as pallet::Config>::Chains as ChainList>::ChainAssets,
            destination: <<<T as pallet::Config>::Chains as ChainList>::ChainAssets as ChainAssetsList>::Chains,
            fee: u128,
            amount: u128,
            to: Option<<T as frame_system::Config>::AccountId>,
        ) -> DispatchResult {
            let dao = ensure_multisig::<T, OriginFor<T>>(origin)?;

            let dao_id = dao.id.into();

            let from_chain = asset.get_chain();
            let from_chain_location = from_chain.get_location();

            let dest = destination.get_location();

            ensure!(
                !(Self::is_under_maintenance(from_chain_location).unwrap_or(false)
                    || Self::is_under_maintenance(dest).unwrap_or(false)),
                Error::<T>::ChainUnderMaintenance
            );

            let descend_interior = Junction::Plurality {
                id: BodyId::Index(dao_id),
                part: BodyPart::Voice,
            };

            let asset_location = asset.get_asset_location();

            let inverted_destination = dest
                .reanchored(&from_chain_location, *from_chain_location.interior())
                .map(|inverted| {
                    if let (ml, Some(Junction::OnlyChild) | None) = inverted.split_last_interior() {
                        ml
                    } else {
                        inverted
                    }
                })
                .map_err(|_| Error::<T>::FailedToInvertLocation)?;

            let multiasset = MultiAsset {
                id: AssetId::Concrete(asset_location),
                fun: Fungibility::Fungible(amount),
            };

            let fee_multiasset = MultiAsset {
                id: AssetId::Concrete(asset_location),
                fun: Fungibility::Fungible(fee),
            };

            let reanchored_multiasset = multiasset
                .clone()
                .reanchored(&dest, *from_chain_location.interior())
                .map(|mut reanchored| {
                    if let AssetId::Concrete(ref mut m) = reanchored.id {
                        if let (ml, Some(Junction::OnlyChild) | None) = (*m).split_last_interior() {
                            *m = ml;
                        }
                    }
                    reanchored
                })
                .map_err(|_| Error::<T>::FailedToReanchorAsset)?;

            let mut dao_multilocation: MultiLocation = MultiLocation {
                parents: 1,
                interior: Junctions::X2(
                    Junction::Parachain(<T as pallet_dao_manager::Config>::ParaId::get()),
                    descend_interior,
                ),
            };

            let beneficiary: MultiLocation = match to.clone() {
                Some(to_inner) => MultiLocation {
                    parents: 0,
                    interior: Junctions::X1(Junction::AccountId32 {
                        network: None,
                        id: to_inner.into(),
                    }),
                },
                None => {
                    let mut dest_dao_multilocation = dao_multilocation;

                    mutate_if_relay(&mut dest_dao_multilocation, &dest);

                    dest_dao_multilocation
                }
            };

            mutate_if_relay(&mut dao_multilocation, &dest);

            // If the asset originates from the destination chain, we need to reverse the reserve-transfer.
            let message = if asset_location.starts_with(&dest) {
                Xcm(vec![
                    WithdrawAsset(vec![fee_multiasset.clone(), multiasset.clone()].into()),
                    // DAO pays for the execution fee incurred on sending the XCM.
                    Instruction::BuyExecution {
                        fees: fee_multiasset,
                        weight_limit: WeightLimit::Unlimited,
                    },
                    InitiateReserveWithdraw {
                        assets: multiasset.into(),
                        reserve: inverted_destination,
                        xcm: Xcm(vec![
                            // the beneficiary buys execution fee in the destination chain for the deposit.
                            Instruction::BuyExecution {
                                fees: reanchored_multiasset,
                                weight_limit: WeightLimit::Unlimited,
                            },
                            Instruction::DepositAsset {
                                assets: AllCounted(1).into(),
                                beneficiary,
                            },
                            Instruction::RefundSurplus,
                            // Refunds the beneficiary the surplus of the execution fees in the destination chain.
                            Instruction::DepositAsset {
                                assets: AllCounted(1).into(),
                                beneficiary,
                            },
                        ]),
                    },
                    Instruction::RefundSurplus,
                    // Refunds the dao the surplus of the execution fees incurred on sending the XCM.
                    Instruction::DepositAsset {
                        assets: AllCounted(1).into(),
                        beneficiary: dao_multilocation,
                    },
                ])
            } else {
                Xcm(vec![
                    // Pay execution fees
                    Instruction::WithdrawAsset(fee_multiasset.clone().into()),
                    Instruction::BuyExecution {
                        fees: fee_multiasset,
                        weight_limit: WeightLimit::Unlimited,
                    },
                    // Actual reserve transfer instruction
                    Instruction::TransferReserveAsset {
                        assets: multiasset.into(),
                        dest: inverted_destination,
                        xcm: Xcm(vec![
                            Instruction::BuyExecution {
                                fees: reanchored_multiasset,
                                weight_limit: WeightLimit::Unlimited,
                            },
                            Instruction::DepositAsset {
                                assets: MultiAssetFilter::Wild(WildMultiAsset::AllCounted(1)),
                                beneficiary,
                            },
                        ]),
                    },
                    // Refund unused fees
                    Instruction::RefundSurplus,
                    Instruction::DepositAsset {
                        assets: MultiAssetFilter::Wild(WildMultiAsset::AllCounted(1)),
                        beneficiary: dao_multilocation,
                    },
                ])
            };

            pallet_xcm::Pallet::<T>::send_xcm(descend_interior, from_chain_location, message)
                .map_err(|_| Error::<T>::SendingFailed)?;

            Self::deposit_event(Event::AssetsBridged {
                origin_chain_asset: asset,
                from: dao.id,
                amount,
                to,
            });

            Ok(())
        }
    }

    pub fn mutate_if_relay(origin: &mut MultiLocation, dest: &MultiLocation) {
        if dest.contains_parents_only(1) {
            origin.dec_parent();
        }
    }
}
