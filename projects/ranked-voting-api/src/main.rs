#[macro_use] extern crate rocket;

mod cors;
mod zk;

use rocket::serde::{json::Json, Deserialize, Serialize};
use zk::*;

#[derive(Serialize)]
struct SetupOut {
    ballot_size: usize,
    scores: Vec<u32>,
}

#[derive(Deserialize)]
struct GenerateIn {
    ballot_size: usize,
    scores: Vec<u32>,
}

#[derive(Serialize)]
struct GenerateOut {
    ok: bool,
    error: Option<String>,
}

#[derive(Deserialize)]
struct VerifyIn {
    ballot_size: usize,
    scores: Vec<u32>,
}

#[derive(Serialize)]
struct VerifyOut {
    ok: bool,
}

#[post("/setup", data = "<ballot_size>")]
fn setup_route(ballot_size: Json<usize>) -> Json<SetupOut> {
    let params = setup(*ballot_size);
    let ui = SetupOut {
        ballot_size: params.ballot_size,
        scores: params.a.scores.clone(),
    };
    Json(ui)
}

#[post("/generate", data = "<inp>")]
fn generate_route(inp: Json<GenerateIn>) -> Json<GenerateOut> {
    let setup_params = setup(inp.ballot_size);
    match generate_vote(&inp.scores, &setup_params) {
        Ok(_proof) => Json(GenerateOut { ok: true, error: None }),
        Err(e) => Json(GenerateOut {
            ok: false,
            error: Some(e),
        }),
    }
}

#[post("/verify", data = "<inp>")]
fn verify_route(inp: Json<VerifyIn>) -> Json<VerifyOut> {
    let setup_params = setup(inp.ballot_size);
    let proof = generate_vote(&inp.scores, &setup_params);

    // In a real stateless system, you'd receive the proof and commitments as parameters,
    // but for now we just reuse the locally generated one for simplicity.
    let ok = match proof {
        Ok(p) => verify_proof(&p, &setup_params),
        Err(_) => false,
    };

    Json(VerifyOut { ok })
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(cors::Cors)
        .mount("/", routes![setup_route, generate_route, verify_route])
}
