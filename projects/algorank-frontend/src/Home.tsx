// src/components/Home.tsx
import React, { useMemo, useState, useEffect } from 'react'
import ConnectWallet from './components/ConnectWallet'
import Register from './components/Register'
import CreateBallot from './components/CreateBallot'
import Tally from './components/Tally'
import { uploadBallotPinata } from './utils/pinata'
import { useWallet } from "@txnlab/use-wallet-react"
import { AlgorandClient } from "@algorandfoundation/algokit-utils"
import { getAlgodConfigFromViteEnvironment, getIndexerConfigFromViteEnvironment } from './utils/network/getAlgoClientConfigs'
import { APP_SPEC } from "./contracts/Election"
import { useSnackbar } from 'notistack'
import { Button } from './components/ui/button'

const ELECTION_APP_ID = BigInt(1002)

const Home: React.FC = () => {
  type Phase = "register" | "voting" | "tally"
  const [phase, setPhase] = useState<Phase>("register")
  const { transactionSigner, activeAddress } = useWallet()
  const { enqueueSnackbar } = useSnackbar()

  let utf8Encode = new TextEncoder();

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
      defaultSender: activeAddress,
    })
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
                const cid = await uploadBallotPinata({ranking})
                const res = await electionClient.send
                  .call({
                    method: 'cast_ballot',
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

        {phase === "tally" && (<Tally />)}
      </div>
    </div>
  )
}

export default Home
