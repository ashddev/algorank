# verifier.py
import os, time, base64, json, typing as t
import requests
import hashlib

from typing import Any, Dict, Optional

from dotenv import load_dotenv
from algosdk import account
from algosdk.abi import Method
from algosdk.v2client import indexer
from algokit_utils import AlgorandClient, AlgoAmount
from algosdk.atomic_transaction_composer import AccountTransactionSigner
from algokit_utils.transactions.transaction_composer import (
    AppCallMethodCallParams,
    PaymentParams,
)

load_dotenv()

APP_ID = int(os.environ.get("APP_ID", "1002"))
INDEXER_URL = os.environ.get("INDEXER_URL", "http://localhost:8980")
SLEEP_INTERVAL = int(os.environ.get("SLEEP_INTERVAL", "5"))
LOCAL_STATE_PROOF_KEY = os.environ.get("LOCAL_STATE_PROOF_KEY", "ballot_ipfs")
PINATA_GATEWAY_BASE = os.environ.get("PINATA_GATEWAY_BASE", "https://gateway.pinata.cloud")
PINATA_GATEWAY_TOKEN = os.environ.get("PINATA_GATEWAY_TOKEN")

IDX = indexer.IndexerClient("", INDEXER_URL)

U64_MOD = 1 << 64
COMMITMENT_SUM_KEY = os.environ.get("COMMITMENT_SUM_KEY", "commitment_sum")

# ---------- Helpers ----------
def method(sig: str) -> Method:
    return Method.from_signature(sig)


def call_abi_method(
    ac: AlgorandClient,
    app_id: int,
    m: Method,
    sender: str,
    signer: AccountTransactionSigner,
    args: list[Any] | None = None,
) -> Any:
    params = AppCallMethodCallParams(
        app_id=app_id,
        method=m,
        sender=sender,
        signer=signer,
        args=args or [],
    )
    result = ac.send.app_call_method_call(params)
    if getattr(result, "returns", None):
        r0 = result.returns[0]
        if hasattr(r0, "value"):
            return r0.value
    print("No ABI return. logs:", getattr(result, "logs", None))
    return None


def get_opted_in_accounts(app_id: int) -> list[str]:
    addrs: list[str] = []
    next_token = None
    while True:
        resp = IDX.accounts(application_id=app_id, next_page=next_token)
        addrs.extend(a["address"] for a in resp.get("accounts", []))
        next_token = resp.get("next-token")
        if not next_token:
            break
    return addrs


def decode_b64_str(b64: str) -> str:
    try:
        return base64.b64decode(b64).decode()
    except Exception:
        return b64


def decode_b64_bytes(b64: str) -> bytes:
    return base64.b64decode(b64)


def get_local_state_for(acct_addr: str, app_id: int) -> Dict[str, Any]:
    """Return dict of decoded key -> (bytes|uint)."""
    info = IDX.account_info(acct_addr)
    for app in info["account"].get("apps-local-state", []):
        if app["id"] == app_id:
            kvs: Dict[str, Any] = {}
            for kv in app.get("key-value", []):
                key = decode_b64_str(kv["key"])
                val = kv["value"]
                if "bytes" in val and val["bytes"]:
                    kvs[key] = decode_b64_bytes(val["bytes"])
                elif "uint" in val:
                    kvs[key] = val["uint"]
                else:
                    kvs[key] = val
            return kvs
    return {}


def maybe_set_verifier(ac: AlgorandClient, verifier_addr: str, signer: AccountTransactionSigner) -> None:
    try:
        msg = call_abi_method(
            ac,
            APP_ID,
            method("set_verifier()string"),
            sender=verifier_addr,
            signer=signer,
            args=[],
        )
        print("set_verifier ->", msg)
    except Exception as e:
        print("set_verifier skipped:", e)


def verify_one_ballot(
    ac: AlgorandClient,
    verifier_addr: str,
    voter_addr: str,
    new_commitment_sum: int,
    signer: AccountTransactionSigner,
) -> None:
    msg = call_abi_method(
        ac,
        APP_ID,
        method("verify_ballot(address,uint64)string"),
        sender=verifier_addr,
        signer=signer,
        args=[voter_addr, new_commitment_sum],
    )
    print("verify_ballot ->", msg)

def commitment_digest_u64(proof_json: dict) -> int:
    """
    64-bit digest bound to the proof contents.
    Uses BLAKE2b-64 over committed parts + proof bytes.
    """
    h = hashlib.blake2b(digest_size=8)
    h.update(base64.b64decode(proof_json["committed_ballot"]))
    h.update(base64.b64decode(proof_json["committed_permutation"]))
    h.update(base64.b64decode(proof_json["proof"]))
    return int.from_bytes(h.digest(), "big")

