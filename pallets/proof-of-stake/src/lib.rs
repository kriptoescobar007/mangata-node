#![cfg_attr(not(feature = "std"), no_std)]

//! # Proof of Stake Module
//!
//! The goal of the Proof of Stake module is to reward people for providing liquidity to the Mangata DEX.
//!
//! ## Types of Rewards
//!
//! ### Native Rewards
//!
//! As described in Mangata tokenomics, during each session, some of the rewards are minted and distributed
//! among promoted pools. The council decides which pool to promote, and each promoted pool has a weight
//! assigned that determines how much of the rewards should
//! be distributed to that pool.
//!
//! The final amount of tokens that a user receives depends on:
//! - the amount of activated liquidity - rewards are distributed proportionally to the amount of
//!   activated liquidity.
//! - the liquidity token itself - the more weight is assigned to the pool, the more rewards it receives.
//! - the time of liquidity activation - the longer a user stores liquidity, the more rewards they receive
//!   (based on an asymptotic curve).
//!
//! Activated Liquidity cannot be transferred to another account; it is considered locked. The moment
//! liquidity is unlocked, the user loses the ability to claim rewards for that liquidity.
//!
//! #### Storage entries
//!
//! - [`TotalActivatedLiquidity`] - Stores information about the total amount of activated liquidity for
//! each liquidity token.
//! - [`PromotedPoolRewards`] - Stores information about the total amount of rewards for each liquidity
//! token.
//! - [`RewardsInfo`] - Stores information about rewards for liquidity mining.
//! - [`ThirdPartyActivationKind`] - Wrapper over origin ActivateKind that is used in
//!
//! #### Extrinsics
//!
//! - [`Pallet::activate_liquidity`] - Activates liquidity for liquidity mining rewards.
//! - [`Pallet::deactivate_liquidity_for_native_rewards`] - Deactivates liquidity for liquidity mining rewards.
//! - [`Pallet::claim_native_rewards`] - Claims all rewards for all liquidity tokens.
//! - [`Pallet::update_pool_promotion`] - Enables/disables the pool for liquidity mining rewards.
//!
//! ### 3rd Party Rewards
//!
//! Anyone can provide tokens to reward people who store a particular liquidity token. Any
//! liquidity token can be rewarded with any other token provided by the user. Liquidity can be
//! activated for multiple scheduled rewards related to that liquidity token. Tokens will remain
//! locked (untransferable) as long as there is at least one schedule for which these rewards are
//! activated.
//!
//! #### Storage entries
//!
//! - [`RewardsInfoForScheduleRewards`] - Stores information about rewards for scheduled rewards.
//! - [`ScheduleRewardsPerSingleLiquidity`] - Stores the amount of rewards per single liquidity token.
//! - [`RewardsSchedules`] - Stores information about scheduled rewards.
//! - [`ScheduleId`] - Stores the unique id of the schedule.
//! - [`RewardTokensPerPool`] - Stores information about which reward tokens are used for a particular
//! liquidity token.
//! - [`TotalActivatedLiquidityForSchedules`] - Stores information about the total amount of activated
//! liquidity for each schedule.
//! - [`ActivatedLiquidityForSchedules`] - Stores information about how much liquidity was activated for
//! each schedule.
//! - [`ActivatedLockedLiquidityForSchedules`] - Stores information about how much liquidity was activated
//! for each schedule and not yet liquidity mining rewards.
//!
//!
//! #### Extrinsics
//!
//! - [`Pallet::reward_pool`] - Schedules rewards for the selected liquidity token.
//! - [`Pallet::activate_liquidity_for_3rdparty_rewards`] - Activates liquidity for scheduled rewards.
//! - [`Pallet::deactivate_liquidity_for_3rdparty_rewards`] - Deactivates liquidity for scheduled rewards.
//! - [`Pallet::claim_3rdparty_rewards`] - Claims all scheduled rewards for all liquidity tokens.
//!
//! ## Reusing a Single Liquidity Token for Multiple Rewards
//!
//! It may happen that a single liquidity token is rewarded with:
//! - Liquidity Mining Rewards - because the pool was promoted by the council.
//! - Scheduled rewards with token X - because Alice decided to do so.
//! - Scheduled rewards with token Y - because Bob decided to do so.
//!
//! In that case, a single liquidity token can be used to obtain rewards from multiple sources. There are
//! several options to do that:
//!
//! - The user can reuse liquidity used for liquidity mining rewards to claim scheduled rewards. In
//!   this case, [`Pallet::activate_liquidity_for_3rdparty_rewards`] should be used with [`ActivateKind::LiquidityMining`].
//!
//! - The user can reuse liquidity used for scheduled rewards (X) to sign up for rewards from other tokens (provided by Bob). In that case, [`Pallet::activate_liquidity_for_3rdparty_rewards`] should be used with [`ActivateKind::ActivatedLiquidity(X)`].
//!
//! - The user can't directly provide liquidity activated for scheduled rewards to activate it for liquidity mining rewards. Instead:
//!     * Liquidity used for schedule rewards can be deactivated
//!     [`Pallet::deactivate_liquidity_for_3rdparty_rewards`].
//!     * Liquidity can be activated for liquidity mining rewards [`Pallet::activate_liquidity`].
//!     * Liquidity can be activated for scheduled rewards [`Pallet::activate_liquidity_for_3rdparty_rewards`] with [`ThirdPartyActivationKind::Mining`].

use frame_support::pallet_prelude::*;

pub type ScheduleId = u64;

#[derive(Encode, Decode, Clone, Default, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct Schedule {
	scheduled_at: u64,
	last_session: u64,
	liq_token: TokenId,
	reward_token: TokenId,
	amount_per_session: Balance,
}

use frame_benchmarking::Zero;
use frame_support::{
	dispatch::{DispatchError, DispatchErrorWithPostInfo, DispatchResult, PostDispatchInfo},
	ensure,
	storage::bounded_btree_map::BoundedBTreeMap,
	traits::Nothing,
};
use frame_system::ensure_signed;
use mangata_support::traits::Valuate;
use sp_core::U256;
use sp_runtime::traits::AccountIdConversion;

use frame_support::{
	pallet_prelude::*,
	traits::{tokens::currency::MultiTokenCurrency, ExistenceRequirement, Get},
	transactional,
};
use frame_system::pallet_prelude::*;
use mangata_support::traits::{
	ActivationReservesProviderTrait, LiquidityMiningApi, ProofOfStakeRewardsApi, XykFunctionsTrait,
};
use mangata_types::{multipurpose_liquidity::ActivateKind, Balance, TokenId};
use orml_tokens::{MultiTokenCurrencyExtended, MultiTokenReservableCurrency};
use sp_std::collections::btree_map::BTreeMap;

use sp_runtime::{traits::SaturatedConversion, Perbill};
use sp_std::{convert::TryInto, prelude::*};

mod reward_info;
use reward_info::{AsymptoticCurveRewards, ConstCurveRewards, RewardInfo, RewardsCalculator};
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub(crate) const LOG_TARGET: &str = "proof-of-stake";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: $crate::LOG_TARGET,
			concat!("[{:?}] 💸 ", $patter), <frame_system::Pallet<T>>::block_number() $(, $values)*
		)
	};
}

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

