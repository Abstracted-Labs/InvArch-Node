mod mock;

use crate::{traits::*, Error};
use frame_support::{assert_err, assert_ok, error::BadOrigin};
use frame_system::RawOrigin;
use mock::*;
use pallet_dao_manager::{origin::MultisigInternalOrigin, Origin};
use sp_std::vec;
use xcm::latest::{BodyId, BodyPart, Junction, Junctions, MultiLocation, Weight};

#[test]
fn set_maintenance_status() {
    ExtBuilder::default().build().execute_with(|| {
        let chain_a = Chains::ChainA;

        assert_eq!(
            Rings::is_under_maintenance(chain_a.clone().get_location()),
            None
        );

        assert_ok!(Rings::set_maintenance_status(
            RawOrigin::Root.into(),
            chain_a.clone(),
            true
        ));

        assert_eq!(
            Rings::is_under_maintenance(chain_a.clone().get_location()),
            Some(true)
        );

        assert_ok!(Rings::set_maintenance_status(
            RawOrigin::Root.into(),
            chain_a.clone(),
            false
        ));

        assert_eq!(
            Rings::is_under_maintenance(chain_a.clone().get_location()),
            Some(false)
        );

        assert_err!(
            Rings::set_maintenance_status(RawOrigin::Signed(ALICE).into(), chain_a, true),
            BadOrigin
        );
    })
}

#[test]
fn send_call_works() {
    ExtBuilder::default().build().execute_with(|| {
        let chain_a = Chains::ChainA;
        let fee_asset = Chains::ChainA.get_main_asset();

        assert_ok!(Rings::send_call(
            Origin::Multisig(MultisigInternalOrigin::new(0u32)).into(),
            chain_a.clone(),
            Weight::from_parts(5000000000, 0),
            fee_asset,
            10000000000000u128,
            vec![1, 2, 3].try_into().unwrap()
        ));
    })
}

#[test]
fn send_call_fails() {
    ExtBuilder::default().build().execute_with(|| {
        let chain_a = Chains::ChainA;
        let fee_asset = Chains::ChainA.get_main_asset();

        // Wrong origin.
        assert_err!(
            Rings::send_call(
                RawOrigin::Signed(ALICE).into(),
                chain_a.clone(),
                Weight::from_parts(5000000000, 0),
                fee_asset.clone(),
                10000000000000u128,
                vec![1, 2, 3].try_into().unwrap()
            ),
            BadOrigin
        );

        // Chain under maintenance.
        Rings::set_maintenance_status(RawOrigin::Root.into(), chain_a.clone(), true).unwrap();
        assert_err!(
            Rings::send_call(
                Origin::Multisig(MultisigInternalOrigin::new(0u32)).into(),
                chain_a,
                Weight::from_parts(5000000000, 0),
                fee_asset,
                10000000000000u128,
                vec![1, 2, 3].try_into().unwrap()
            ),
            Error::<Test>::ChainUnderMaintenance
        );
    })
}

#[test]
fn transfer_assets_works() {
    ExtBuilder::default().build().execute_with(|| {
        let asset = Chains::ChainA.get_main_asset();

        assert_ok!(Rings::transfer_assets(
            Origin::Multisig(MultisigInternalOrigin::new(0u32)).into(),
            asset.clone(),
            100000000000000u128,
            ALICE,
            asset.clone(),
            10000000000000u128,
        ));
    })
}

#[test]
fn transfer_assets_fails() {
    ExtBuilder::default().build().execute_with(|| {
        let asset = Chains::ChainA.get_main_asset();
        let other_asset = Chains::ChainB.get_main_asset();

        // Wrong origin.
        assert_err!(
            Rings::transfer_assets(
                RawOrigin::Signed(ALICE).into(),
                asset.clone(),
                100000000000000u128,
                ALICE,
                asset.clone(),
                10000000000000u128,
            ),
            BadOrigin
        );

        // Fee asset is from a different chain.
        assert_err!(
            Rings::transfer_assets(
                Origin::Multisig(MultisigInternalOrigin::new(0u32)).into(),
                asset.clone(),
                100000000000000u128,
                ALICE,
                other_asset.clone(),
                10000000000000u128,
            ),
            Error::<Test>::DifferentChains
        );

        // Chain under maintenance.
        Rings::set_maintenance_status(RawOrigin::Root.into(), asset.get_chain().clone(), true)
            .unwrap();
        assert_err!(
            Rings::transfer_assets(
                Origin::Multisig(MultisigInternalOrigin::new(0u32)).into(),
                asset.clone(),
                100000000000000u128,
                ALICE,
                asset.clone(),
                10000000000000u128,
            ),
            Error::<Test>::ChainUnderMaintenance
        );
    })
}

