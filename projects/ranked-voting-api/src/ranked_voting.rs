use core::iter;
use std::collections::HashMap;
use std::io::Cursor;

use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::{CurveGroup};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, SerializationError};
use ark_std::rand::{rngs::StdRng, SeedableRng};
use ark_std::UniformRand;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use curdleproofs::msm_accumulator::MsmAccumulator;
use curdleproofs::same_permutation_argument::SamePermutationProof;
use curdleproofs::util::{generate_blinders, msm};
use merlin::Transcript;

pub struct SetupParameters {
    pub crs_g_vec: Vec<G1Affine>,
    pub crs_h_vec: Vec<G1Affine>,
    pub crs_u: G1Projective,
    pub crs_g_sum: G1Affine,
    pub crs_h_sum: G1Affine,
}

#[derive(Clone)] 
pub struct RankedVotingProof {
    proof: SamePermutationProof,
    committed_ballot: G1Projective,
    committed_permutation: G1Projective,
}

pub fn setup(n: usize, seed: u64) -> SetupParameters {
    let mut rng = StdRng::seed_from_u64(seed);

    let crs_g_vec: Vec<_> = iter::repeat_with(|| G1Projective::rand(&mut rng).into_affine())
        .take(n)
        .collect();
    let crs_h_vec: Vec<_> = iter::repeat_with(|| G1Projective::rand(&mut rng).into_affine())
        .take(n)
        .collect();

    let crs_u = G1Projective::rand(&mut rng);

    let crs_g_sum = sum_affine_points(&crs_g_vec);
    let crs_h_sum = sum_affine_points(&crs_h_vec);

    SetupParameters {
        crs_g_vec,
        crs_h_vec,
        crs_u,
        crs_g_sum,
        crs_h_sum,
    }
}

pub fn generate_vote(
    ballot: &[u32],
    setup_seed: u64,
    proof_seed: u64
) -> Result<RankedVotingProof, String> {
    let n = ballot.len();
    let a_indices: Vec<u32> = (0..n as u32).collect();
    let permutation = find_permutation(&a_indices, &ballot)?;

    let a_fr: Vec<Fr> = a_indices.iter().map(|&x| Fr::from(x)).collect();

    let setup_params = setup(n, setup_seed);

    let ballot_fr: Vec<Fr> = ballot.iter().map(|&x| Fr::from(x)).collect();
    let permutation_fr: Vec<Fr> = permutation.iter().map(|&x| Fr::from(x)).collect();

    let mut rng = StdRng::seed_from_u64(proof_seed);
    let r_ballot = generate_blinders(&mut rng, n);
    let r_permutation = generate_blinders(&mut rng, n);

    let committed_permutation = msm(&setup_params.crs_g_vec, &permutation_fr)
        + msm(&setup_params.crs_h_vec, &r_permutation);
    let committed_ballot = msm(&setup_params.crs_g_vec, &ballot_fr)
        + msm(&setup_params.crs_h_vec, &r_ballot);


    let proof = SamePermutationProof::new(
        &setup_params.crs_g_vec,
        &setup_params.crs_h_vec,
        &setup_params.crs_u,
        committed_ballot,
        committed_permutation,
        &a_fr,
        permutation,
        r_ballot,
        r_permutation,
        &mut Transcript::new(b"sameperm"),
        &mut rng,
    );

    Ok(RankedVotingProof {
        proof,
        committed_ballot,
        committed_permutation,
    })
}

pub fn verify_proof(proof: &RankedVotingProof, n: usize, setup_seed: u64, proof_seed: u64) -> bool {
    let setup_params = setup(n, setup_seed);

    let mut rng = StdRng::seed_from_u64(proof_seed);
    let mut msm_accumulator = MsmAccumulator::default();

    let a_fr: Vec<Fr> = (0..n as u32).map(Fr::from).collect();

    let verification = proof.proof.verify(
        &setup_params.crs_g_vec,
        &setup_params.crs_h_vec,
        &setup_params.crs_u,
        &setup_params.crs_g_sum,
        &setup_params.crs_h_sum,
        &proof.committed_ballot,
        &proof.committed_permutation,
        &a_fr,
        n,
        &mut Transcript::new(b"sameperm"),
        &mut msm_accumulator,
        &mut rng,
    );

    verification.is_ok() && msm_accumulator.verify().is_ok()
}

fn sum_affine_points(affine_points: &[G1Affine]) -> G1Affine {
    affine_points
        .iter()
        .sum::<G1Projective>()
        .into_affine()
}