/// Wrapper over origin ActivateKind that is used in [`Pallet::activat_liquidity`]
/// with extension that allows activating liquidity that was already used for:
/// - `ActivatedLiquidity` - already activated liquidity (for scheduled rewards)
/// - `LiquidityMining` - already activated liquidity (for liquidity mining rewards)
#[derive(Eq, PartialEq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub enum ThirdPartyActivationKind {
	ActivateKind(Option<ActivateKind>),
	ActivatedLiquidity(TokenId),
	LiquidityMining,
}

const PALLET_ID: frame_support::PalletId = frame_support::PalletId(*b"rewards!");

#[frame_support::pallet]
pub mod pallet {
	use frame_support::traits::Currency;
	use mangata_support::traits::PoolCreateApi;

	use super::*;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(n: T::BlockNumber) -> Weight {
			let session_id = Self::session_index() as u64;

			// NOTE: 1R
			if Self::is_new_session() {
				ScheduleListPos::<T>::kill();
				return Default::default()
			}

			const AMOUNT_PER_BLOCK: u64 = 5;

			for idx in 0..AMOUNT_PER_BLOCK {
				// READS PER ITERTION
				//
				// 			ON VALID SCHEDULE  (AVERAGE)                        ====> 3R + 1 W + N*R + N*W
				// 				3 x READ HEAD,TAIL,POS            : ALWAYS
				// 				1 x WRITE ScheduleListPos         : ALWAYS (pesimistic)
				// 				PER ITER:
				// 					- READ RewardsSchedulesList   : ALWAYS
				// 					- WRITE ScheduleRewardsTotal  : ALWAYS (pesemisitic)

				// 			ON OUTDATED SCHEDULE (PESIMITIC)                   =====> 3R + 1W + (N-1)*W + 2W
				// 				3 x READ HEAD,TAIL,POS                         : ALWAYS
				// 				1 x WRITE ScheduleListPos `next`               : ONCE (pesemisitic)
				// 				REMOVE N-1 SCHEDULES IN THE MIDDDLE
				// 					- 1 x WRITE update previous schedudle `next`  : ALWAYS (pesemisitic)
				// 				REMOVE LAST ELEM:
				// 					- 1 x WRITE update list tail
				// 					- 1 x WRITE update elem before last : ALWAYS (pesemisitic)

				// NOTE: 1R
				let last_valid = ScheduleListPos::<T>::get();
				// NOTE: 1R
				let pos = match (last_valid, ScheduleListHead::<T>::get()) {
					(Some(pos), _) => {
						if let Some((schedule, next)) = RewardsSchedulesList::<T>::get(pos) {
							next
						} else {
							None
						}
					},
					(None, Some(head)) => Some(head),
					_ => None,
				};

				if let Some(pos_val) = pos {
					// NOTE: 1R
					if let Some((schedule, next)) = RewardsSchedulesList::<T>::get(pos_val) {
						if schedule.last_session >= session_id {
							if schedule.scheduled_at < session_id {
								// NOTE: 1R 1W
								ScheduleRewardsTotal::<T>::mutate(
									(schedule.liq_token, schedule.reward_token),
									|(pending, idx, cumulative)| {
										if *idx >= session_id {
											*pending += schedule.amount_per_session
										} else {
											*cumulative += *pending;
											*pending = schedule.amount_per_session;
											*idx = session_id;
										}
									},
								);
							}
							// NOTE: 1W
							ScheduleListPos::<T>::put(pos_val);
						} else {
							// NOTE: 2R
							match (Self::head(), Self::tail()) {
								(Some(head), Some(tail)) if head == pos_val && head != tail =>
									if let Some(next) = next {
										// NOTE: 1W
										ScheduleListHead::<T>::put(next);
									},
								(Some(head), Some(tail)) if tail == pos_val && head == tail => {
									// NOTE: 3W
									ScheduleListTail::<T>::kill();
									ScheduleListHead::<T>::kill();
									ScheduleListPos::<T>::kill();
								},
								(Some(head), Some(tail)) if tail == pos_val && head != tail =>
									if let Some(last_valid) = last_valid {
										// NOTE: 1W
										ScheduleListTail::<T>::put(last_valid);
										// NOTE: 1R 1W
										RewardsSchedulesList::<T>::mutate(last_valid, |data| {
											if let Some((schedule, next)) = data.as_mut() {
												*next = None
											}
										});
									},
								(Some(head), Some(tail)) =>
									if let Some(last_valid) = last_valid {
										// NOTE: 1R 1W
										RewardsSchedulesList::<T>::mutate(last_valid, |data| {
											if let Some((schedule, prev_next)) = data.as_mut() {
												*prev_next = next
											}
										});
									},
								_ => {},
							}
						}
					}
				} else {
					break
				}
			}
			Default::default()
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	pub trait PoSBenchmarkingConfig: pallet_issuance::Config {}
	#[cfg(feature = "runtime-benchmarks")]
	impl<T: pallet_issuance::Config> PoSBenchmarkingConfig for T {}

	#[cfg(not(feature = "runtime-benchmarks"))]
	pub trait PoSBenchmarkingConfig {}
	#[cfg(not(feature = "runtime-benchmarks"))]
	impl<T> PoSBenchmarkingConfig for T {}

	#[cfg(not(feature = "runtime-benchmarks"))]
	pub trait ValutationApiTrait<T: Config>:
		Valuate<Balance = mangata_types::Balance, CurrencyId = mangata_types::TokenId>
	{
	}

	#[cfg(feature = "runtime-benchmarks")]
	pub trait ValutationApiTrait<T: Config>:
		Valuate<Balance = mangata_types::Balance, CurrencyId = mangata_types::TokenId>
		+ XykFunctionsTrait<T::AccountId>
	{
	}

	#[cfg(not(feature = "runtime-benchmarks"))]
	impl<T, C> ValutationApiTrait<C> for T
	where
		C: Config,
		T: Valuate<Balance = mangata_types::Balance, CurrencyId = mangata_types::TokenId>,
	{
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<T, C> ValutationApiTrait<C> for T
	where
		C: Config,
		T: Valuate<Balance = mangata_types::Balance, CurrencyId = mangata_types::TokenId>,
		T: XykFunctionsTrait<C::AccountId>,
	{
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + PoSBenchmarkingConfig {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type ActivationReservesProvider: ActivationReservesProviderTrait<
			AccountId = Self::AccountId,
		>;
		type NativeCurrencyId: Get<TokenId>;
		type Currency: MultiTokenCurrencyExtended<Self::AccountId>
			+ MultiTokenReservableCurrency<Self::AccountId>;
		#[pallet::constant]
		/// The account id that holds the liquidity mining issuance
		type LiquidityMiningIssuanceVault: Get<Self::AccountId>;
		#[pallet::constant]
		type RewardsDistributionPeriod: Get<u32>;
		/// The maximum number of schedules that can be active at one moment
		type RewardsSchedulesLimit: Get<u32>;
		/// The minimum number of rewards per session for schedule rewards
		type Min3rdPartyRewards: Get<u128>;
		type WeightInfo: WeightInfo;
		type ValuationApi: ValutationApiTrait<Self>;
	}

	#[pallet::error]
	/// Errors
	pub enum Error<T> {
		/// Not enought assets
		NotEnoughAssets,
		/// Math overflow
		MathOverflow,
		/// Not enough rewards earned
		NotEnoughRewardsEarned,
		/// Not a promoted pool
		NotAPromotedPool,
		/// Past time calculation
		PastTimeCalculation,
		LiquidityCheckpointMathError,
		CalculateRewardsMathError,
		MathError,
		CalculateRewardsAllMathError,
		MissingRewardsInfoError,
		DeprecatedExtrinsic,
		/// Cannot schedule rewards in past
		CannotScheduleRewardsInPast,
		/// Pool does not exist
		PoolDoesNotExist,
		/// Too many schedules
		TooManySchedules,
		/// Too little rewards per session
		TooLittleRewards,
		// Liquidity is reused for 3rdparty rewards
		LiquidityLockedIn3rdpartyRewards,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		PoolPromotionUpdated(TokenId, Option<u8>),
		LiquidityActivated(T::AccountId, TokenId, Balance),
		LiquidityDeactivated(T::AccountId, TokenId, Balance),
		RewardsClaimed(T::AccountId, TokenId, Balance),
	}

	#[pallet::storage]
	#[pallet::getter(fn get_rewards_info)]
	pub type RewardsInfo<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		AccountIdOf<T>,
		Twox64Concat,
		TokenId,
		RewardInfo,
		ValueQuery,
	>;

	#[pallet::storage]
	/// Stores information about pool weight and accumulated rewards. The accumulated
	/// rewards amount is the number of rewards that can be claimed per liquidity
	/// token. Here is tracked the number of rewards per liquidity token relationship.
	/// Expect larger values when the number of liquidity tokens are smaller.
	pub type PromotedPoolRewards<T: Config> =
		StorageValue<_, BTreeMap<TokenId, PromotedPools>, ValueQuery>;

	#[derive(Encode, Decode, Clone, Default, RuntimeDebug, PartialEq, Eq, TypeInfo)]
	/// Information about single token rewards
	pub struct PromotedPools {
		// Weight of the pool, each of the activated tokens has its weight assignedd
		// Liquidityt Mining Rewards are distributed based on that weight
		pub weight: u8,
		/// **Cumulative** number of rewards amount that can be claimed for single
		/// activted liquidity token
		pub rewards: U256,
	}

	#[pallet::storage]
	#[pallet::getter(fn total_activated_amount)]
	pub type TotalActivatedLiquidity<T: Config> =
		StorageMap<_, Twox64Concat, TokenId, u128, ValueQuery>;

	// //////////////////////////////////////////////////////////////////////////////////////////////
	// 3rd Party Rewards
	// //////////////////////////////////////////////////////////////////////////////////////////////

	/// Stores information about pool weight and accumulated rewards
	#[pallet::storage]
	pub type RewardsInfoForScheduleRewards<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		AccountIdOf<T>,
		Twox64Concat,
		(TokenId, TokenId),
		RewardInfo,
		ValueQuery,
	>;

	/// How much scheduled rewards per single liquidty_token should be distribute_rewards
	/// the **value is multiplied by u128::MAX** to avoid floating point arithmetic
	#[pallet::storage]
	pub type ScheduleRewardsPerSingleLiquidity<T: Config> =
		StorageValue<_, BTreeMap<(TokenId, TokenId), u128>, ValueQuery>;

	/// How much scheduled rewards per single liquidty_token should be distribute_rewards
	/// the **value is multiplied by u128::MAX** to avoid floating point arithmetic
	#[pallet::storage]
	pub type ScheduleRewardsTotal<T: Config> =
		StorageMap<_, Twox64Concat, (TokenId, TokenId), (u128, u64, u128), ValueQuery>;

	#[pallet::storage]
	pub type ScheduleRewardsPerLiquidity<T: Config> =
		StorageMap<_, Twox64Concat, (TokenId, TokenId), (U256, u64), ValueQuery>;

	/// How much scheduled rewards per single liquidty_token should be distribute_rewards
	/// the **value is multiplied by u128::MAX** to avoid floating point arithmetic
	// #[pallet::storage]
	// pub type ScheduleRewardsCumulative<T: Config> = StorageMap<_, Twox64Concat ,(TokenId, TokenId, u64), u128, ValueQuery>;

	/// List of activated schedules sorted by expiry date
	#[pallet::storage]
	#[pallet::getter(fn schedules)]
	pub type RewardsSchedules<T: Config> = StorageValue<
		_,
		BoundedBTreeMap<
			(T::BlockNumber, TokenId, TokenId, Balance, u64),
			(),
			T::RewardsSchedulesLimit,
		>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn pos)]
	pub type ScheduleListPos<T: Config> = StorageValue<_, ScheduleId, OptionQuery>;

	#[pallet::storage]
	pub type ScheduleListPosPrev<T: Config> = StorageValue<_, ScheduleId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn head)]
	pub type ScheduleListHead<T: Config> = StorageValue<_, ScheduleId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn tail)]
	pub type ScheduleListTail<T: Config> = StorageValue<_, ScheduleId, OptionQuery>;

	#[pallet::storage]
	pub type RewardsSchedulesList<T: Config> =
		StorageMap<_, Twox64Concat, ScheduleId, (Schedule, Option<ScheduleId>), OptionQuery>;

	/// Maps liquidity token to list of tokens that it ever was rewarded with
	#[pallet::storage]
	pub type RewardTokensPerPool<T: Config> =
		StorageDoubleMap<_, Twox64Concat, TokenId, Twox64Concat, TokenId, (), ValueQuery>;

	/// Total amount of activated liquidity for each schedule
	#[pallet::storage]
	pub type TotalActivatedLiquidityForSchedules<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		TokenId,
		Twox64Concat,
		TokenId,
		(u128, u128, u64, u128),
		ValueQuery,
	>;

	// #[pallet::storage]
	// pub type PrevTotalActivatedLiquidityForSchedules<T: Config> =
	// 	StorageDoubleMap<_, Twox64Concat, TokenId, Twox64Concat, TokenId, u128, ValueQuery>;

	/// Tracks how much liquidity user activated for particular (liq token, reward token) pair
	/// StorageNMap was used because it only require single read to know if user deactivated all
	/// liquidity associated with particular liquidity_token that is rewarded. If so part of the
	/// liquididty tokens can be unlocked.
	#[pallet::storage]
	pub type ActivatedLiquidityForSchedules<T> = StorageNMap<
		_,
		(
			NMapKey<Twox64Concat, AccountIdOf<T>>,
			NMapKey<Twox64Concat, TokenId>,
			NMapKey<Twox64Concat, TokenId>,
		),
		u128,
		OptionQuery,
	>;

	/// Tracks how much of the liquidity was activated for schedule rewards and not yet
	/// liquidity mining rewards. That information is essential to properly handle token unlcocks
	/// when liquidity is deactivated.
	#[pallet::storage]
	pub type ActivatedLockedLiquidityForSchedules<T: Config> =
		StorageDoubleMap<_, Twox64Concat, AccountIdOf<T>, Twox64Concat, TokenId, u128, ValueQuery>;

	/// Tracks how much of the liquidity was activated for schedule rewards and not yet
	/// liquidity mining rewards. That information is essential to properly handle token unlcocks
	/// when liquidity is deactivated.
	#[pallet::storage]
	pub type ActivatedNativeRewardsLiq<T: Config> =
		StorageDoubleMap<_, Twox64Concat, AccountIdOf<T>, Twox64Concat, TokenId, u128, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Claims liquidity mining rewards
		#[transactional]
		#[pallet::call_index(0)]
		#[pallet::weight(<<T as Config>::WeightInfo>::claim_native_rewards())]
		#[deprecated(note = "claim_native_rewards should be used instead")]
		pub fn claim_rewards_all(
			origin: OriginFor<T>,
			liquidity_token_id: TokenId,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			<Self as ProofOfStakeRewardsApi<T::AccountId>>::claim_rewards_all(
				sender,
				liquidity_token_id,
			)?;

			Ok(())
		}

		/// Enables/disables pool for liquidity mining rewards
		#[pallet::call_index(1)]
		#[pallet::weight(<<T as Config>::WeightInfo>::update_pool_promotion())]
		pub fn update_pool_promotion(
			origin: OriginFor<T>,
			liquidity_token_id: TokenId,
			liquidity_mining_issuance_weight: u8,
		) -> DispatchResult {
			ensure_root(origin)?;

			if liquidity_mining_issuance_weight > 0 {
				<Self as ProofOfStakeRewardsApi<T::AccountId>>::enable(
					liquidity_token_id,
					liquidity_mining_issuance_weight,
				);
			} else {
				<Self as ProofOfStakeRewardsApi<T::AccountId>>::disable(liquidity_token_id);
			}
			Ok(())
		}

		/// Increases number of tokens used for liquidity mining purposes.
		///
		/// Parameters:
		/// - liquidity_token_id - id of the token
		/// - amount - amount of the token
		/// - use_balance_from - where from tokens should be used
		#[transactional]
		#[pallet::call_index(2)]
		#[pallet::weight(<<T as Config>::WeightInfo>::activate_liquidity_for_native_rewards())]
		#[deprecated(note = "activate_liquidity_for_native_rewards should be used instead")]
		pub fn activate_liquidity(
			origin: OriginFor<T>,
			liquidity_token_id: TokenId,
			amount: Balance,
			use_balance_from: Option<ActivateKind>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::activate_liquidity_for_native_rewards_impl(
				sender,
				liquidity_token_id,
				amount,
				use_balance_from,
			)
		}

		/// Decreases number of tokens used for liquidity mining purposes
		#[transactional]
		#[pallet::call_index(3)]
		#[pallet::weight(<<T as Config>::WeightInfo>::deactivate_liquidity_for_native_rewards())]
		#[deprecated(note = "deactivate_liquidity_for_native_rewards should be used instead")]
		pub fn deactivate_liquidity(
			origin: OriginFor<T>,
			liquidity_token_id: TokenId,
			amount: Balance,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::deactivate_liquidity_for_native_rewards_impl(sender, liquidity_token_id, amount)
		}

		/// Schedules rewards for selected liquidity token
		/// - tokens - pair of tokens
		/// - amount - amount of the token
		/// - schedule_end - id of the last rewarded seession. Rewards will be distributedd equally between sessions in range (now ..
		/// schedule_end). Distribution starts from the *next* session till `schedule_end`.
		// TODO: delays schedule by 1 session
		#[transactional]
		#[pallet::call_index(4)]
		#[pallet::weight(<<T as Config>::WeightInfo>::reward_pool())]
		pub fn reward_pool(
			origin: OriginFor<T>,
			pool: (TokenId, TokenId),
			token_id: TokenId,
			amount: Balance,
			schedule_end: T::BlockNumber,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			Self::reward_pool_impl(sender, pool, token_id, amount, schedule_end)
		}

		/// Increases number of tokens used for liquidity mining purposes.
		///
		/// Parameters:
		/// - liquidity_token_id - id of the token
		/// - amount - amount of the token
		/// - use_balance_from - where from tokens should be used. If set to `None` then tokens will
		/// be taken from available balance
		#[transactional]
		#[pallet::call_index(5)]
		#[pallet::weight(<<T as Config>::WeightInfo>::activate_liquidity_for_3rdparty_rewards())]
		pub fn activate_liquidity_for_3rdparty_rewards(
			origin: OriginFor<T>,
			liquidity_token_id: TokenId,
			amount: Balance,
			reward_token: TokenId,
			use_balance_from: Option<ThirdPartyActivationKind>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			Self::activate_liquidity_for_3rdparty_rewards_impl(
				sender,
				liquidity_token_id,
				amount,
				use_balance_from.unwrap_or(ThirdPartyActivationKind::ActivateKind(None)),
				reward_token,
			)
			.map_err(|err| DispatchErrorWithPostInfo {
				post_info: PostDispatchInfo {
					actual_weight: Some(
						<<T as Config>::WeightInfo>::activate_liquidity_for_3rdparty_rewards(),
					),
					pays_fee: Pays::Yes,
				},
				error: err,
			})?;
			Ok(Pays::No.into())
		}

		/// Decreases number of tokens used for liquidity mining purposes.
		///
		/// Parameters:
		/// - liquidity_token_id - id of the token
		/// - amount - amount of the token
		/// - use_balance_from - where from tokens should be used
		#[transactional]
		#[pallet::call_index(6)]
		#[pallet::weight(<<T as Config>::WeightInfo>::deactivate_liquidity_for_3rdparty_rewards())]
		pub fn deactivate_liquidity_for_3rdparty_rewards(
			origin: OriginFor<T>,
			liquidity_token_id: TokenId,
			amount: Balance,
			reward_token: TokenId,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			Self::deactivate_liquidity_for_3rdparty_rewards_impl(
				sender,
				liquidity_token_id,
				amount,
				reward_token,
			)
			.map_err(|err| DispatchErrorWithPostInfo {
				post_info: PostDispatchInfo {
					actual_weight: Some(
						<<T as Config>::WeightInfo>::activate_liquidity_for_3rdparty_rewards(),
					),
					pays_fee: Pays::Yes,
				},
				error: err,
			})?;
			Ok(Pays::No.into())
		}

		/// Claims liquidity mining rewards
		/// - tokens - pair of tokens
		/// - amount - amount of the token
		/// - reward_token - id of the token that is rewarded
		#[transactional]
		#[pallet::call_index(7)]
		#[pallet::weight(<<T as Config>::WeightInfo>::claim_3rdparty_rewards())]
		pub fn claim_3rdparty_rewards(
			origin: OriginFor<T>,
			liquidity_token_id: TokenId,
			reward_token: TokenId,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::update_cumulative_rewards(liquidity_token_id, reward_token);
			Self::claim_schedule_rewards_all_impl(sender, liquidity_token_id, reward_token)?;
			Ok(())
		}

		/// Increases number of tokens used for liquidity mining purposes.
		///
		/// Parameters:
		/// - liquidity_token_id - id of the token
		/// - amount - amount of the token
		/// - use_balance_from - where from tokens should be used
		#[transactional]
		#[pallet::call_index(8)]
		#[pallet::weight(<<T as Config>::WeightInfo>::activate_liquidity_for_native_rewards())]
		pub fn activate_liquidity_for_native_rewards(
			origin: OriginFor<T>,
			liquidity_token_id: TokenId,
			amount: Balance,
			use_balance_from: Option<ActivateKind>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::activate_liquidity_for_native_rewards_impl(
				sender,
				liquidity_token_id,
				amount,
				use_balance_from,
			)
		}

		/// Decreases number of tokens used for liquidity mining purposes
		#[transactional]
		#[pallet::call_index(9)]
		#[pallet::weight(<<T as Config>::WeightInfo>::deactivate_liquidity_for_native_rewards())]
		pub fn deactivate_liquidity_for_native_rewards(
			origin: OriginFor<T>,
			liquidity_token_id: TokenId,
			amount: Balance,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::deactivate_liquidity_for_native_rewards_impl(sender, liquidity_token_id, amount)
		}

		#[transactional]
		#[pallet::call_index(10)]
		#[pallet::weight(<<T as Config>::WeightInfo>::claim_native_rewards())]
		#[deprecated(note = "claim_native_rewards should be used instead")]
		pub fn claim_native_rewards(
			origin: OriginFor<T>,
			liquidity_token_id: TokenId,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			<Self as ProofOfStakeRewardsApi<T::AccountId>>::claim_rewards_all(
				sender,
				liquidity_token_id,
			)?;

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn update_cumulative_rewards(liquidity_asset_id: TokenId, liquidity_assets_reward: TokenId) {
		let (cumulative, idx) =
			ScheduleRewardsPerLiquidity::<T>::get((liquidity_asset_id, liquidity_assets_reward));
		if idx == (Self::session_index() as u64) {
		} else {
			let total_activated_liquidity =
				Self::total_activated_liquidity(liquidity_asset_id, liquidity_assets_reward);
			let total_schedule_rewards =
				Self::total_schedule_rewards(liquidity_asset_id, liquidity_assets_reward);
			if total_activated_liquidity > 0 {
				ScheduleRewardsTotal::<T>::mutate(
					(liquidity_asset_id, liquidity_assets_reward),
					|(cumulative, _, _)| {
						*cumulative = 0;
					},
				);
				let pending = (U256::from(total_schedule_rewards) * U256::from(u128::MAX))
					.checked_div(U256::from(total_activated_liquidity))
					.unwrap_or_default();
				ScheduleRewardsPerLiquidity::<T>::insert(
					(liquidity_asset_id, liquidity_assets_reward),
					(cumulative + pending, (Self::session_index() as u64)),
				);
			}
		}
	}

	fn total_rewards_for_liquidity(
		liquidity_asset_id: TokenId,
		liquidity_assets_reward: TokenId,
	) -> U256 {
		let (cumulative, idx) =
			ScheduleRewardsPerLiquidity::<T>::get((liquidity_asset_id, liquidity_assets_reward));
		if idx == (Self::session_index() as u64) {
			cumulative
		} else {
			let total_activated_liquidity =
				Self::total_activated_liquidity(liquidity_asset_id, liquidity_assets_reward);
			let total_schedule_rewards =
				Self::total_schedule_rewards(liquidity_asset_id, liquidity_assets_reward);
			let pending = (U256::from(total_schedule_rewards) * U256::from(u128::MAX))
				.checked_div(U256::from(total_activated_liquidity))
				.unwrap_or_default();
			cumulative + pending
		}
	}

	fn total_activated_liquidity(
		liquidity_asset_id: TokenId,
		liquidity_assets_reward: TokenId,
	) -> Balance {
		let (pending_negative, pending_positive, idx, cumulative) =
			TotalActivatedLiquidityForSchedules::<T>::get(
				liquidity_asset_id,
				liquidity_assets_reward,
			);
		if idx == (Self::session_index() as u64) {
			cumulative
		} else {
			cumulative + pending_positive - pending_negative
		}
	}

	fn total_schedule_rewards(
		liquidity_asset_id: TokenId,
		liquidity_assets_reward: TokenId,
	) -> Balance {
		let (pending, idx, cumulative) =
			ScheduleRewardsTotal::<T>::get((liquidity_asset_id, liquidity_assets_reward));
		if idx == (Self::session_index() as u64) {
			cumulative
		} else {
			cumulative + pending
		}
	}

	fn update_total_activated_liqudity(
		liquidity_asset_id: TokenId,
		liquidity_assets_reward: TokenId,
		diff: Balance,
		change: bool,
	) {
		// TODO: make configurable
		let session_id = Self::session_index() as u64;

		TotalActivatedLiquidityForSchedules::<T>::mutate(
			liquidity_asset_id,
			liquidity_assets_reward,
			|(pending_negative, pending_positive, idx, cumulative)| {
				if *idx == session_id {
					if change {
						*pending_positive += diff;
					} else {
						*pending_negative += diff;
					};
				} else {
					// NOTE: handle burn so negative diff
					*cumulative = *cumulative + *pending_positive - *pending_negative;
					if change {
						*pending_positive = diff;
						*pending_negative = 0u128;
					} else {
						*pending_positive = 0u128;
						*pending_negative = diff;
					};
					*idx = session_id;
				}
			},
		);
	}

	fn activate_liquidity_for_native_rewards_impl(
		user: AccountIdOf<T>,
		liquidity_asset_id: TokenId,
		amount: Balance,
		use_balance_from: Option<ActivateKind>,
	) -> DispatchResult {
		Self::ensure_is_promoted_pool(liquidity_asset_id)?;

		ensure!(
			<T as Config>::ActivationReservesProvider::can_activate(
				liquidity_asset_id,
				&user,
				amount,
				use_balance_from.clone()
			),
			Error::<T>::NotEnoughAssets
		);

		Self::set_liquidity_minting_checkpoint(user.clone(), liquidity_asset_id, amount)?;

		<T as Config>::ActivationReservesProvider::activate(
			liquidity_asset_id,
			&user,
			amount,
			use_balance_from,
		)?;
		Pallet::<T>::deposit_event(Event::LiquidityActivated(user, liquidity_asset_id, amount));

		Ok(())
	}

	pub fn calculate_native_rewards_amount(
		user: AccountIdOf<T>,
		liquidity_asset_id: TokenId,
	) -> Result<Balance, DispatchError> {
		Self::ensure_is_promoted_pool(liquidity_asset_id)?;
		let rewards_info = RewardsInfo::<T>::try_get(user.clone(), liquidity_asset_id)
			.or(Err(DispatchError::from(Error::<T>::MissingRewardsInfoError)))?;

		let current_rewards = match rewards_info.activated_amount {
			0 => 0u128,
			_ => {
				let calc =
					RewardsCalculator::mining_rewards::<T>(user.clone(), liquidity_asset_id)?;
				calc.calculate_rewards().map_err(|err| Into::<Error<T>>::into(err))?
			},
		};

		Ok(current_rewards
			.checked_add(rewards_info.rewards_not_yet_claimed)
			.and_then(|v| v.checked_sub(rewards_info.rewards_already_claimed))
			.ok_or(Error::<T>::CalculateRewardsMathError)?)
	}

	fn deactivate_liquidity_for_native_rewards_impl(
		user: AccountIdOf<T>,
		liquidity_asset_id: TokenId,
		amount: Balance,
	) -> DispatchResult {
		ensure!(
			ActivatedNativeRewardsLiq::<T>::get(user.clone(), liquidity_asset_id) == 0,
			Error::<T>::LiquidityLockedIn3rdpartyRewards
		);

		if amount > 0 {
			Self::set_liquidity_burning_checkpoint(user.clone(), liquidity_asset_id, amount)?;
			Pallet::<T>::deposit_event(Event::LiquidityDeactivated(
				user,
				liquidity_asset_id,
				amount,
			));
		}
		Ok(())
	}

	fn activate_liquidity_for_3rdparty_rewards_impl(
		user: AccountIdOf<T>,
		liquidity_asset_id: TokenId,
		amount: Balance,
		use_balance_from: ThirdPartyActivationKind,
		reward_token: TokenId,
	) -> DispatchResult {
		Self::ensure_is_promoted_pool(liquidity_asset_id)?;

		match use_balance_from {
			// 1R 1W
			ThirdPartyActivationKind::ActivateKind(ref use_balance_from) => {
				ensure!(
					<T as Config>::ActivationReservesProvider::can_activate(
						liquidity_asset_id,
						&user,
						amount,
						use_balance_from.clone(),
					),
					Error::<T>::NotEnoughAssets
				);
				ActivatedLockedLiquidityForSchedules::<T>::mutate(
					user.clone(),
					liquidity_asset_id,
					|val| *val += amount,
				);
			},
			// 2R
			ThirdPartyActivationKind::ActivatedLiquidity(token_id) => {
				let already_activated_amount = RewardsInfoForScheduleRewards::<T>::get(
					user.clone(),
					(liquidity_asset_id, reward_token),
				)
				.activated_amount;
				let available_amount = RewardsInfoForScheduleRewards::<T>::get(
					user.clone(),
					(liquidity_asset_id, token_id),
				)
				.activated_amount;
				ensure!(
					already_activated_amount + amount <= available_amount,
					Error::<T>::NotEnoughAssets
				);
			},
			ThirdPartyActivationKind::LiquidityMining => {
				let already_activated_amount = RewardsInfoForScheduleRewards::<T>::get(
					user.clone(),
					(liquidity_asset_id, reward_token),
				)
				.activated_amount;
				let available_amount =
					RewardsInfo::<T>::get(user.clone(), liquidity_asset_id).activated_amount;
				ensure!(
					already_activated_amount + amount <= available_amount,
					Error::<T>::NotEnoughAssets
				);
				ActivatedNativeRewardsLiq::<T>::mutate(user.clone(), liquidity_asset_id, |val| {
					*val += amount
				});
			},
		}

		Self::set_liquidity_minting_checkpoint_3rdparty(
			user.clone(),
			liquidity_asset_id,
			amount,
			reward_token,
		)?;

		match use_balance_from {
			ThirdPartyActivationKind::ActivateKind(use_balance_from) => {
				<T as Config>::ActivationReservesProvider::activate(
					liquidity_asset_id,
					&user,
					amount,
					use_balance_from,
				)?;
			},
			_ => {},
		}

		Pallet::<T>::deposit_event(Event::LiquidityActivated(user, liquidity_asset_id, amount));

		Ok(())
	}

	fn deactivate_liquidity_for_3rdparty_rewards_impl(
		user: AccountIdOf<T>,
		liquidity_asset_id: TokenId,
		amount: Balance,
		rewards_asset_id: TokenId,
	) -> DispatchResult {
		if amount > 0 {
			Self::set_liquidity_burning_checkpoint_for_schedule(
				user.clone(),
				liquidity_asset_id,
				amount,
				rewards_asset_id,
			)?;
			Pallet::<T>::deposit_event(Event::LiquidityDeactivated(
				user,
				liquidity_asset_id,
				amount,
			));
		}
		Ok(())
	}

	pub fn calculate_3rdparty_rewards_all(
		user: AccountIdOf<T>,
	) -> Result<Vec<(TokenId, TokenId, Balance)>, DispatchError> {
		let mut result = RewardsInfoForScheduleRewards::<T>::iter_prefix(user.clone())
			.map(|((liq_token, reward_token), _)| {
				Self::calculate_3rdparty_rewards_amount(user.clone(), liq_token, reward_token)
					.map(|amount| (liq_token, reward_token, amount))
			})
			.collect::<Result<Vec<_>, _>>();
		result.as_mut().map(|v| v.sort());
		result
	}

	pub fn calculate_3rdparty_rewards_amount(
		user: AccountIdOf<T>,
		liquidity_asset_id: TokenId,
		rewards_asset_id: TokenId,
	) -> Result<Balance, DispatchError> {
		Self::ensure_is_promoted_pool(liquidity_asset_id)?;

		if let Ok(info) = RewardsInfoForScheduleRewards::<T>::try_get(
			user.clone(),
			(liquidity_asset_id, rewards_asset_id),
		) {
			let current_rewards = match info.activated_amount {
				0 => 0u128,
				_ => {
					let calc = RewardsCalculator::schedule_rewards::<T>(
						user.clone(),
						liquidity_asset_id,
						rewards_asset_id,
					)?;
					calc.calculate_rewards().map_err(|err| Into::<Error<T>>::into(err))?
				},
			};

			Ok(current_rewards
				.checked_add(info.rewards_not_yet_claimed)
				.and_then(|v| v.checked_sub(info.rewards_already_claimed))
				.ok_or(Error::<T>::CalculateRewardsMathError)?)
		} else {
			Ok(0u128)
		}
	}

	fn pallet_account() -> T::AccountId {
		PALLET_ID.into_account_truncating()
	}

	pub fn session_index() -> u32 {
		Self::get_current_rewards_time().unwrap_or_default()
	}

	pub fn rewards_period() -> u32 {
		T::RewardsDistributionPeriod::get()
	}

	pub fn is_new_session() -> bool {
		let block_nr = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
		(block_nr + 1) % Self::rewards_period() == 0u32
	}

	fn native_token_id() -> TokenId {
		<T as Config>::NativeCurrencyId::get()
	}

	fn get_pool_rewards(liquidity_asset_id: TokenId) -> Result<U256, sp_runtime::DispatchError> {
		Ok(PromotedPoolRewards::<T>::get()
			.get(&liquidity_asset_id)
			.map(|v| v.rewards)
			.ok_or(Error::<T>::NotAPromotedPool)?)
	}

	fn get_current_rewards_time() -> Result<u32, sp_runtime::DispatchError> {
		<frame_system::Pallet<T>>::block_number()
			.saturated_into::<u32>()
			.checked_add(1)
			.and_then(|v| v.checked_div(T::RewardsDistributionPeriod::get()))
			.ok_or(DispatchError::from(Error::<T>::CalculateRewardsMathError))
	}

	fn ensure_is_promoted_pool(liquidity_asset_id: TokenId) -> Result<(), DispatchError> {
		//NOTE: 2 separate functions for separate rewards
		if Self::get_pool_rewards(liquidity_asset_id).is_ok() ||
			RewardTokensPerPool::<T>::iter_prefix_values(liquidity_asset_id)
				.next()
				.is_some()
		{
			Ok(())
		} else {
			Err(DispatchError::from(Error::<T>::NotAPromotedPool))
		}
	}

	fn set_liquidity_minting_checkpoint(
		user: AccountIdOf<T>,
		liquidity_asset_id: TokenId,
		liquidity_assets_added: Balance,
	) -> DispatchResult {
		Self::ensure_is_promoted_pool(liquidity_asset_id)?;

		{
			let calc = RewardsCalculator::mining_rewards::<T>(user.clone(), liquidity_asset_id)?;
			let rewards_info = calc
				.activate_more(liquidity_assets_added)
				.map_err(|err| Into::<Error<T>>::into(err))?;

			RewardsInfo::<T>::insert(user.clone(), liquidity_asset_id, rewards_info);
		}

		TotalActivatedLiquidity::<T>::try_mutate(liquidity_asset_id, |active_amount| {
			if let Some(val) = active_amount.checked_add(liquidity_assets_added) {
				*active_amount = val;
				Ok(())
			} else {
				Err(())
			}
		})
		.map_err(|_| DispatchError::from(Error::<T>::LiquidityCheckpointMathError))?;

		Ok(())
	}

	fn set_liquidity_minting_checkpoint_3rdparty(
		user: AccountIdOf<T>,
		liquidity_asset_id: TokenId,
		liquidity_assets_added: Balance,
		liquidity_assets_reward: TokenId,
	) -> DispatchResult {
		Self::ensure_is_promoted_pool(liquidity_asset_id)?;

		Self::update_cumulative_rewards(liquidity_asset_id, liquidity_assets_reward);
		{
			let calc = RewardsCalculator::schedule_rewards::<T>(
				user.clone(),
				liquidity_asset_id,
				liquidity_assets_reward,
			)?;
			let rewards_info = calc
				.activate_more(liquidity_assets_added)
				.map_err(|err| Into::<Error<T>>::into(err))?;
			RewardsInfoForScheduleRewards::<T>::insert(
				user.clone(),
				(liquidity_asset_id, liquidity_assets_reward),
				rewards_info,
			);
		}

		ActivatedLiquidityForSchedules::<T>::try_mutate_exists(
			(user.clone(), liquidity_asset_id, liquidity_assets_reward),
			|v| {
				match v {
					Some(x) => {
						v.as_mut().map(|a| *a += liquidity_assets_added);
					},
					None => {
						*v = Some(liquidity_assets_added);
					},
				};
				Ok::<(), Error<T>>(())
			},
		)?;

		Self::update_total_activated_liqudity(
			liquidity_asset_id,
			liquidity_assets_reward,
			liquidity_assets_added,
			true,
		);

		Ok(())
	}

	fn set_liquidity_burning_checkpoint(
		user: AccountIdOf<T>,
		liquidity_asset_id: TokenId,
		liquidity_assets_burned: Balance,
	) -> DispatchResult {
		Self::ensure_is_promoted_pool(liquidity_asset_id)?;
		let current_time: u32 = Self::get_current_rewards_time()?;
		let pool_ratio_current = Self::get_pool_rewards(liquidity_asset_id)?;

		let mut rewards_info = RewardsInfo::<T>::try_get(user.clone(), liquidity_asset_id)
			.or(Err(DispatchError::from(Error::<T>::MissingRewardsInfoError)))?;
		ensure!(
			rewards_info.activated_amount >= liquidity_assets_burned,
			Error::<T>::NotEnoughAssets
		);

		let calc = RewardsCalculator::mining_rewards::<T>(user.clone(), liquidity_asset_id)?;
		let rewards_info = calc
			.activate_less(liquidity_assets_burned)
			.map_err(|err| Into::<Error<T>>::into(err))?;

		RewardsInfo::<T>::insert(user.clone(), liquidity_asset_id, rewards_info);

		TotalActivatedLiquidity::<T>::try_mutate(liquidity_asset_id, |active_amount| {
			if let Some(val) = active_amount.checked_sub(liquidity_assets_burned) {
				*active_amount = val;
				Ok(())
			} else {
				Err(())
			}
		})
		.map_err(|_| DispatchError::from(Error::<T>::LiquidityCheckpointMathError))?;

		<T as Config>::ActivationReservesProvider::deactivate(
			liquidity_asset_id,
			&user,
			liquidity_assets_burned,
		);

		Ok(())
	}

	fn set_liquidity_burning_checkpoint_for_schedule(
		user: AccountIdOf<T>,
		liquidity_asset_id: TokenId,
		liquidity_assets_burned: Balance,
		reward_token: TokenId,
	) -> DispatchResult {
		Self::ensure_is_promoted_pool(liquidity_asset_id)?;
		Self::update_cumulative_rewards(liquidity_asset_id, reward_token);

		let calc = RewardsCalculator::schedule_rewards::<T>(
			user.clone(),
			liquidity_asset_id,
			reward_token,
		)?;

		let rewards_info = calc
			.activate_less(liquidity_assets_burned)
			.map_err(|err| Into::<Error<T>>::into(err))?;

		RewardsInfoForScheduleRewards::<T>::insert(
			user.clone(),
			(liquidity_asset_id, reward_token),
			rewards_info,
		);

		Self::update_total_activated_liqudity(
			liquidity_asset_id,
			reward_token,
			liquidity_assets_burned,
			false,
		);

		ActivatedLiquidityForSchedules::<T>::try_mutate_exists(
			(user.clone(), liquidity_asset_id, reward_token),
			|v| {
				v.and_then(|a| {
					a.checked_sub(liquidity_assets_burned).and_then(|val| {
						if val > 0 {
							*v = Some(val);
						} else {
							*v = None;
						}
						Some(val)
					})
				})
				.ok_or(Error::<T>::MathOverflow)
			},
		)?;

		if let None = ActivatedLiquidityForSchedules::<T>::iter_prefix_values((
			user.clone(),
			liquidity_asset_id,
		))
		.next()
		{
			let amount = ActivatedLockedLiquidityForSchedules::<T>::mutate(
				user.clone(),
				liquidity_asset_id,
				|val| {
					let prev = *val;
					*val = 0;
					prev
				},
			);

			<T as Config>::ActivationReservesProvider::deactivate(
				liquidity_asset_id,
				&user,
				amount,
			);

			let amount =
				ActivatedNativeRewardsLiq::<T>::mutate(user.clone(), liquidity_asset_id, |val| {
					let prev = *val;
					*val = 0;
					prev
				});
		}

		Ok(())
	}

	fn claim_schedule_rewards_all_impl(
		user: T::AccountId,
		liquidity_asset_id: TokenId,
		reward_token: TokenId,
	) -> Result<Balance, DispatchError> {
		Self::ensure_is_promoted_pool(liquidity_asset_id)?;

		let calc = RewardsCalculator::schedule_rewards::<T>(
			user.clone(),
			liquidity_asset_id,
			reward_token,
		)?;
		let (rewards_info, total_available_rewards) =
			calc.claim_rewards().map_err(|err| Into::<Error<T>>::into(err))?;

		<T as Config>::Currency::transfer(
			reward_token.into(),
			&Self::pallet_account(),
			&user,
			total_available_rewards.into(),
			ExistenceRequirement::KeepAlive,
		)?;

		RewardsInfoForScheduleRewards::<T>::insert(
			user.clone(),
			(liquidity_asset_id, reward_token),
			rewards_info,
		);

		Pallet::<T>::deposit_event(Event::RewardsClaimed(
			user,
			liquidity_asset_id,
			total_available_rewards,
		));

		Ok(total_available_rewards)
	}

	pub(crate) fn reward_pool_impl(
		sender: T::AccountId,
		pool: (TokenId, TokenId),
		token_id: TokenId,
		amount: Balance,
		schedule_end: T::BlockNumber,
	) -> DispatchResult {
		let liquidity_token_id = <T as Config>::ValuationApi::get_liquidity_asset(pool.0, pool.1)
			.map_err(|_| Error::<T>::PoolDoesNotExist)?;

		let current_session = Self::session_index();
		ensure!(
			schedule_end.saturated_into::<u32>() > current_session,
			Error::<T>::CannotScheduleRewardsInPast
		);

		let amount_per_session = schedule_end
			.saturated_into::<u32>()
			.checked_sub(current_session)
			.and_then(|v| amount.checked_div(v.into()))
			.ok_or(Error::<T>::MathOverflow)?;

		ensure!(
			<T as Config>::ValuationApi::valuate_liquidity_token(token_id, amount_per_session) >=
				T::Min3rdPartyRewards::get() ||
				((token_id == Into::<u32>::into(Self::native_token_id())) &&
					amount_per_session >= T::Min3rdPartyRewards::get()) ||
				<T as Config>::ValuationApi::valuate_non_liquidity_token(
					token_id,
					amount_per_session
				) >= T::Min3rdPartyRewards::get(),
			Error::<T>::TooLittleRewards
		);

		RewardTokensPerPool::<T>::insert(liquidity_token_id, token_id, ());

		T::Currency::transfer(
			token_id.into(),
			&sender,
			&Self::pallet_account(),
			amount.into(),
			ExistenceRequirement::KeepAlive,
		)?;

		let current_session = Self::session_index();

		let head = ScheduleListHead::<T>::get();
		let tail = ScheduleListTail::<T>::get();
		let schedule = Schedule {
			scheduled_at: Self::session_index() as u64,
			last_session: schedule_end.saturated_into::<u64>(),
			liq_token: liquidity_token_id,
			reward_token: token_id,
			amount_per_session,
		};

		match (head, tail) {
			(None, None) => {
				// first schedule
				RewardsSchedulesList::<T>::insert(0, (schedule, None::<ScheduleId>));
				ScheduleListHead::<T>::put(0);
				ScheduleListTail::<T>::put(0);
			},
			(Some(_head), Some(tail)) => {
				RewardsSchedulesList::<T>::mutate(tail, |info| {
					if let Some((_schedule, next)) = info.as_mut() {
						*next = Some(tail + 1u64)
					}
				});
				RewardsSchedulesList::<T>::insert(tail + 1, (schedule, None::<ScheduleId>));
				ScheduleListTail::<T>::put(tail + 1);
			},
			_ => {}, // invariant assures this will never happen
		}

		Ok(())
	}
}

