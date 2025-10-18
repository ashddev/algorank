import { useWallet, Wallet, WalletId } from '@txnlab/use-wallet-react'
import Account from './Account'
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Button } from './ui/button'
import { useState } from 'react'
import { Spinner } from './ui/spinner'

const ConnectWallet = () => {
  const { wallets, activeAddress } = useWallet()
  const [connecting, setConnecting] = useState(false)
  const [connectingWalletId, setConnectingWalletId] = useState<WalletId | null>(null)

  const handleConnect = async (wallet: Wallet) => {
    setConnecting(true)
    setConnectingWalletId(wallet.id)
    try {
      await wallet.connect()
    } catch (err) {
      console.error(err)
    } finally {
      setConnecting(false)
      setConnectingWalletId(null)
    }
  }

  const handleLogout = async () => {
    if (!wallets) return
    const activeWallet = wallets.find((w) => w.isActive)
    if (activeWallet) {
      await activeWallet.disconnect()
    } else {
      localStorage.removeItem('@txnlab/use-wallet:v3')
      window.location.reload()
    }
  }

  const isKmd = (wallet: Wallet) => wallet.id === WalletId.KMD

  return (

    <Dialog>
      <DialogTrigger><Button>Connect wallet</Button></DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Select wallet provider</DialogTitle>
          <DialogDescription>
            Choose a wallet to connect.
          </DialogDescription>
        </DialogHeader>
        <div className="grid">
          {activeAddress && (<Account />)}
          {!activeAddress &&
            wallets?.map((wallet) => {
              const isThisConnecting = connecting && connectingWalletId === wallet.id
              return (
              <Button
                data-test-id={`${wallet.id}-connect`}
                variant="outline"
                key={`provider-${wallet.id}`}
                disabled={connecting && !isThisConnecting}
                onClick={() => handleConnect(wallet)}
              >
                {!isKmd(wallet) && (
                  <img
                    alt={`wallet_icon_${wallet.id}`}
                    src={wallet.metadata.icon}
                    style={{ objectFit: 'contain', width: '30px', height: 'auto' }}
                  />
                )}
                <span>{isKmd(wallet) ? 'LocalNet Wallet' : wallet.metadata.name}</span>
                {isThisConnecting && <Spinner />}
              </Button>
            )})}
        </div>
        <DialogFooter className="sm:justify-start w-full">
          <div className='w-full flex gap-4'>
            <DialogClose asChild>
              <Button type="button" variant="secondary">
                Close
              </Button>
            </DialogClose>

            <Button
              variant="destructive"
              disabled={!activeAddress || connecting}
              onClick={handleLogout}
            >
              Logout
            </Button>
          </div>
        </DialogFooter>

      </DialogContent>
    </Dialog >
  )
}
export default ConnectWallet
