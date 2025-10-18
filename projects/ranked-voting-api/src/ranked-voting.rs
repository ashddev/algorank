use core::iter;
use ark_std::UniformRand;
use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::{AffineCurve, ProjectiveCurve};
use merlin::Transcript;
use std::collections::HashMap;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use curdleproofs::msm_accumulator::MsmAccumulator;
use curdleproofs::same_permutation_argument::SamePermutationProof;
use curdleproofs::util::{generate_blinders, msm};

pub struct SetupParameters {
    crs_g_vec: Vec<G1Affine>,
    crs_h_vec: Vec<G1Affine>,
    crs_u: G1Projective,
    crs_g_sum: G1Affine,
    crs_h_sum: G1Affine,
    a: Options,
    ballot_size: usize
}

pub struct Options {
    scores: Vec<u32>,
    scores_as_field_elements: Vec<Fr>
}

#[derive(Clone)] 
pub struct RankedVotingProof {
    proof: SamePermutationProof,
    committed_ballot: G1Projective,
    committed_permutation: G1Projective,
}

pub fn setup(ballot_size: usize, seed: u64) -> SetupParameters {
    let mut rng: StdRng = StdRng::from_seed(seed);

    let crs_g_vec: Vec<_> = iter::repeat_with(|| G1Projective::rand(&mut rng).into_affine())
        .take(ballot_size)
        .collect();
    let crs_h_vec: Vec<_> = iter::repeat_with(|| G1Projective::rand(&mut rng).into_affine())
        .take(ballot_size)
        .collect();

    let crs_u = G1Projective::rand(&mut rng);
    let crs_g_sum = sum_affine_points(&crs_g_vec);
    let crs_h_sum = sum_affine_points(&crs_h_vec);

    let scores: Vec<u32> = (0..ballot_size as u32).rev().collect();
    let scores_as_field_elements: Vec<Fr> = scores.iter().map(|&x| Fr::from(x)).collect();

    let a = Options {
        scores,
        scores_as_field_elements
    };

    SetupParameters {
        crs_g_vec,
        crs_h_vec,
        crs_u,
        crs_g_sum,
        crs_h_sum,
        a,
        ballot_size
    }
}

pub fn generate_vote(
    scores: &Vec<u32>,
    setup_params: &SetupParameters,
) -> Result<RankedVotingProof, String> {
    let permutation= find_permutation(&setup_params.a.scores, &scores)?;
    
    let mut rng: StdRng = StdRng::seed_from_u64(0u64);

    let ballot_size = scores.len();

    let ballot_fr: Vec<Fr> = scores.iter().map(|&x| Fr::from(x)).collect();
    let permutation_as_fr: Vec<Fr> = (0..permutation.len())
    .map(|i| Fr::from(permutation[i] as u64))
    .collect();

    let committed_ballot_blinders = generate_blinders(&mut rng, ballot_size);
    let committed_permutation_blinders = generate_blinders(&mut rng, ballot_size);

    let committed_ballot = msm(&setup_params.crs_g_vec, &ballot_fr) + msm(&setup_params.crs_h_vec, &committed_ballot_blinders);
    let committed_permutation: ark_ec::short_weierstrass_jacobian::GroupProjective<ark_bls12_381::g1::Parameters> = msm(&setup_params.crs_g_vec, &permutation_as_fr) + msm(&setup_params.crs_h_vec, &committed_permutation_blinders);

    let proof = SamePermutationProof::new(
        &setup_params.crs_g_vec,
        &setup_params.crs_h_vec,
        &setup_params.crs_u,
        committed_ballot,
        committed_permutation,
        &setup_params.a.scores_as_field_elements,
        permutation,
        committed_ballot_blinders,
        committed_permutation_blinders,
        &mut Transcript::new(b"sameperm"),
        &mut rng,
    );

    Ok(RankedVotingProof {
        proof,
        committed_ballot,
        committed_permutation,
    })
}

pub fn verify_proof(proof: &RankedVotingProof, setup_params: &SetupParameters) -> bool {
    let mut rng: StdRng = StdRng::seed_from_u64(0u64);
    let mut msm_accumulator = MsmAccumulator::default();

    let verification = proof.proof.verify(
        &setup_params.crs_g_vec,
        &setup_params.crs_h_vec,
        &setup_params.crs_u,
        &setup_params.crs_g_sum,
        &setup_params.crs_h_sum,
        &proof.committed_ballot,
        &proof.committed_permutation,
        &setup_params.a.scores_as_field_elements,
        setup_params.ballot_size,
        &mut Transcript::new(b"sameperm"),
        &mut msm_accumulator,
        &mut rng,
    );

    verification.is_ok() && msm_accumulator.verify().is_ok()
}

fn sum_affine_points(affine_points: &[G1Affine]) -> G1Affine {
    affine_points
        .iter()
        .map(|affine| affine.into_projective())
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

    let permutation: Vec<u32> = vec_b
        .iter()
        .map(|&val| {
            index_map
                .get(&val)
                .map(|&i| i as u32)
                .ok_or_else(|| format!("Value {} in vec_b not found in vec_a", val))
        })
        .collect::<Result<_, _>>()?;

    Ok(permutation)
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_permutation_proof() {
        let ballot = vec![1, 0, 2, 3];
        let setup_params = setup(ballot.len());

        let proof = generate_vote(&ballot, &setup_params).expect("Should generate proof");
        assert!(verify_proof(&proof, &setup_params), "Proof should verify for valid permutation");
    }

    #[test]
    fn test_invalid_permutation_proof() {
        let ballot = vec![1, 0, 2, 2];
        let setup_params = setup(ballot.len());

        let result = generate_vote(&ballot, &setup_params);
        assert!(result.is_err(), "Should fail to generate proof for invalid permutation");
    }

    #[test]
    fn test_mismatched_vector_length() {
        let ballot = vec![2, 3];
        let setup_params = setup(3);

        let result = generate_vote(&ballot, &setup_params);
        assert!(result.is_err(), "Should fail due to mismatched lengths");
    }

    #[test]
    fn test_proof_integrity_fails_on_tamper() {
        let ballot = vec![0, 1, 2, 3];
        let setup_params = setup(ballot.len());

        let mut proof = generate_vote(&ballot, &setup_params).expect("Proof should be valid");

        proof.committed_permutation = G1Projective::rand(&mut StdRng::seed_from_u64(999));

        assert!(!verify_proof(&proof, &setup_params), "Tampered proof should not verify");
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