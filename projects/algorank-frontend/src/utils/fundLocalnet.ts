// src/utils/fundLocalnet.ts
import { AlgorandClient } from '@algorandfoundation/algokit-utils'
import { AlgoAmount } from '@algorandfoundation/algokit-utils/types/amount';


export async function ensureLocalnetFunds(
  algorand: AlgorandClient,
  addr: string,
  { minAlgo = 5, topUpAlgo = 10 }: { minAlgo?: number; topUpAlgo?: number } = {}
) {
  const info = await algorand.client.algod.accountInformation(addr).do()
  const balanceMicro = info.amount as unknown as number
  const minMicro = AlgoAmount.Algos(minAlgo).microAlgos

  if (balanceMicro >= minMicro) return

  const dispenser = await algorand.account.localNetDispenser()

  await algorand.send.payment({
    sender: dispenser.addr,
    receiver: addr,
    amount: AlgoAmount.Algos(topUpAlgo),
    signer: dispenser.signer,
  })
}
