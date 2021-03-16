//! Mock for the sert-tes module.
//! Mocks for the Stp258 currencies module.

#![cfg(test)]

use super::*;
use frame_support::{construct_runtime, parameter_types};
use stp258_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, IdentityLookup},
	AccountId32, ModuleId,
};

use crate as serp_tes;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

pub type AccountId = AccountId32;
impl frame_system::Config for Runtime {
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

type CurrencyId = u32;
type Balance = u64;
type Price = FixedU128;
type BaseUnit = u64;

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Module<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

parameter_types! {
	pub DustAccount: AccountId = ModuleId(*b"dsss/dst").into_account();
}

impl stp258_tokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Amount = i64;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = stp258_tokens::TransferDust<Runtime, DustAccount>;
}

pub const STP258_NATIVE_ID: CurrencyId = 1;
pub const STP258_TOKEN_ID: CurrencyId = 2;

const STP258_BASE_UNIT:BaseUnit = 1000;

parameter_types! {
	pub const GetStp258NativeId: CurrencyId = STP258_NATIVE_ID;
	pub const GetBaseUnit: BaseUnit =  STP258_BASE_UNIT;
}

impl stp258_currencies::Config for Runtime {
	type Event = Event;
	type Stp258Currency = Stp258Tokens;
	type Stp258Native = AdaptedStp258Asset;
	type GetStp258NativeId = GetStp258NativeId;
	type GetBaseUnit = GetBaseUnit;
	type WeightInfo = ();
}

parameter_types! {
	pub const GetSettPayAcc: AccountId = SETT_PAY_ACC;
}

impl serp_market::Config for Runtime {
	type Event = Event;
	type GetNativeAssetId = GetNativeAssetId;
	type SettCurrency = Stp258Currency;
	type Price = Price;
	type Quote = Quote;
	type GetBaseUnit = GetBaseUnit;
	type GetSerpQuoteMultiple = GetSerpQuoteMultiple;
	type GetSettPayAcc = GetSettPayAcc;
	type GetSerperAcc = GetSerperAcc;
	type NativeAsset = AdaptedStp258Asset;
}

const ELAST_ADJUSTMENT_FREQUENCY: Blocknumber = 10;

parameter_types! {
	pub const ElastAdjustmentFrequency: Blocknumber =  ELAST_ADJUSTMENT_FREQUENCY;
}

impl Config for Runtime {
	type Event = Event;
	type SettCurrency = Stp258Currency;
	type SerpMarket = SerpMarket;
	type ElastAdjustmentFrequency = ElastAdjustmentFrequency;
	type WeightInfo = ();
}

pub type Stp258Native = Stp258NativeOf<Runtime>;
pub type AdaptedStp258Asset = Stp258AssetAdapter<Runtime, PalletBalances, i64, u64>;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Module, Call, Storage, Config, Event<T>},
		SerpTes: serp_tes::{Module, Call, Event<T>},
		SerpMarket: serp_market::{Module, Call, Event<T>},
		Stp258Currency: stp258_currencies::{Module, Call, Event<T>},
		Stp258Tokens: stp258_tokens::{Module, Storage, Event<T>, Config<T>},
		PalletBalances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
	}
);

pub const SERPER_ACC: AccountId = AccountId32::new([1u8; 32]);
pub const SETT_PAY_ACC: AccountId = AccountId32::new([2u8; 32]);
pub const ID_1: LockIdentifier = *b"1       ";

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn balances(mut self, endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.endowed_accounts = endowed_accounts;
		self
	}

	pub fn two_hundred_thousand_for_sett_pay_n_serper(self) -> Self {
		self.balances(vec![
			(SETT_PAY_ACC, STP258_NATIVE_ID, 200_000),
			(SERPER_ACC, STP258_NATIVE_ID, 200_000),
			(SETT_PAY_ACC, STP258_TOKEN_ID, 200_000 * STP258_BASE_UNIT),
			(SERPER_ACC, STP258_TOKEN_ID, 200_000 * STP258_BASE_UNIT),
		])
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self
				.endowed_accounts
				.clone()
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id == STP258_NATIVE_ID)
				.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		stp258_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self
				.endowed_accounts
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id != STP258_NATIVE_ID)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
