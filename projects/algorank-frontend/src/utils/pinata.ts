
const jwt = import.meta.env.VITE_PINATA_JWT ?? ""

export async function uploadBallotPinata(ballot: unknown): Promise<string> {
  const res = await fetch('https://api.pinata.cloud/pinning/pinJSONToIPFS', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${jwt}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({ pinataContent: ballot })
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`Pinata error ${res.status}: ${text}`);
  }
  const data = await res.json() as { IpfsHash: string };
  return data.IpfsHash
}
