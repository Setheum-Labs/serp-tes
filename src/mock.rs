//! Mocks for the SerpMarket module.

#![cfg(test)]

use super::*;
use frame_support::{construct_runtime, parameter_types};
use stp258_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, IdentityLookup},
	AccountId32, ModuleId, Perbill,
};

use crate as serp;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

pub type AccountId = AccountId32;
impl frame_system::Config for Runtime {
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = Blocknumber;
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
type Blocknumber = u64;

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

parameter_type_with_key! {
	pub GetBaseUnit: |currency_id: CurrencyId| -> Balance {
		match currency_id {
			&SETT => 10_000,
			&JUSD => 1_000,
			_ => 0,
		}
	};
}

const PERCENT: Balance = 100;
const SERP_QUOTE_MULTIPLE: Balance = 2;
const SINGLE_UNIT: Balance = 1;
const SERPER_RATIO: Perbill = Perbill::from_percent(25);
const SETT_PAY_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
	pub DustAccount: AccountId = ModuleId(*b"dsss/dst").into_account();
}

parameter_types! {
	pub const GetSerperAcc: AccountId = SERPER;
	pub const GetSerpQuoteMultiple: Balance = SERP_QUOTE_MULTIPLE;
	pub const GetSettPayAcc: AccountId = SETTPAY;
	pub const GetSingleUnit: Balance = SINGLE_UNIT;
	pub const GetSerperRatio: Perbill = SERPER_RATIO;
	pub const GetSettPayRatio: Perbill = SETT_PAY_RATIO;
	pub const GetSerpNativeId: CurrencyId = DNAR;
	pub const GetPercent: Balance = PERCENT;
	pub const AdjustmentFrequency: Blocknumber = ADJUSTMENT_FREQUENCY;
}

impl stp258_tokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Amount = i64;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type GetBaseUnit = GetBaseUnit;
	type AdjustmentFrequency = AdjustmentFrequency;
	type GetPercent = GetPercent;
	type GetSerpNativeId = GetSerpNativeId;
	type GetSerpQuoteMultiple = GetSerpQuoteMultiple;
	type GetSerperAcc = GetSerperAcc;
	type GetSettPayAcc = GetSettPayAcc;
	type GetSerperRatio = GetSerperRatio;
	type GetSettPayRatio = GetSettPayRatio;
	type GetSingleUnit = GetSingleUnit;
	type OnDust = stp258_tokens::TransferDust<Runtime, DustAccount>;
}

pub const DNAR: CurrencyId = 1;
pub const SETT: CurrencyId = 2;
pub const JUSD: CurrencyId = 3;

pub const ADJUSTMENT_FREQUENCY: Blocknumber = 10;

parameter_types! {
	pub const GetStp258NativeId: CurrencyId = DNAR;
}

impl stp258_standard::Config for Runtime {
	type Event = Event;
	type Stp258Currency = Stp258Tokens;
	type Stp258Native = AdaptedStp258Asset;
	type GetStp258NativeId = GetStp258NativeId;
	type WeightInfo = ();
}
pub type Stp258Native = Stp258NativeOf<Runtime>;
pub type AdaptedStp258Asset = Stp258AssetAdapter<Runtime, PalletBalances, i64, u64>;

impl Config for Runtime {
	type Event = Event;
	type Stp258Currency = Stp258Tokens;
	type Stp258Native = AdaptedStp258Asset;
	type GetStp258NativeId = GetStp258NativeId;
	type WeightInfo = ();
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Module, Call, Storage, Config, Event<T>},
		Serp: serp::{Module, Call, Event<T>},
		Stp258Standard: stp258_standard::{Module, Call, Event<T>},
		Stp258Tokens: stp258_tokens::{Module, Storage, Event<T>, Config<T>},
		PalletBalances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
	}
);

pub const ALICE: AccountId = AccountId32::new([0u8; 32]);
pub const BOB: AccountId = AccountId32::new([1u8; 32]);
pub const SERPER: AccountId = AccountId32::new([3u8; 32]);
pub const SETTPAY: AccountId = AccountId32::new([4u8; 32]);

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

	pub fn one_hundred_for_alice_n_bob_n_serper_n_settpay(self) -> Self {
		self.balances(vec![
			(ALICE, DNAR, 100), 
			(BOB, DNAR, 100),
			(SERPER, DNAR, 100),
			(SETTPAY, DNAR, 100),
			(ALICE, SETT, 100 * 10_000), 
			(BOB, SETT, 100 * 10_000),
			(SERPER, SETT, 100 * 10_000),
			(SETTPAY, SETT, 100 * 10_000),
			(ALICE, JUSD, 100 * 1_000), 
			(BOB, JUSD, 100 * 1_000),
			(SERPER, JUSD, 100 * 1_000),
			(SETTPAY, JUSD, 100 * 1_000),
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
				.filter(|(_, currency_id, _)| *currency_id == DNAR)
				.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		stp258_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self
				.endowed_accounts
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id != DNAR)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
