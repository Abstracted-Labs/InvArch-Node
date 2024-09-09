use crate::{fee_handling::*, *};
use codec::{Decode, Encode};
use core::convert::TryFrom;
use frame_support::{
    derive_impl, parameter_types,
    traits::{
        fungibles::Credit, ConstU128, ConstU32, ConstU64, Contains, Currency, EnsureOrigin,
        EnsureOriginWithArg,
    },
    weights::ConstantMultiplier,
};
use frame_system::EnsureRoot;
use orml_asset_registry::AssetMetadata;
use pallet_balances::AccountData;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{AccountId32, BuildStorage};
use sp_std::{convert::TryInto, vec};

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;

type AccountId = AccountId32;

pub const EXISTENTIAL_DEPOSIT: Balance = 1_000_000_000;

pub const ALICE: AccountId = AccountId::new([0u8; 32]);
pub const BOB: AccountId = AccountId::new([1u8; 32]);
pub const CHARLIE: AccountId = AccountId::new([2u8; 32]);
pub const DAVE: AccountId = AccountId::new([3u8; 32]);

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Tokens: orml_tokens,
        AssetRegistry: orml_asset_registry,
        CoreAssets: orml_tokens2,
        INV4: pallet,
    }
);

pub struct TestBaseCallFilter;
impl Contains<RuntimeCall> for TestBaseCallFilter {
    fn contains(_c: &RuntimeCall) -> bool {
        true
    }
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type RuntimeOrigin = RuntimeOrigin;
    type Nonce = u64;
    type RuntimeCall = RuntimeCall;
    type Hash = H256;
    type Hashing = ::sp_runtime::traits::BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = sp_runtime::traits::IdentityLookup<AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type BlockWeights = ();
    type BlockLength = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type DbWeight = ();
    type BaseCallFilter = TestBaseCallFilter;
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<50>;
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
    type AccountStore = System;
    type MaxReserves = ConstU32<50>;
    type ReserveIdentifier = [u8; 8];
    type MaxFreezes = ConstU32<1>;
}

const UNIT: u128 = 1000000000000;
const MICROUNIT: Balance = 1_000_000;

pub struct DaoDustRemovalWhitelist;
impl Contains<AccountId> for DaoDustRemovalWhitelist {
    fn contains(_: &AccountId) -> bool {
        true
    }
}

pub struct DisallowIfFrozen;
impl
    orml_traits2::currency::OnTransfer<
        <Test as frame_system::Config>::AccountId,
        <Test as pallet::Config>::DaoId,
        Balance,
    > for DisallowIfFrozen
{
    fn on_transfer(
        currency_id: <Test as pallet::Config>::DaoId,
        _from: &AccountId,
        _to: &AccountId,
        _amount: Balance,
    ) -> sp_std::result::Result<(), orml_traits::parameters::sp_runtime::DispatchError> {
        if let Some(true) = INV4::is_asset_frozen(currency_id) {
            Err(sp_runtime::DispatchError::Token(
                sp_runtime::TokenError::Frozen,
            ))
        } else {
            Ok(())
        }
    }
}

pub struct HandleNewMembers;
impl
    orml_traits2::Happened<(
        <Test as frame_system::Config>::AccountId,
        <Test as pallet::Config>::DaoId,
    )> for HandleNewMembers
{
    fn happened((member, dao_id): &(AccountId, <Test as pallet::Config>::DaoId)) {
        INV4::add_member(dao_id, member)
    }
}

pub struct HandleRemovedMembers;
impl
    orml_traits2::Happened<(
        <Test as frame_system::Config>::AccountId,
        <Test as pallet::Config>::DaoId,
    )> for HandleRemovedMembers
{
    fn happened((member, dao_id): &(AccountId, <Test as pallet::Config>::DaoId)) {
        INV4::remove_member(dao_id, member)
    }
}