fn find_permutation(vec_a: &[u32], vec_b: &[u32]) -> Result<Vec<u32>, String> {
    if vec_a.len() != vec_b.len() {
        return Err("Vectors must be the same length".to_string());
    }

    let mut index_map: HashMap<u32, usize> = HashMap::new();
    for (i, &val) in vec_a.iter().enumerate() {
        if index_map.insert(val, i).is_some() {
            return Err(format!("Duplicate value {} in vec_a not allowed", val));
        }
    }

    let mut seen = vec![false; vec_a.len()];

    let permutation: Vec<u32> = vec_b
        .iter()
        .map(|&val| {
            let &i = index_map
                .get(&val)
                .ok_or_else(|| format!("Value {} in vec_b not found in vec_a", val))?;
            if seen[i] {
                return Err(format!("Duplicate value {} in vec_b not allowed", val));
            }
            seen[i] = true;
            Ok(i as u32)
        })
        .collect::<Result<_, _>>()?;

    Ok(permutation)
}

pub fn proof_to_bytes(p: &RankedVotingProof) -> Result<Vec<u8>, SerializationError> {
    let mut buf = Vec::new();
    p.committed_ballot.serialize_compressed(&mut buf)?;
    p.committed_permutation.serialize_compressed(&mut buf)?;
    p.proof.serialize(&mut buf)?;
    Ok(buf)
}

pub fn proof_from_bytes(bytes: &[u8], log2_n: usize) -> Result<RankedVotingProof, SerializationError> {
    let mut cur = Cursor::new(bytes);
    let committed_ballot = G1Projective::deserialize_compressed(&mut cur)?;
    let committed_permutation = G1Projective::deserialize_compressed(&mut cur)?;
    let proof = SamePermutationProof::deserialize(&mut cur, log2_n)?;
    Ok(RankedVotingProof {
        proof,
        committed_ballot,
        committed_permutation,
    })
}

pub fn proof_to_b64(p: &RankedVotingProof) -> Result<String, String> {
    proof_to_bytes(p).map(|b| B64.encode(b)).map_err(|e| e.to_string())
}

pub fn proof_from_b64(s: &str, log2_n: usize) -> Result<RankedVotingProof, String> {
    let bytes = B64.decode(s).map_err(|e| format!("base64 decode: {e}"))?;
    proof_from_bytes(&bytes, log2_n).map_err(|e| e.to_string())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_permutation_proof() {
        let ballot = vec![1, 0, 2, 3];
        let setup_seed = 42;
        let proof_seed = 999;

        let proof = generate_vote(&ballot, setup_seed, proof_seed).expect("Should generate proof");
        assert!(verify_proof(&proof, ballot.len(), setup_seed, proof_seed), "Proof should verify for valid permutation");
    }

    #[test]
    fn test_invalid_permutation_proof() {
        let ballot = vec![1, 0, 2, 2];
        let setup_seed = 42;
        let proof_seed = 999;

        assert!(generate_vote(&ballot, setup_seed, proof_seed).is_err(), "Should fail to generate proof for invalid permutation");
    }

    #[test]
    fn test_mismatched_vector_length_still_checked_by_generate() {
        let a_vec = vec![0,1,2];
        let b_vec = vec![0,1];
        assert!(find_permutation(&a_vec, &b_vec).is_err());
    }

    #[test]
    fn test_proof_integrity_fails_on_tamper() {
        let ballot = vec![0, 1, 2, 3];
        let setup_seed = 42;
        let proof_seed = 999;

        let mut proof = generate_vote(&ballot, setup_seed, proof_seed).expect("Proof should be valid");

        proof.committed_permutation = G1Projective::rand(&mut StdRng::seed_from_u64(999));

        assert!(!verify_proof(&proof, ballot.len(), setup_seed, proof_seed), "Tampered proof should not verify");
    }

    #[test]
    fn test_find_permutation_correctness() {
        let a_vec = vec![7, 8, 9, 10];
        let b_vec = vec![8, 10, 9, 7];
    
        let permutation = find_permutation(&a_vec, &b_vec).expect("Should return a valid forward permutation");
    
        assert_eq!(permutation.len(), a_vec.len());
    
        for i in 0..a_vec.len() {
            let sigma_i = permutation[i] as usize;
            assert_eq!(a_vec[sigma_i], b_vec[i], "Mismatch at i={}", i);
        }
    }
}