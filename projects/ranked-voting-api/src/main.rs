#[macro_use]
extern crate rocket;

use rocket::serde::{json::Json, Deserialize, Serialize};

mod ranked_voting;
use ranked_voting::{
    generate_vote, verify_proof,
    ProofPartsB64,
    proof_to_parts_b64, proof_from_parts_b64,
};
use curdleproofs::N_BLINDERS;

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct GenerateIn {
    ballot: Vec<u32>,
    setup_seed: u64,
    proof_seed: u64,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct GenerateOut {
    ok: bool,
    error: Option<String>,
    proof: Option<ProofPartsB64>,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct VerifyIn {
    // Split proof parts (includes log2_n)
    proof: ProofPartsB64,
    setup_seed: u64,
    proof_seed: u64,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct VerifyOut {
    ok: bool,
    error: Option<String>,
}

#[post("/generate", data = "<inp>")]
async fn generate_route(inp: Json<GenerateIn>) -> Json<GenerateOut> {
    match generate_vote(&inp.ballot, inp.setup_seed, inp.proof_seed) {
        Ok(p) => match proof_to_parts_b64(&p, inp.ballot.len()) {
            Ok(parts) => Json(GenerateOut { ok: true, error: None, proof: Some(parts) }),
            Err(e) => Json(GenerateOut { ok: false, error: Some(e), proof: None }),
        },
        Err(e) => Json(GenerateOut { ok: false, error: Some(e), proof: None }),
    }
}

#[post("/verify", data = "<inp>")]
async fn verify_route(inp: Json<VerifyIn>) -> Json<VerifyOut> {
    // Rebuild the proof + commitments from JSON parts
    let ranked = match proof_from_parts_b64(&inp.proof) {
        Ok(p) => p,
        Err(e) => return Json(VerifyOut { ok: false, error: Some(format!("decode parts: {e}")) }),
    };

    // Recover ell (ballot length) from log2_n: n = 2^k, and n = ell + N_BLINDERS
    let k = inp.proof.log2_n as usize;
    let n = 1usize << k;
    if n < N_BLINDERS {
        return Json(VerifyOut { ok: false, error: Some("invalid log2_n (n < N_BLINDERS)".into()) });
    }
    let ell = n - N_BLINDERS;

    let ok = verify_proof(&ranked, ell, inp.setup_seed, inp.proof_seed);
    Json(VerifyOut { ok, error: None })
}

#[get("/selfcheck")]
async fn selfcheck() -> String {
    let ballot = vec![0,1,2,3]; // ell = 8
    let setup_seed = 42u64;
    let proof_seed = 7u64;

    let p = match ranked_voting::generate_vote(&ballot, setup_seed, proof_seed) {
        Ok(p) => p, Err(e) => return format!("generate error: {e}")
    };

    // parts (bytes and b64)
    let b = match ranked_voting::proof_to_parts_bytes(&p, ballot.len()) {
        Ok(x) => x, Err(e) => return format!("to_parts_bytes error: {e}")
    };
    let bb64 = match ranked_voting::proof_to_parts_b64(&p, ballot.len()) {
        Ok(x) => x, Err(e) => return format!("to_parts_b64 error: {e}")
    };

    // from bytes
    let p1 = match ranked_voting::proof_from_parts_bytes(&b) {
        Ok(x) => x, Err(e) => return format!("from_parts_bytes error: {e:?}")
    };
    let ok1 = ranked_voting::verify_proof(&p1, ballot.len(), setup_seed, proof_seed);

    // from b64
    let p2 = match ranked_voting::proof_from_parts_b64(&bb64) {
        Ok(x) => x, Err(e) => return format!("from_parts_b64 error: {e}")
    };
    let ok2 = ranked_voting::verify_proof(&p2, ballot.len(), setup_seed, proof_seed);

    format!(
        "selfcheck ok_bytes={ok1} ok_b64={ok2} log2_n={} sizes: cb={}, cp={}, pr={}",
        bb64.log2_n, b.committed_ballot.len(), b.committed_permutation.len(), b.proof.len()
    )
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![generate_route, verify_route, selfcheck])
}
