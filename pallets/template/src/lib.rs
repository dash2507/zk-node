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
	sp_runtime::{self, traits::AccountIdConversion},
	traits::{Currency, ExistenceRequirement, Get},
	PalletId,
};
use frame_system::pallet_prelude::*;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

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
		type MaxPublicParameterLen: Get<u32>;

		#[pallet::constant]
		type MaxVerifierDataLen: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// The pallet's runtime storage items.
	// https://docs.substrate.io/v3/runtime/storage
	#[pallet::storage]
	#[pallet::getter(fn srs)]
	// Learn more about declaring storage items:
	// https://docs.substrate.io/v3/runtime/storage#declaring-storage-items
	pub type PublicParametersStorage<T: Config> =
		StorageValue<_, BoundedVec<u8, T::MaxPublicParameterLen>>;

	#[pallet::storage]
	#[pallet::getter(fn verifier_data)]
	// Learn more about declaring storage items:
	// https://docs.substrate.io/v3/runtime/storage#declaring-storage-items
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

	#[cfg(feature = "std")]
	impl GenesisConfig {
		/// Direct implementation of `GenesisBuild::assimilate_storage`.
		#[deprecated(
			note = "use `<GensisConfig<T> as GenesisBuild<T>>::assimilate_storage` instead"
		)]
		pub fn assimilate_storage<T: Config>(
			&self,
			storage: &mut sp_runtime::Storage,
		) -> Result<(), String> {
			<Self as GenesisBuild<T>>::assimilate_storage(self, storage)
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
		}
	}
	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		PaublicParameteresStored(u32, T::AccountId),
		VerifierDataStored(u32, T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		ConvertionVecToBVecFail,
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// An example dispatchable that takes a singles value as a parameter, writes the value to
		/// storage and emits an event. This function must be dispatched by a signed extrinsic.
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

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn deposit(
			origin: OriginFor<T>,
			value: BalanceOf<T>,
			//commitment: T::Hash,
		) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://docs.substrate.io/v3/runtime/origins
			let who = ensure_signed(origin)?;
			let account_id = Self::account_id();
			let _ =
				T::Currency::transfer(&who, &account_id, value, ExistenceRequirement::AllowDeath);
			//let sender_balance = Self::get_balance(&who);
			debug(&T::Currency::total_balance(&account_id));
			Ok(())
			//debug(&pp.to_var_bytes().len());
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
