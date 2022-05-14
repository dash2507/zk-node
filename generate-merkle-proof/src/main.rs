use dusk_bls12_381::BlsScalar;
use dusk_bytes::Serializable;
use dusk_poseidon::sponge;

mod circuit;
use circuit::*;

const ZERO: [u8; 32] = [
    100, 72, 182, 70, 132, 238, 57, 168, 35, 213, 254, 95, 213, 36, 49, 220, 129, 228, 129, 123,
    242, 195, 234, 60, 171, 158, 35, 158, 251, 245, 152, 32,
];

pub fn construct_zeroes(zero: BlsScalar, levels: u32) -> Vec<BlsScalar> {
    let mut zeroes = vec![zero];
    for i in 1..levels as usize {
        zeroes.push(sponge::hash(&[
            zeroes[i - 1].clone(),
            zeroes[i - 1].clone(),
        ]));
    }
    zeroes
}

#[derive(Debug)]
pub struct ProofPath {
    path_idx: Vec<usize>,
    path_els: Vec<BlsScalar>,
    //path_position: Vec<usize>,
    path_root: BlsScalar,
}

#[derive(Debug)]
pub struct FixedMerkleTree {
    levels: u32,
    //zero_element: BlsScalar,
    zeroes: Vec<BlsScalar>,
    layers: Vec<Vec<BlsScalar>>,
}

impl FixedMerkleTree {
    pub fn new(levels: u32, zero_element: &[u8; 32]) -> Self {
        let zero_element = BlsScalar::from_bytes(zero_element).unwrap();
        let zeroes = construct_zeroes(zero_element, levels);
        let layers = vec![vec![]; (levels + 1) as usize];
        Self {
            levels,
            //zero_element,
            zeroes,
            layers,
        }
    }

    pub fn capacity(&self) -> u32 {
        1 << self.levels
    }

    pub fn zeros(&self, level: usize) -> BlsScalar {
        self.zeroes[level].clone()
    }

    pub fn layers(&self) -> Vec<Vec<BlsScalar>> {
        self.layers.clone()
    }
    pub fn root(&self) -> BlsScalar {
        self.layers[self.levels as usize][0]
    }

    fn index_of(&self, level: usize, leaf: BlsScalar) -> usize {
        let index = self.layers[level].iter().position(|x| x.eq(&leaf));
        index.unwrap()
    }

    pub fn path(&self, index: usize) -> ProofPath {
        let mut path_idx = vec![];
        let mut path_els = vec![];
        let mut path_position = vec![];
        let mut el_index = index;
        for i in 0..self.levels as usize {
            path_idx.push(el_index % 2);
            let leaf_index = el_index ^ 1;
            if leaf_index < self.layers[i].len() {
                path_els.push(self.layers[i][leaf_index]);
                path_position.push(leaf_index);
            } else {
                path_els.push(self.zeroes[i]);
                path_position.push(0);
            }
            el_index >>= 1;
        }
        ProofPath {
            path_idx,
            path_els,
            //path_position,
            path_root: self.root(),
        }
    }

    pub fn proof(&self, leaf: BlsScalar) -> ProofPath {
        let index = self.index_of(0, leaf);
        self.path(index)
    }

    pub fn insert(&mut self, leaf: BlsScalar) -> () {
        let index = {
            let capacity = self.capacity();
            let layer_zero = &mut self.layers[0];
            assert!(
                layer_zero.len() < capacity as usize,
                "Tree is already full."
            );
            layer_zero.push(leaf);
            layer_zero.len()
        };
        self.process_update(index - 1);
    }

    fn process_update(&mut self, mut index: usize) {
        for i in 1..=self.levels as usize {
            index >>= 1;
            //println!("level: {}, index: {}", i, index);
            let left = self.layers[i - 1][index * 2];
            let right = if index * 2 + 1 < self.layers[i - 1].len() {
                self.layers[i - 1][index * 2 + 1]
            } else {
                self.zeroes[i - 1]
            };
            if index < self.layers[i].len() {
                self.layers[i][index] = sponge::hash(&[left, right]);
            } else {
                self.layers[i].push(sponge::hash(&[left, right]));
            }
        }
    }
}

const l1: &str = "bb67ed265bf1db490ded2e1ede55c0d14c55521509dc73f9c354e98ab76c9625";
const l2: &str = "7e74220084d75e10c89e9435d47bb5b8075991b2e29be3b84421dac3b1ee6007";
const l3: &str = "5ce5481a4d78cca03498f72761da1b9f1d2aa8fb300be39f0e4fe2534f9d4308";
const l4: &str = "b1e710e3c4a8c35154b0ce4e4f4af6f498ebd79f8e7cdf3150372c7501be250b";

