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
	sp_runtime::traits::AccountIdConversion,
	storage::bounded_btree_map::BoundedBTreeMap,
	traits::{Currency, ExistenceRequirement, Get},
	PalletId,
};
use frame_system::pallet_prelude::*;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

//#[derive(Encode, Decode, Clone, PartialEq, Eq, MaxEncodedLen, RuntimeDebug, TypeInfo)]
//pub struct MerkleTreeWithHistory<R: Get<u32>> {
//	pub next_index: u32,
//	pub current_root_index: u32,
//	pub roots: BoundedBTreeMap<u32, [u8; 32], R>,
//	pub filled_subtrees: BoundedBTreeMap<u32, [u8; 32], R>,
//}
//
//impl<R: Get<u32>> Default for MerkleTreeWithHistory<R> {
//	fn default() -> Self {
//		let levels = R::get();
//		let filled_subtrees = BoundedBTreeMap::new();
//		let roots = BoundedBTreeMap::new();
//		Self { filled_subtrees, next_index: 0, current_root_index: 0, roots }
//	}
//}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use dusk_bytes::Serializable;
	use dusk_plonk::{circuit::verify, prelude::*};

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
			// Create Treasury account
			let account_id = <Pallet<T>>::account_id();
			let min = T::Currency::minimum_balance();
			if T::Currency::free_balance(&account_id) < min {
				let _ = T::Currency::make_free_balance_be(&account_id, min);
			}
			debug(&T::Currency::total_balance(&account_id));
			//<CommitmentStorage<T>>::put(BoundedBTreeSet::new());
		}
	}
	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		Deposit([u8; 32], u32),
		PaublicParameteresStored(u32, T::AccountId),
		VerifierDataStored(u32, T::AccountId),
	}

	// Errors inform users that something went wrong.
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
			assert_eq!(<CommitmentStorage<T>>::contains_key(&commitment), false);
			<CommitmentStorage<T>>::insert(commitment.clone(), ());
			let account_id = Self::account_id();

			T::Currency::transfer(&who, &account_id, value, ExistenceRequirement::AllowDeath)?;
			debug(&T::Currency::total_balance(&account_id));
			Self::deposit_event(Event::Deposit(commitment, 0));
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn store_parameters(origin: OriginFor<T>, parameters: Vec<u8>) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://docs.substrate.io/v3/runtime/origins
			let who = ensure_signed(origin)?;
			//debug(&pp.to_var_bytes().len());
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
				Err(_e) => {
					//debug(&format!("Failed to convert vec to boundedvec: {:?}", &e));
					Ok(())
				},
			}
		}
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn store_vd(origin: OriginFor<T>, verifier_data: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let vd_size = verifier_data.len() as u32;
			let bvec: BoundedVec<u8, T::MaxVerifierDataLen> = verifier_data.try_into().unwrap();
			// Update storage.
			<VerifierDataStorage<T>>::put(bvec);

			// Emit an event.
			Self::deposit_event(Event::VerifierDataStored(vd_size, who));
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}

		/// An example dispatchable that may throw a custom error.
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
}