def get_app_global_uint(ac: AlgorandClient, app_id: int, key: str) -> int:
    """
    Read a uint value from application global state using AlgorandClient.
    Returns 0 if missing or not found.
    """
    try:
        app_info = ac.client.algod.application_info(app_id)
        global_state = app_info["params"].get("global-state", [])

        for kv in global_state:
            k = decode_b64_str(kv["key"])
            if k == key:
                val = kv.get("value", {})
                if "uint" in val:
                    return int(val["uint"])
                break
        return 0
    except Exception as e:
        print(f"⚠️ Error fetching global uint '{key}': {e}")
        return 0


class FetchError(Exception):
    pass

def _gateway_url_for_cid(cid: str) -> str:
    return f"{PINATA_GATEWAY_BASE.rstrip('/')}/ipfs/{cid}"

def fetch_ipfs_content(
    cid: str,
    *,
    timeout: int = 20,
    max_retries: int = 3,
    backoff: float = 0.75,
) -> bytes:
    url = _gateway_url_for_cid(cid)
    params = {}
    headers = {}

    params["pinataGatewayToken"] = PINATA_GATEWAY_TOKEN

    last_err = None
    for attempt in range(max_retries):
        try:
            r = requests.get(url, params=params, headers=headers, timeout=timeout, stream=True)
            if r.status_code == 200:
                return r.content
            if r.status_code in (408, 425, 429, 500, 502, 503, 504):
                last_err = FetchError(f"{r.status_code} from gateway")
            else:
                raise FetchError(f"Gateway returned {r.status_code}: {r.text[:200]}")
        except (requests.Timeout, requests.ConnectionError) as e:
            last_err = e

        time.sleep(backoff * (attempt + 1))

    raise FetchError(f"Failed to fetch {cid}: {last_err}")

def fetch_ipfs_json(cid: str) -> t.Any:
    raw = fetch_ipfs_content(cid)
    try:
        return json.loads(raw.decode("utf-8"))
    except Exception as e:
        raise FetchError(f"Failed to parse JSON for {cid}: {e}") from e

def verify_zk_proof(proof_json: dict) -> bool:
    """
    Fetch proof JSON from IPFS (Pinata gateway) and verify it using the Rust ZK service (/verify).
    Returns True if verification passes, False otherwise.
    """
    ZK_URL = os.environ.get("ZK_URL", "http://127.0.0.1:8000").rstrip("/")
    SETUP_SEED = int(os.environ.get("ZK_SETUP_SEED"))
    PROOF_SEED = int(os.environ.get("ZK_PROOF_SEED"))

    try:
        payload = {
            "proof": proof_json,
            "setup_seed": SETUP_SEED,
            "proof_seed": PROOF_SEED,
        }

        r = requests.post(f"{ZK_URL}/verify", json=payload, timeout=20)
        if r.status_code != 200:
            print(f"ZK verify failed HTTP {r.status_code}: {r.text[:200]}")
            return False

        data = r.json()
        if data.get("ok"):
            print(f"Proof verified successfully")
            return True
        else:
            print(f"Proof verification failed: {data.get('error')}")
            return False

    except Exception as e:
        print(f"ZK verify error for CID {cid}: {e}")
        return False


# ---------- Main ----------
def main():
    print(f"Watching app {APP_ID} for new ballots (indexer: {INDEXER_URL})")

    algorand = AlgorandClient.from_environment()

    private_key, verifier_addr = account.generate_account()
    signer = AccountTransactionSigner(private_key)
    print("Verifier address:", verifier_addr)

    dispenser = algorand.account.dispenser_from_environment()
    algorand.send.payment(
        PaymentParams(
            sender=dispenser.address,
            receiver=verifier_addr,
            amount=AlgoAmount(algo=3),
            signer=dispenser.signer,
        )
    )

    maybe_set_verifier(algorand, verifier_addr, signer)

    last_seen_hash: Dict[str, Optional[bytes]] = {}

    while True:
        try:
            addrs = get_opted_in_accounts(APP_ID)

            for voter_addr in addrs:
                local = get_local_state_for(voter_addr, APP_ID)
                ipfs_bytes: Optional[bytes] = local.get(LOCAL_STATE_PROOF_KEY)
                if not ipfs_bytes:
                    continue

                if last_seen_hash.get(voter_addr) == ipfs_bytes:
                    continue
                print(f"Found opted-in account: {voter_addr}")

                cid = ipfs_bytes.decode("utf-8")
                proof = fetch_ipfs_json(cid)
                proof_valid = verify_zk_proof(proof)
                
                if proof_valid:
                    digest = commitment_digest_u64(proof)
                    old_commitment_sum = get_app_global_uint(algorand, APP_ID, COMMITMENT_SUM_KEY)
                    new_commitment_sum = (old_commitment_sum + digest) % U64_MOD
                    verify_one_ballot(algorand, verifier_addr, voter_addr, new_commitment_sum, signer)
                    print(f"✅ Verified ballot for {voter_addr}")
                else:
                    print(f"❌ Invalid proof for {voter_addr}")

                last_seen_hash[voter_addr] = ipfs_bytes

            time.sleep(SLEEP_INTERVAL)

        except Exception as e:
            print("⚠️ Loop error:", e)
            time.sleep(SLEEP_INTERVAL)


if __name__ == "__main__":
    main()
