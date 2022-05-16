use super::*;
use dusk_bls12_381::BlsScalar;
use dusk_plonk::{
    constraint_system::TurboComposer,
    prelude::{Circuit, PublicInputValue},
};
use dusk_poseidon::sponge;

pub const CAPACITY: usize = 13;

#[derive(Debug, Default)]
pub struct WithdrawCircuit {
    secret: BlsScalar,
    nullifier: BlsScalar,
    nullifier_hash: BlsScalar,
    level: usize,
    path_idx: Vec<usize>,
    path_els: Vec<BlsScalar>,
    path_root: BlsScalar,
}

impl WithdrawCircuit {
    pub fn new(
        level: usize,
        secret: BlsScalar,
        nullifier: BlsScalar,
        nullifier_hash: BlsScalar,
        path: ProofPath,
    ) -> Self {
        Self {
            secret,
            nullifier,
            nullifier_hash,
            path_idx: path.path_idx,
            path_els: path.path_els,
            path_root: path.path_root,
            level,
        }
    }
}

impl Circuit for WithdrawCircuit {
    const CIRCUIT_ID: [u8; 32] = [0xff; 32];
    fn gadget(&mut self, composer: &mut TurboComposer) -> Result<(), dusk_plonk::error::Error> {
        let nullifier = composer.append_witness(self.nullifier);
        let secret = composer.append_witness(self.secret);
        let commitment = sponge::gadget(composer, &[secret, nullifier]);
        let nullifier_hash = composer.append_witness(self.nullifier_hash);
        let computed_nullifier_hash = sponge::gadget(composer, &[nullifier]);
        composer.assert_equal(nullifier_hash, computed_nullifier_hash);

        let mut hashers = vec![TurboComposer::constant_zero(); self.level];
        let mut witness_idx = vec![];
        let mut witness_els = vec![];
        for (a, b) in self.path_idx.iter().zip(self.path_els.iter()) {
            witness_idx.push(composer.append_witness(*a as u64));
            witness_els.push(composer.append_witness(*b));
        }

        for i in 0..self.level {
            composer.component_boolean(witness_idx[i]);
            let leaf_or_previous_hash = if i == 0 { commitment } else { hashers[i - 1] };
            let left =
                composer.component_select(witness_idx[i], witness_els[i], leaf_or_previous_hash);
            let right =
                composer.component_select(witness_idx[i], leaf_or_previous_hash, witness_els[i]);
            hashers[i] = sponge::gadget(composer, &[left, right]);
        }
        let root = composer.append_witness(self.path_root);

        composer.assert_equal(root, hashers[self.level - 1]);
        Ok(())
    }

    fn public_inputs(&self) -> Vec<PublicInputValue> {
        vec![]
    }

    fn padded_gates(&self) -> usize {
        1 << CAPACITY
    }
}