fn main() {}

#[cfg(test)]
mod test {
    use dusk_bytes::ParseHexStr;
    use dusk_plonk::prelude::{Circuit, Error, PublicParameters};
    use rand::{prelude::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn simple_insert() {
        let mut mt = FixedMerkleTree::new(2, &ZERO);
        mt.insert(BlsScalar::from_hex_str(l1).unwrap());
        mt.insert(BlsScalar::from_hex_str(l2).unwrap());
        mt.insert(BlsScalar::from_hex_str(l1).unwrap());
        mt.insert(BlsScalar::from_hex_str(l2).unwrap());
        //mt.insert(BlsScalar::from_hex_str(l1).unwrap());
        println!("{:?}", mt.layers);
    }

    #[test]
    fn test_path() {
        let mut mt = FixedMerkleTree::new(2, &ZERO);
        mt.insert(BlsScalar::from_hex_str(l1).unwrap());
        mt.insert(BlsScalar::from_hex_str(l2).unwrap());
        mt.insert(BlsScalar::from_hex_str(l3).unwrap());
        mt.insert(BlsScalar::from_hex_str(l4).unwrap());
        //println!("{:?}", mt.proof(BlsScalar::from_hex_str(l1).unwrap()));
        //println!("{:?}", mt.proof(BlsScalar::from_hex_str(l2).unwrap()));
        //println!("{:?}", mt.proof(BlsScalar::from_hex_str(l3).unwrap()));
        //println!("{:?}", mt.proof(BlsScalar::from_hex_str(l4).unwrap()));
    }

    #[test]
    fn test_circuit() -> Result<(), Error> {
        use std::fs;
        use std::path::Path;

        let level = 3;
        let dummy_proof = ProofPath {
            path_els: vec![BlsScalar::zero(); level],
            path_idx: vec![0; level],
            path_root: BlsScalar::zero(),
        };
        let label = b"mixer-verifier";
        let seed = [2u8; 32];
        let mut rng = StdRng::from_seed(seed);
        let pp = PublicParameters::setup(1 << circuit::CAPACITY, &mut rng)?;
        println!("public paremeters generated");
        let pp_bytes = pp.to_var_bytes();
        let hex_string = format!("0x{}", hex::encode(pp_bytes));

        fs::write(Path::new("pp.txt"), hex_string).unwrap();

        let mut mt = FixedMerkleTree::new(level as u32, &ZERO);
        mt.insert(BlsScalar::from_hex_str(l1).unwrap());
        mt.insert(BlsScalar::from_hex_str(l2).unwrap());
        mt.insert(BlsScalar::from_hex_str(l3).unwrap());
        mt.insert(BlsScalar::from_hex_str(l4).unwrap());

        let proof = mt.proof(BlsScalar::from_hex_str(l3).unwrap());
        println!("merkle proof generated successfully");
        let (pk, vd) =
            MerkleTreeCircuit::new(level as usize, BlsScalar::zero(), dummy_proof).compile(&pp)?;
        //dbg!(&vd.to_var_bytes().len());

        fs::write(
            Path::new("vd.txt"),
            format!("0x{}", hex::encode(vd.to_var_bytes())),
        )
        .unwrap();
        println!("compiled circuit");
        let proof =
            MerkleTreeCircuit::new(level as usize, BlsScalar::from_hex_str(l3).unwrap(), proof)
                .prove(&pp, &pk, label)?;
        println!("zk proof generated");
        fs::write(
            Path::new("proof.txt"),
            format!("0x{}", hex::encode(proof.to_bytes())),
        )
        .unwrap();

        MerkleTreeCircuit::verify(&pp, &vd, &proof, &[], label)?;
        println!("proof verified");

        //let proof = mt.proof(BlsScalar::from_hex_str(l2).unwrap());
        //println!("merkle proof generated successfully");
        //let proof =
        //    MerkleTreeCircuit::new(level as usize, BlsScalar::from_hex_str(l2).unwrap(), proof)
        //        .prove(&pp, &pk, label)?;
        //println!("zk proof generated");

        //MerkleTreeCircuit::verify(&pp, &vd, &proof, &[], label)?;
        //println!("proof verified");
        Ok(())
    }
}
