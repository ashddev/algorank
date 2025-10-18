import { useWallet } from '@txnlab/use-wallet-react'
import { useMemo } from 'react'
import { ellipseAddress } from '../utils/ellipseAddress'
import { getAlgodConfigFromViteEnvironment } from '../utils/network/getAlgoClientConfigs'

const Account = () => {
  const { activeAddress } = useWallet()
  const algoConfig = getAlgodConfigFromViteEnvironment()

  const networkName = useMemo(() => {
    const n = (algoConfig.network || '').trim()
    return (n === '' ? 'localnet' : n).toLowerCase()
  }, [algoConfig.network])

  return (
    <div className="space-y-1 text-sm">
      <div >
        <span className="font-medium">Address:</span>{' '}
        <span className="font-mono">
          {activeAddress ? ellipseAddress(activeAddress) : '—'}
        </span>
      </div>
      <div>
        <span className="font-medium">Network:</span>{' '}
        <span className="uppercase">{networkName}</span>
      </div>
    </div>
  )
}

export default Account
