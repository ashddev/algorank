# verifier.py
import os, time, base64, json, typing as t
import requests

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

APP_ID = int(os.environ.get("APP_ID", "1015"))
INDEXER_URL = os.environ.get("INDEXER_URL", "http://localhost:8980")
SLEEP_INTERVAL = int(os.environ.get("SLEEP_INTERVAL", "5"))
LOCAL_STATE_PROOF_KEY = os.environ.get("LOCAL_STATE_PROOF_KEY", "ballot_ipfs")
PINATA_GATEWAY_BASE = os.environ.get("PINATA_GATEWAY_BASE", "https://gateway.pinata.cloud")
PINATA_GATEWAY_TOKEN = os.environ.get("PINATA_GATEWAY_TOKEN")

IDX = indexer.IndexerClient("", INDEXER_URL)


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

                # TODO: verify proof via your Rust verifier or bridge
                cid = ipfs_bytes.decode("utf-8")
                content = fetch_ipfs_json(cid)
                print(content)
                proof_valid = True

                if proof_valid:
                    new_commitment_sum = 123456
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
