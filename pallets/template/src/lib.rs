#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;

//use r;
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use frame_support::{
	debug,
	dispatch::Vec,
	pallet_prelude::*,
	sp_runtime::traits::*,
	traits::{Currency, ExistenceRequirement, Get},
	PalletId,
};
use frame_system::pallet_prelude::*;

use dusk_bls12_381::BlsScalar;
use dusk_bytes::Serializable;
use dusk_plonk::{
	circuit::verify,
	prelude::{PublicParameters, VerifierData},
	proof_system::Proof,
};

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Currency: Currency<Self::AccountId>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type MerkleTreeLevels: Get<u32>;

		#[pallet::constant]
		type RootHistorySize: Get<u32>;

		#[pallet::constant]
		type MaxPublicParameterLen: Get<u32>;

		#[pallet::constant]
		type MaxVerifierDataLen: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn commitments)]
	pub type CommitmentStorage<T: Config> = StorageMap<_, Twox64Concat, [u8; 32], (), ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn next_index)]
	pub type NextIndexStorage<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn current_root_index)]
	pub type CurrentRootIndexStorage<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn filled_subtrees)]
	pub type FilledSubtreesStorage<T: Config> =
		StorageMap<_, Twox64Concat, u32, [u8; 32], ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn roots)]
	pub type RootsStorage<T: Config> = StorageMap<_, Twox64Concat, u32, [u8; 32], ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn srs)]
	pub type PublicParametersStorage<T: Config> =
		StorageValue<_, BoundedVec<u8, T::MaxPublicParameterLen>>;

	#[pallet::storage]
	#[pallet::getter(fn verifier_data)]
	pub type VerifierDataStorage<T: Config> =
		StorageValue<_, BoundedVec<u8, T::MaxVerifierDataLen>>;

	#[pallet::genesis_config]
	pub struct GenesisConfig;

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			// Create Pallet account
			let account_id = <Pallet<T>>::account_id();
			let min = T::Currency::minimum_balance();
			if T::Currency::free_balance(&account_id) < min {
				let _ = T::Currency::make_free_balance_be(&account_id, min);
			}
			debug(&T::Currency::total_balance(&account_id));
			let mut tmp_hash = <Pallet<T>>::zeroes(0);
			FilledSubtreesStorage::<T>::insert(0, &tmp_hash);

			for i in 1..T::MerkleTreeLevels::get() {
				tmp_hash = dusk_poseidon::sponge::hash(&[
					BlsScalar::from_bytes(&tmp_hash).unwrap(),
					BlsScalar::from_bytes(&tmp_hash).unwrap(),
				])
				.to_bytes();
				FilledSubtreesStorage::<T>::insert(i, &tmp_hash);
			}
			RootsStorage::<T>::insert(0, &tmp_hash);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Deposit([u8; 32], u32),
		PaublicParameteresStored(u32, T::AccountId),
		VerifierDataStored(u32, T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		ConvertionVecToBVecFail,
		NoneValue,
		StorageOverflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn deposit(
			origin: OriginFor<T>,
			value: BalanceOf<T>,
			commitment: [u8; 32],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			assert!(!<CommitmentStorage<T>>::contains_key(&commitment));
			let index = Self::insert_into_tree(&commitment);

			<CommitmentStorage<T>>::insert(commitment.clone(), ());
			let account_id = Self::account_id();

			T::Currency::transfer(&who, &account_id, value, ExistenceRequirement::AllowDeath)?;
			debug(&T::Currency::total_balance(&account_id));
			Self::deposit_event(Event::Deposit(commitment, index));
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn store_parameters(origin: OriginFor<T>, parameters: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			debug(&parameters.len());
			assert_eq!(T::MaxPublicParameterLen::get(), parameters.len() as u32);
			match parameters.try_into() {
				Ok(bvec) => {
					let bvec: BoundedVec<u8, T::MaxPublicParameterLen> = bvec;
					let bounded_vec_size = bvec.len() as u32;
					<PublicParametersStorage<T>>::put(bvec);
					Self::deposit_event(Event::PaublicParameteresStored(bounded_vec_size, who));
					Ok(())
				},
				Err(_e) => Ok(()),
			}
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn store_vd(origin: OriginFor<T>, verifier_data: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let vd_size = verifier_data.len() as u32;
			let bvec: BoundedVec<u8, T::MaxVerifierDataLen> = verifier_data.try_into().unwrap();
			<VerifierDataStorage<T>>::put(bvec);
			Self::deposit_event(Event::VerifierDataStored(vd_size, who));
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn verify_proof(origin: OriginFor<T>, proof_bytes: Vec<u8>) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			assert_eq!(proof_bytes.len(), 1488usize);
			match (
				<PublicParametersStorage<T>>::get().as_ref(),
				<VerifierDataStorage<T>>::get().as_ref(),
			) {
				(Some(pp_bytes), Some(vd_bytes)) => {
					let pp = PublicParameters::from_slice(pp_bytes).unwrap();
					let vd = VerifierData::from_slice(vd_bytes).unwrap();
					let mut proof_array = [0u8; 1488];
					for i in 0..1488 {
						proof_array[i] = proof_bytes[i];
					}
					let label = b"mixer-verifier";
					let proof = Proof::from_bytes(&proof_array).unwrap();
					let verification_result = verify(&pp, &vd, &proof, &[], label);
					debug(&verification_result);
					Ok(())
				},
				(_, _) => Ok(()),
			}
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account()
	}
	pub fn insert_into_tree(leaf: &[u8; 32]) -> u32 {
		let next_index = Self::next_index();
		assert_ne!(next_index, 2u32.saturating_pow(<T as Config>::MerkleTreeLevels::get()));

		let mut current_index = next_index;
		let mut current_level_hash = leaf.clone();
		let mut left: [u8; 32];
		let mut right: [u8; 32];
		for i in 0..<T as Config>::MerkleTreeLevels::get() {
			if current_index % 2 == 0 {
				FilledSubtreesStorage::<T>::insert(i, &current_level_hash);
				left = current_level_hash;
				right = Self::zeroes(i);
			} else {
				left = Self::filled_subtrees(i);
				right = current_level_hash;
			}
			debug(&left);
			debug(&right);
			current_level_hash = dusk_poseidon::sponge::hash(&[
				BlsScalar::from_bytes(&left).unwrap(),
				BlsScalar::from_bytes(&right).unwrap(),
			])
			.to_bytes();
			current_index /= 2;
		}
		let new_root_index =
			(Self::current_root_index() + 1) % <T as Config>::RootHistorySize::get();
		CurrentRootIndexStorage::<T>::put(new_root_index);
		RootsStorage::<T>::insert(new_root_index, current_level_hash);
		NextIndexStorage::<T>::put(next_index + 1);
		next_index
	}

	pub fn zeroes(_idx: u32) -> [u8; 32] {
		[
			100, 72, 182, 70, 132, 238, 57, 168, 35, 213, 254, 95, 213, 36, 49, 220, 129, 228, 129,
			123, 242, 195, 234, 60, 171, 158, 35, 158, 251, 245, 152, 32,
		]
	}
}
