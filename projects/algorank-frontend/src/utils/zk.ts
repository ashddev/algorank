// src/utils/zk.ts

export type ProofPartsB64 = {
  log2_n: number
  committed_ballot: string
  committed_permutation: string
  proof: string
}

export type GenerateInput = {
  ballot: number[]              // Vec<u32> on Rust side
  setup_seed: number | bigint   // u64
  proof_seed: number | bigint   // u64
}

type GenerateOut = {
  ok: boolean
  error?: string | null
  proof?: ProofPartsB64 | null
}

const ZK_URL = (import.meta.env.VITE_ZK_URL as string | undefined) ?? 'http://127.0.0.1:8000'
const ZK_TIMEOUT_MS = Number(import.meta.env.VITE_ZK_TIMEOUT_MS ?? 20000)

function withTimeout<T>(p: Promise<T>, ms: number, label = 'request'): Promise<T> {
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms)
    p.then(v => { clearTimeout(t); resolve(v) }, e => { clearTimeout(t); reject(e) })
  })
}

/** Call Rocket: POST /generate -> returns ProofPartsB64 */
export async function generateProofParts(input: GenerateInput): Promise<ProofPartsB64> {
  const url = `${ZK_URL.replace(/\/+$/, '')}/generate`
  const res = await withTimeout(
    fetch(url, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(input),
    }),
    ZK_TIMEOUT_MS,
    'zk generate'
  )

  if (!res.ok) {
    const text = await res.text().catch(() => '')
    throw new Error(`ZK /generate error ${res.status}: ${text || res.statusText}`)
  }

  const data = (await res.json()) as GenerateOut
  if (!data.ok || !data.proof) {
    throw new Error(`ZK /generate failed: ${data.error ?? 'unknown error'}`)
  }
  return data.proof
}

export function proofPartsToJson(proof: ProofPartsB64, pretty = false): string {
  return JSON.stringify(proof, null, pretty ? 2 : 0)
}

export function proofPartsToJsonBytes(proof: ProofPartsB64, pretty = false): Uint8Array {
  return new TextEncoder().encode(proofPartsToJson(proof, pretty))
}
