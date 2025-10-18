use core::iter;
use std::collections::HashMap;
use std::io::Cursor;

use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::{CurveGroup, AffineRepr};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, SerializationError, Compress, Validate};
use ark_std::rand::{rngs::StdRng, SeedableRng};
use ark_std::UniformRand;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use rocket::serde::{Serialize, Deserialize};

use curdleproofs::msm_accumulator::MsmAccumulator;
use curdleproofs::same_permutation_argument::SamePermutationProof;
use curdleproofs::util::{generate_blinders, msm};
use curdleproofs::N_BLINDERS;
use merlin::Transcript;

pub struct SetupParameters {
    pub crs_g_vec: Vec<G1Affine>,
    pub crs_h_vec: Vec<G1Affine>,
    pub crs_u: G1Projective,
    pub crs_g_sum: G1Affine,
    pub crs_h_sum: G1Affine,
}

#[derive(Clone, Debug)] 
pub struct RankedVotingProof {
    pub proof: SamePermutationProof,
    pub committed_ballot: G1Projective,
    pub committed_permutation: G1Projective,
}

pub fn setup(n: usize, seed: u64) -> SetupParameters {
    let mut rng = StdRng::seed_from_u64(seed);

    let crs_g_vec: Vec<_> = iter::repeat_with(|| G1Projective::rand(&mut rng).into_affine())
        .take(n)
        .collect();
    let crs_h_vec: Vec<_> = iter::repeat_with(|| G1Projective::rand(&mut rng).into_affine())
        .take(N_BLINDERS)
        .collect();

    let crs_u = G1Projective::rand(&mut rng);

    let crs_g_sum = sum_affine_points(&crs_g_vec);
    let crs_h_sum = sum_affine_points(&crs_h_vec);

    SetupParameters { crs_g_vec, crs_h_vec, crs_u, crs_g_sum, crs_h_sum }
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
    let r_ballot = generate_blinders(&mut rng, N_BLINDERS);
    let r_permutation = generate_blinders(&mut rng, N_BLINDERS);

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
        N_BLINDERS,
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

#[derive(Debug)]
pub struct ProofPartsBytes {
    pub log2_n: u8,
    pub committed_ballot: Vec<u8>,
    pub committed_permutation: Vec<u8>,
    pub proof: Vec<u8>,
}

pub fn proof_to_parts_bytes(p: &RankedVotingProof, ell: usize)
    -> Result<ProofPartsBytes, SerializationError>
{
    let n = ell + N_BLINDERS;
    let log2_n = n.checked_ilog2()
        .filter(|&k| (1usize << k) == n)
        .ok_or(SerializationError::InvalidData)? as u8;

    let mut c_ballot = Vec::new();
    p.committed_ballot.serialize_uncompressed(&mut c_ballot)?;

    let mut c_perm = Vec::new();
    p.committed_permutation.serialize_uncompressed(&mut c_perm)?;

    let mut proof = Vec::new();
    p.proof.serialize(&mut proof)?;

    Ok(ProofPartsBytes { log2_n, committed_ballot: c_ballot, committed_permutation: c_perm, proof })
}


pub fn proof_from_parts_bytes(parts: &ProofPartsBytes)
    -> Result<RankedVotingProof, SerializationError>
{
    let mut r1 = Cursor::new(parts.committed_ballot.as_slice());
    let committed_ballot = G1Projective::deserialize_uncompressed(&mut r1)?;

    let mut r2 = Cursor::new(parts.committed_permutation.as_slice());
    let committed_permutation = G1Projective::deserialize_uncompressed(&mut r2)?;

    let mut r3 = Cursor::new(parts.proof.as_slice());
    let proof = SamePermutationProof::deserialize(&mut r3, parts.log2_n as usize)?;

    Ok(RankedVotingProof { proof, committed_ballot, committed_permutation })
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct ProofPartsB64 {
    pub log2_n: u8,
    pub committed_ballot: String,
    pub committed_permutation: String,
    pub proof: String,
}

pub fn proof_to_parts_b64(p: &RankedVotingProof, ell: usize) -> Result<ProofPartsB64, String> {
    let parts = proof_to_parts_bytes(p, ell).map_err(|e| e.to_string())?;
    Ok(ProofPartsB64 {
        log2_n: parts.log2_n,
        committed_ballot: B64.encode(parts.committed_ballot),
        committed_permutation: B64.encode(parts.committed_permutation),
        proof: B64.encode(parts.proof),
    })
}

pub fn proof_from_parts_b64(x: &ProofPartsB64) -> Result<RankedVotingProof, String> {
    let parts = ProofPartsBytes {
        log2_n: x.log2_n,
        committed_ballot: B64.decode(&x.committed_ballot).map_err(|e| e.to_string())?,
        committed_permutation: B64.decode(&x.committed_permutation).map_err(|e| e.to_string())?,
        proof: B64.decode(&x.proof).map_err(|e| e.to_string())?,
    };
    proof_from_parts_bytes(&parts).map_err(|e| e.to_string())
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

    #[test]
    fn roundtrip_parts_and_verify() {
        let ballot = vec![1,0,2,3];
        let setup_seed = 42;
        let proof_seed = 999;

        let proof = generate_vote(&ballot, setup_seed, proof_seed).unwrap();

        let parts = proof_to_parts_bytes(&proof, ballot.len()).unwrap();

        let proof2 = proof_from_parts_bytes(&parts).unwrap();

        assert!(verify_proof(&proof2, ballot.len(), setup_seed, proof_seed));
    }

}