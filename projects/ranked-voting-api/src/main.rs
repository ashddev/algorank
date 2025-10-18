#[macro_use]
extern crate rocket;

use rocket::serde::{json::Json, Deserialize, Serialize};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

mod ranked_voting;
use ranked_voting::{generate_vote, verify_proof, proof_to_bytes, proof_from_bytes};

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
    proof: Option<String>,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct VerifyIn {
    proof: String,
    n: usize,
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
        Ok(p) => match proof_to_bytes(&p) {
            Ok(bytes) => {
                let proof_b64 = B64.encode(bytes);
                Json(GenerateOut { ok: true, error: None, proof: Some(proof_b64) })
            }
            Err(e) => Json(GenerateOut { ok: false, error: Some(e.to_string()), proof: None }),
        },
        Err(e) => Json(GenerateOut { ok: false, error: Some(e), proof: None }),
    }
}

#[post("/verify", data = "<inp>")]
async fn verify_route(inp: Json<VerifyIn>) -> Json<VerifyOut> {
    let decoded = match B64.decode(&inp.proof) {
        Ok(b) => b,
        Err(e) => return Json(VerifyOut { ok: false, error: Some(format!("base64 decode error: {e}")) }),
    };

    let proof = match proof_from_bytes(&decoded, inp.n) {
        Ok(p) => p,
        Err(e) => return Json(VerifyOut { ok: false, error: Some(e.to_string()) }),
    };

    let ok = verify_proof(&proof, inp.n, inp.setup_seed, inp.proof_seed);
    Json(VerifyOut { ok, error: None })
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![generate_route, verify_route])
}
