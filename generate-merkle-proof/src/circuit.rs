use super::*;
use dusk_bls12_381::BlsScalar;
use dusk_plonk::{
    constraint_system::TurboComposer,
    prelude::{Circuit, PublicInputValue},
};
use dusk_poseidon::sponge;

pub const CAPACITY: usize = 12;

#[derive(Debug, Default)]
pub struct MerkleTreeCircuit {
    level: usize,
    leaf: BlsScalar,
    path_idx: Vec<usize>,
    path_els: Vec<BlsScalar>,
    path_root: BlsScalar,
}

impl MerkleTreeCircuit {
    pub fn new(level: usize, leaf: BlsScalar, path: ProofPath) -> Self {
        Self {
            path_idx: path.path_idx,
            path_els: path.path_els,
            path_root: path.path_root,
            leaf,
            level,
        }
    }
}

impl Circuit for MerkleTreeCircuit {
    const CIRCUIT_ID: [u8; 32] = [0xff; 32];
    fn gadget(&mut self, composer: &mut TurboComposer) -> Result<(), dusk_plonk::error::Error> {
        let leaf = composer.append_witness(self.leaf);
        let mut hashers = vec![TurboComposer::constant_zero(); self.level];
        let mut witness_idx = vec![];
        let mut witness_els = vec![];
        for (a, b) in self.path_idx.iter().zip(self.path_els.iter()) {
            witness_idx.push(composer.append_witness(*a as u64));
            witness_els.push(composer.append_witness(*b));
        }

        for i in 0..self.level {
            composer.component_boolean(witness_idx[i]);
            let leaf_or_previous_hash = if i == 0 { leaf } else { hashers[i - 1] };
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