pub struct INV4TokenHooks;
impl
    orml_traits2::currency::MutationHooks<
        <Test as frame_system::Config>::AccountId,
        <Test as pallet::Config>::DaoId,
        Balance,
    > for INV4TokenHooks
{
    type PreTransfer = DisallowIfFrozen;
    type OnDust = ();
    type OnSlash = ();
    type PreDeposit = ();
    type PostDeposit = ();
    type PostTransfer = ();
    type OnNewTokenAccount = HandleNewMembers;
    type OnKilledTokenAccount = HandleRemovedMembers;
}

orml_traits2::parameter_type_with_key! {
    pub DaoExistentialDeposits: |_currency_id: <Test as pallet::Config>::DaoId| -> Balance {
        CExistentialDeposit::get()
    };
}

impl orml_tokens2::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = i128;
    type CurrencyId = <Test as pallet::Config>::DaoId;
    type WeightInfo = ();
    type ExistentialDeposits = DaoExistentialDeposits;
    type MaxLocks = ConstU32<0u32>;
    type MaxReserves = ConstU32<0u32>;
    type DustRemovalWhitelist = DaoDustRemovalWhitelist;
    type ReserveIdentifier = [u8; 8];
    type CurrencyHooks = INV4TokenHooks;
}

parameter_types! {
    pub const MaxMetadata: u32 = 10000;
    pub const MaxCallers: u32 = 10000;
    pub const DaoSeedBalance: Balance = 1000000u128;
    pub const DaoCreationFee: Balance = UNIT;
    pub const GenesisHash: <Test as frame_system::Config>::Hash = H256([
        212, 46, 150, 6, 169, 149, 223, 228, 51, 220, 121, 85, 220, 42, 112, 244, 149, 243, 80,
        243, 115, 218, 162, 0, 9, 138, 232, 68, 55, 129, 106, 210,
    ]);

    pub const RelayDaoCreationFee: Balance = UNIT;
}

pub type AssetId = u32;

pub const NATIVE_ASSET_ID: AssetId = 0;
pub const RELAY_ASSET_ID: AssetId = 1;

parameter_types! {
    pub const NativeAssetId: AssetId = NATIVE_ASSET_ID;
    pub const RelayAssetId: AssetId = RELAY_ASSET_ID;
    pub const ExistentialDeposit: u128 = 100000000000;
    pub const CExistentialDeposit: u128 = 1;
    pub const MaxLocks: u32 = 1;
    pub const MaxReserves: u32 = 1;
    pub const MaxCallSize: u32 = 50 * 1024;
    pub const StringLimit: u32 = 2125;
    pub const TransactionByteFee: Balance = 10 * MICROUNIT;

}

pub struct AssetAuthority;
impl EnsureOriginWithArg<RuntimeOrigin, Option<u32>> for AssetAuthority {
    type Success = ();

    fn try_origin(
        origin: RuntimeOrigin,
        _asset_id: &Option<u32>,
    ) -> Result<Self::Success, RuntimeOrigin> {
        <EnsureRoot<_> as EnsureOrigin<RuntimeOrigin>>::try_origin(origin)
    }

    fn ensure_origin(
        o: RuntimeOrigin,
        a: &Option<u32>,
    ) -> Result<Self::Success, sp_runtime::traits::BadOrigin> {
        Self::try_origin(o, a).map_err(|_| sp_runtime::traits::BadOrigin)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin(_o: &Option<u32>) -> Result<RuntimeOrigin, ()> {
        Err(())
    }
}

impl orml_asset_registry::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AuthorityOrigin = AssetAuthority;
    type AssetId = AssetId;
    type Balance = Balance;
    type AssetProcessor = orml_asset_registry::SequentialId<Test>;
    type CustomMetadata = ();
    type WeightInfo = ();
    type StringLimit = StringLimit;
}

pub struct DustRemovalWhitelist;
impl Contains<AccountId> for DustRemovalWhitelist {
    fn contains(_: &AccountId) -> bool {
        true
    }
}

pub type Amount = i128;

orml_traits::parameter_type_with_key! {
      pub ExistentialDeposits: |currency_id: AssetId| -> Balance {
          if currency_id == &RELAY_ASSET_ID {
              ExistentialDeposit::get()
          } else {
              orml_asset_registry::ExistentialDeposits::<Test>::get(currency_id)
          }
      };
}

