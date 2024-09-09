mod parachain;
mod relay_chain;

use frame_support::__private::sp_tracing;
use sp_runtime::{traits::TryConvert, BuildStorage};
use xcm::prelude::*;
use xcm_executor::traits::ConvertLocation;
use xcm_simulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain, TestExt};

pub const ALICE: sp_runtime::AccountId32 = sp_runtime::AccountId32::new([0u8; 32]);
pub const BOB: sp_runtime::AccountId32 = sp_runtime::AccountId32::new([1u8; 32]);
pub const CORE_0: sp_runtime::AccountId32 = sp_runtime::AccountId32::new([
    147, 83, 7, 98, 71, 245, 98, 15, 146, 176, 22, 221, 20, 216, 188, 203, 166, 234, 117, 86, 56,
    214, 204, 37, 238, 26, 161, 82, 2, 174, 180, 74,
]);
pub const INITIAL_BALANCE: u128 = 1_000_000_000_000_000;

decl_test_parachain! {
    pub struct Tinkernet {
        Runtime = parachain::Runtime,
        XcmpMessageHandler = parachain::MsgQueue,
        DmpMessageHandler = parachain::MsgQueue,
        new_ext = para_ext(2125),
    }
}

decl_test_parachain! {
    pub struct ParaB {
        Runtime = parachain::Runtime,
        XcmpMessageHandler = parachain::MsgQueue,
        DmpMessageHandler = parachain::MsgQueue,
        new_ext = para_ext(1000),
    }
}

decl_test_relay_chain! {
    pub struct Relay {
        Runtime = relay_chain::Runtime,
        RuntimeCall = relay_chain::RuntimeCall,
        RuntimeEvent = relay_chain::RuntimeEvent,
        XcmConfig = relay_chain::XcmConfig,
        MessageQueue = relay_chain::MessageQueue,
        System = relay_chain::System,
        new_ext = relay_ext(),
    }
}

decl_test_network! {
    pub struct MockNet {
        relay_chain = Relay,
        parachains = vec![
            (2125, Tinkernet),
            (1000, ParaB),
        ],
    }
}

pub fn child_account_id(para: u32) -> relay_chain::AccountId {
    let location = (Parachain(para),);
    relay_chain::LocationToAccountId::convert_location(&MultiLocation::from(location)).unwrap()
}

pub fn _child_account_account_id(
    para: u32,
    who: sp_runtime::AccountId32,
) -> relay_chain::AccountId {
    let location = (
        Parachain(para),
        AccountId32 {
            network: None,
            id: who.into(),
        },
    );
    relay_chain::LocationToAccountId::convert_location(&MultiLocation::from(location)).unwrap()
}

pub fn sibling_dao_account_id(dao: u32) -> parachain::AccountId {
    let location = MultiLocation {
        parents: 1,
        interior: Junctions::X2(
            Parachain(2125),
            Plurality {
                id: BodyId::Index(dao),
                part: BodyPart::Voice,
            },
        ),
    };

    parachain::HashedDescription::try_convert(location.into()).unwrap()
}

pub fn _tinkernet_ext(_para_id: u32) -> sp_io::TestExternalities {
    use crate::{PolkadotXcm, Runtime, RuntimeOrigin, System};

    let mut t = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![(ALICE, INITIAL_BALANCE), (CORE_0, INITIAL_BALANCE)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        sp_tracing::try_init_simple();
        System::set_block_number(1);
        let _ = PolkadotXcm::force_xcm_version(
            RuntimeOrigin::root(),
            Box::new(MultiLocation::new(
                1,
                Junctions::X1(Junction::Parachain(1000)),
            )),
            3,
        );

        System::set_block_number(2);
    });
    ext
}

pub fn para_ext(para_id: u32) -> sp_io::TestExternalities {
    #[allow(unused_imports)]
    use parachain::{MsgQueue, PolkadotXcm, Runtime, RuntimeOrigin, System};

    let mut t = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![(ALICE, INITIAL_BALANCE), (CORE_0, INITIAL_BALANCE)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        sp_tracing::try_init_simple();
        System::set_block_number(1);
        MsgQueue::set_para_id(para_id.into());
    });
    ext
}

pub fn relay_ext() -> sp_io::TestExternalities {
    #[allow(unused_imports)]
    use relay_chain::{Runtime, RuntimeOrigin, System};

    let mut t = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![
            (ALICE, INITIAL_BALANCE),
            (child_account_id(1), INITIAL_BALANCE),
            (child_account_id(2), INITIAL_BALANCE),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}

pub type _RelayChainPalletXcm = pallet_xcm::Pallet<relay_chain::Runtime>;
pub type _ParachainPalletXcm = pallet_xcm::Pallet<parachain::Runtime>;

#[allow(unused_imports)]
#[cfg(test)]
mod tests {
    use super::*;
    use parachain::INV4;

    use codec::Encode;
    use frame_support::{assert_ok, weights::Weight};
    use pallet_rings::ChainList;
    use xcm::latest::QueryResponseInfo;
    use xcm_simulator::TestExt;

    // Helper function for forming buy execution message
    fn _buy_execution<C>(fees: impl Into<MultiAsset>) -> Instruction<C> {
        BuyExecution {
            fees: fees.into(),
            weight_limit: Unlimited,
        }
    }

    #[test]
    fn remote_account_ids_work() {
        MockNet::reset();

        Tinkernet::execute_with(|| {
            assert_eq!(
                <parachain::INV4 as pallet_dao_manager::DaoAccountDerivation<
                    parachain::Runtime,
                >>::derive_dao_account(0),
                sibling_dao_account_id(0)
            );
        });
    }

    #[test]
    fn rings_transfer_work() {
        MockNet::reset();

        Tinkernet::execute_with(|| {
            log::trace!(target: "xcm::DaoAccount", "DaoAccount: account: {:?}", <parachain::INV4 as pallet_dao_manager::DaoAccountDerivation<
                    parachain::Runtime,
                >>::derive_dao_account(0));

            assert_ok!(parachain::Rings::transfer_assets(
                pallet_dao_manager::Origin::<parachain::Runtime>::Multisig(
                    pallet_dao_manager::origin::MultisigInternalOrigin::<parachain::Runtime>::new(
                        0
                    )
                )
                .into(),
                ChainList::get_main_asset(&crate::rings::Chains::AssetHub),
                10_000000000000u128,
                BOB,
                ChainList::get_main_asset(&crate::rings::Chains::AssetHub),
                1_000000000000u128
            ));
        });

        ParaB::execute_with(|| {
            assert_eq!(parachain::Balances::free_balance(BOB), 10_000000000000u128);
        });
    }
}
