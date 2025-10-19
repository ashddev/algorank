// src/components/Home.tsx
import React, { useMemo, useState, useEffect } from 'react'
import ConnectWallet from './components/ConnectWallet'
import Register from './components/Register'
import CreateBallot from './components/CreateBallot'
import { uploadBallotPinata } from './utils/pinata'
import { useWallet } from "@txnlab/use-wallet-react"
import { AlgorandClient } from "@algorandfoundation/algokit-utils"
import { getAlgodConfigFromViteEnvironment, getIndexerConfigFromViteEnvironment } from './utils/network/getAlgoClientConfigs'
import { APP_SPEC } from "./contracts/Election"
import { useSnackbar } from 'notistack'
import { Button } from './components/ui/button'
import { generateProofParts } from './utils/zk'
import { ensureLocalnetFunds } from './utils/fundLocalnet'

const ELECTION_APP_ID = BigInt(1002)

const Home: React.FC = () => {
  type Phase = "register" | "voting"
  const [phase, setPhase] = useState<Phase>("register")
  const { transactionSigner, activeAddress, activeWalletAccounts } = useWallet()
  const { enqueueSnackbar } = useSnackbar()

  let utf8Encode = new TextEncoder();

  const setup_seed = 0
  const proof_seed = 10

  const algodConfig = getAlgodConfigFromViteEnvironment()
  const indexerConfig = getIndexerConfigFromViteEnvironment()

  const algorand = useMemo(() => {
    const c = AlgorandClient.fromConfig({ algodConfig, indexerConfig })
    if (transactionSigner) c.setDefaultSigner(transactionSigner)
    return c
  }, [algodConfig, indexerConfig, transactionSigner])

  const electionClient = useMemo(() => {
    if (!activeAddress) return undefined
    return algorand.client.getAppClientById({
      appSpec: APP_SPEC,
      appId: ELECTION_APP_ID,
    })
  }, [algorand, activeAddress])

  const isLocalnet = import.meta.env.VITE_ALGOD_NETWORK === 'localnet'
  useEffect(() => {
    if (!isLocalnet || !activeAddress) return
    ;(async () => {
      try {
        if (activeWalletAccounts?.length) {
          for (const a of activeWalletAccounts) {
            await ensureLocalnetFunds(algorand, a.address, { minAlgo: 5, topUpAlgo: 10 })
          }
        }
      } catch (e) {
        console.error('Auto-fund failed:', e)
      }
    })()
  }, [algorand, activeAddress])


  

  return (
    <div className="min-h-screen bg-teal-400 flex items-center">
      <div className="text-center rounded-lg p-6 bg-white mx-auto">

        {phase === "register" && (
          <div>
            <h1 className="text-4xl">
              Welcome to <div className="font-mono">algorank</div>
            </h1>
            <p className="py-6">
              This is the open source, confidential, ranked choice voting protocol on Algorand.
            </p>

            <div className='flex flex-col gap-2'>
              <ConnectWallet />
              {electionClient && <Register electionClient={electionClient} />}
              <Button onClick={() => setPhase("voting")} disabled={!electionClient}>
                Vote
              </Button>
            </div>
          </div>
        )}

        {phase === "voting" && electionClient && (
          <CreateBallot
            onSubmit={async (ranking) => {
              try {
                const proof = await generateProofParts({ ballot: ranking, setup_seed, proof_seed })
                const cid = await uploadBallotPinata(proof)
                const res = await electionClient.send
                  .call({
                    method: 'cast_ballot',
                    sender: activeAddress ?? undefined,
                    args: [utf8Encode.encode(cid)]
                  })
                enqueueSnackbar(`Ballot submitted! Tx: ${res.transaction.txID()}`, { variant: 'success' })
              } catch (e: any) {
                enqueueSnackbar(`Error calling the contract: ${e.message}`, { variant: 'error' })
              }
            }}
          />
        )}

        {phase === "voting" && !electionClient && (
          <div className="text-sm text-red-600">Connect wallet to vote.</div>
        )}

      </div>
    </div>
  )
}

export default Home