impl orml_tokens::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type MaxLocks = MaxLocks;
    type DustRemovalWhitelist = DaoDustRemovalWhitelist;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type CurrencyHooks = ();
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo, Debug)]
pub struct FeeCharger;

impl MultisigFeeHandler<Test> for FeeCharger {
    type Pre = (
        // tip
        Balance,
        // who paid the fee
        AccountId,
        // imbalance resulting from withdrawing the fee
        (),
        // asset_id for the transaction payment
        Option<AssetId>,
    );

    fn pre_dispatch(
        fee_asset: &FeeAsset,
        who: &AccountId,
        _call: &RuntimeCall,
        _info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        _len: usize,
    ) -> Result<Self::Pre, frame_support::unsigned::TransactionValidityError> {
        Ok((
            0u128,
            who.clone(),
            (),
            match fee_asset {
                FeeAsset::Native => None,
                FeeAsset::Relay => Some(1u32),
            },
        ))
    }

    fn post_dispatch(
        _fee_asset: &FeeAsset,
        _pre: Option<Self::Pre>,
        _info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        _post_info: &sp_runtime::traits::PostDispatchInfoOf<RuntimeCall>,
        _len: usize,
        _result: &sp_runtime::DispatchResult,
    ) -> Result<(), frame_support::unsigned::TransactionValidityError> {
        Ok(())
    }

    fn handle_creation_fee(
        _imbalance: FeeAssetNegativeImbalance<
            <Balances as Currency<AccountId>>::NegativeImbalance,
            Credit<AccountId, Tokens>,
        >,
    ) {
    }
}

impl pallet::Config for Test {
    type MaxMetadata = MaxMetadata;
    type DaoId = u32;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type RuntimeCall = RuntimeCall;
    type MaxCallers = MaxCallers;
    type DaoSeedBalance = DaoSeedBalance;
    type AssetsProvider = CoreAssets;
    type RuntimeOrigin = RuntimeOrigin;
    type DaoCreationFee = DaoCreationFee;
    type FeeCharger = FeeCharger;
    type WeightInfo = crate::weights::SubstrateWeight<Test>;

    type Tokens = Tokens;
    type RelayAssetId = RelayAssetId;
    type RelayDaoCreationFee = RelayDaoCreationFee;

    type MaxCallSize = MaxCallSize;

    type ParaId = ConstU32<2125>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
}

pub struct ExtBuilder;

impl Default for ExtBuilder {
    fn default() -> Self {
        ExtBuilder
    }
}

pub const INITIAL_BALANCE: Balance = 100000000000000000;

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();

        pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (ALICE, INITIAL_BALANCE),
                (BOB, INITIAL_BALANCE),
                (CHARLIE, INITIAL_BALANCE),
                (INV4::derive_dao_account(0u32), INITIAL_BALANCE),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        orml_asset_registry::GenesisConfig::<Test> {
            assets: vec![
                (
                    0u32,
                    AssetMetadata {
                        decimals: 12,
                        name: sp_core::bounded_vec::BoundedVec::<u8, StringLimit>::new(),
                        symbol: sp_core::bounded_vec::BoundedVec::<u8, StringLimit>::new(),
                        existential_deposit: ExistentialDeposit::get(),
                        location: None,
                        additional: (),
                    }
                    .encode(),
                ),
                (
                    1u32,
                    AssetMetadata {
                        decimals: 12,
                        name: sp_core::bounded_vec::BoundedVec::<u8, StringLimit>::new(),
                        symbol: sp_core::bounded_vec::BoundedVec::<u8, StringLimit>::new(),
                        existential_deposit: ExistentialDeposit::get(),
                        location: None,
                        additional: (),
                    }
                    .encode(),
                ),
            ],
            last_asset_id: 1u32,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        orml_tokens::GenesisConfig::<Test> {
            balances: vec![
                (ALICE, RELAY_ASSET_ID, INITIAL_BALANCE),
                (BOB, RELAY_ASSET_ID, INITIAL_BALANCE),
                (CHARLIE, RELAY_ASSET_ID, INITIAL_BALANCE),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| System::set_block_number(0));

        ext
    }
}