#[test]
fn bridge_assets_works() {
    ExtBuilder::default().build().execute_with(|| {
        let asset = Chains::ChainA.get_main_asset();
        let destination = Chains::ChainB;

        assert_ok!(Rings::bridge_assets(
            Origin::Multisig(MultisigInternalOrigin::new(0u32)).into(),
            asset.clone(),
            destination.clone(),
            10000000000000u128,
            100000000000000u128,
            Some(ALICE),
        ));

        assert_ok!(Rings::bridge_assets(
            Origin::Multisig(MultisigInternalOrigin::new(0u32)).into(),
            asset.clone(),
            destination.clone(),
            10000000000000u128,
            100000000000000u128,
            None,
        ));
    })
}

#[test]
fn bridge_assets_fails() {
    ExtBuilder::default().build().execute_with(|| {
        let asset = Chains::ChainA.get_main_asset();
        let destination = Chains::ChainB;

        // Wrong origin.
        assert_err!(
            Rings::bridge_assets(
                RawOrigin::Signed(ALICE).into(),
                asset.clone(),
                destination.clone(),
                10000000000000u128,
                100000000000000u128,
                None,
            ),
            BadOrigin
        );

        // Chain under maintenance.
        Rings::set_maintenance_status(RawOrigin::Root.into(), asset.get_chain().clone(), true)
            .unwrap();
        assert_err!(
            Rings::bridge_assets(
                Origin::Multisig(MultisigInternalOrigin::new(0u32)).into(),
                asset.clone(),
                destination.clone(),
                10000000000000u128,
                100000000000000u128,
                None,
            ),
            Error::<Test>::ChainUnderMaintenance
        );
        Rings::set_maintenance_status(RawOrigin::Root.into(), asset.get_chain().clone(), false)
            .unwrap();
        Rings::set_maintenance_status(RawOrigin::Root.into(), destination.clone(), true).unwrap();
        assert_err!(
            Rings::bridge_assets(
                Origin::Multisig(MultisigInternalOrigin::new(0u32)).into(),
                asset.clone(),
                destination.clone(),
                10000000000000u128,
                100000000000000u128,
                None,
            ),
            Error::<Test>::ChainUnderMaintenance
        );
    })
}

#[test]
fn mutate_location_if_dest_is_relay() {
    let relay_dest = Chains::Relay.get_location();
    let para_dest = Chains::ChainA.get_location();

    let mut dao_multilocation = MultiLocation {
        parents: 1,
        interior: Junctions::X2(
            Junction::Parachain(2125),
            Junction::Plurality {
                id: BodyId::Index(0),
                part: BodyPart::Voice,
            },
        ),
    };

    crate::pallet::mutate_if_relay(&mut dao_multilocation, &para_dest);

    assert_eq!(
        dao_multilocation,
        MultiLocation {
            parents: 1,
            interior: Junctions::X2(
                Junction::Parachain(2125),
                Junction::Plurality {
                    id: BodyId::Index(0),
                    part: BodyPart::Voice,
                }
            )
        }
    );

    crate::pallet::mutate_if_relay(&mut dao_multilocation, &relay_dest);

    assert_eq!(
        dao_multilocation,
        MultiLocation {
            parents: 0,
            interior: Junctions::X2(
                Junction::Parachain(2125),
                Junction::Plurality {
                    id: BodyId::Index(0),
                    part: BodyPart::Voice,
                }
            )
        }
    );
}