impl<T: Config> ProofOfStakeRewardsApi<T::AccountId> for Pallet<T> {
	type Balance = Balance;

	type CurrencyId = TokenId;

	#[cfg(feature = "runtime-benchmarks")]
	fn enable_3rdparty_rewards(
		account: T::AccountId,
		pool: (Self::CurrencyId, Self::CurrencyId),
		reward_token_id: Self::CurrencyId,
		last_session: u32,
		amount: Self::Balance,
	) {
		let liquidity_token_id =
			<T as Config>::ValuationApi::get_liquidity_asset(pool.0, pool.1).expect("pool exist");
		Pallet::<T>::reward_pool_impl(
			account.clone(),
			pool,
			reward_token_id,
			amount,
			last_session.into(),
		)
		.expect("call should pass");
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn activate_liquidity_for_3rdparty_rewards(
		account: T::AccountId,
		liquidity_token: Self::CurrencyId,
		amount: Self::Balance,
		reward_token_id: Self::CurrencyId,
	) {
		Pallet::<T>::activate_liquidity_for_3rdparty_rewards_impl(
			account,
			liquidity_token,
			amount,
			ThirdPartyActivationKind::ActivateKind(None),
			reward_token_id,
		)
		.expect("call should pass")
	}

	fn enable(liquidity_token_id: TokenId, weight: u8) {
		PromotedPoolRewards::<T>::mutate(|promoted_pools| {
			promoted_pools
				.entry(liquidity_token_id)
				.and_modify(|info| info.weight = weight)
				.or_insert(PromotedPools { weight, rewards: U256::zero() });
		});
		Pallet::<T>::deposit_event(Event::PoolPromotionUpdated(liquidity_token_id, Some(weight)));
	}

	fn disable(liquidity_token_id: TokenId) {
		PromotedPoolRewards::<T>::mutate(|promoted_pools| {
			if let Some(info) = promoted_pools.get_mut(&liquidity_token_id) {
				info.weight = 0;
			}
		});
		Pallet::<T>::deposit_event(Event::PoolPromotionUpdated(liquidity_token_id, None));
	}

	fn is_enabled(liquidity_token_id: TokenId) -> bool {
		PromotedPoolRewards::<T>::get().contains_key(&liquidity_token_id)
	}

	fn claim_rewards_all(
		user: T::AccountId,
		liquidity_asset_id: Self::CurrencyId,
	) -> Result<Self::Balance, DispatchError> {
		Self::ensure_is_promoted_pool(liquidity_asset_id)?;

		let calc = RewardsCalculator::mining_rewards::<T>(user.clone(), liquidity_asset_id)?;
		let (rewards_info, total_available_rewards) =
			calc.claim_rewards().map_err(|err| Into::<Error<T>>::into(err))?;

		<T as Config>::Currency::transfer(
			Self::native_token_id().into(),
			&<T as Config>::LiquidityMiningIssuanceVault::get(),
			&user,
			total_available_rewards.into(),
			ExistenceRequirement::KeepAlive,
		)?;

		RewardsInfo::<T>::insert(user.clone(), liquidity_asset_id, rewards_info);

		Pallet::<T>::deposit_event(Event::RewardsClaimed(
			user,
			liquidity_asset_id,
			total_available_rewards,
		));

		Ok(total_available_rewards)
	}

	fn activate_liquidity(
		user: T::AccountId,
		liquidity_asset_id: Self::CurrencyId,
		amount: Self::Balance,
		use_balance_from: Option<ActivateKind>,
	) -> DispatchResult {
		Self::activate_liquidity_for_native_rewards_impl(
			user,
			liquidity_asset_id,
			amount,
			use_balance_from,
		)
	}

	fn deactivate_liquidity(
		user: T::AccountId,
		liquidity_asset_id: Self::CurrencyId,
		amount: Self::Balance,
	) -> DispatchResult {
		Self::deactivate_liquidity_for_native_rewards_impl(user, liquidity_asset_id, amount)
	}

	fn calculate_rewards_amount(
		user: AccountIdOf<T>,
		liquidity_asset_id: TokenId,
	) -> Result<Balance, DispatchError> {
		Self::calculate_native_rewards_amount(user, liquidity_asset_id)
	}
}

impl<T: Config> LiquidityMiningApi for Pallet<T> {
	/// Distributs liquidity mining rewards between all the activated tokens based on their weight
	fn distribute_rewards(liquidity_mining_rewards: Balance) {
		let _ = PromotedPoolRewards::<T>::try_mutate(|promoted_pools| -> DispatchResult {
			// benchmark with max of X prom pools
			let activated_pools: Vec<_> = promoted_pools
				.clone()
				.into_iter()
				.filter_map(|(token_id, info)| {
					let activated_amount = Self::total_activated_amount(token_id);
					if activated_amount > 0 && info.weight > 0 {
						Some((token_id, info.weight, info.rewards, activated_amount))
					} else {
						None
					}
				})
				.collect();

			let maybe_total_weight = activated_pools.iter().try_fold(
				0u64,
				|acc, &(_token_id, weight, _rewards, _activated_amount)| {
					acc.checked_add(weight.into())
				},
			);

			for (token_id, weight, rewards, activated_amount) in activated_pools {
				let liquidity_mining_issuance_for_pool = match maybe_total_weight {
					Some(total_weight) if !total_weight.is_zero() =>
						Perbill::from_rational(weight.into(), total_weight)
							.mul_floor(liquidity_mining_rewards),
					_ => Balance::zero(),
				};

				let rewards_for_liquidity: U256 = U256::from(liquidity_mining_issuance_for_pool)
					.checked_mul(U256::from(u128::MAX))
					.and_then(|x| x.checked_div(activated_amount.into()))
					.and_then(|x| x.checked_add(rewards))
					.ok_or(Error::<T>::MathError)?;

				promoted_pools
					.entry(token_id.clone())
					.and_modify(|info| info.rewards = rewards_for_liquidity);
			}
			Ok(())
		});
	}
}
