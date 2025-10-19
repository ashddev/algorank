import { useEffect, useState } from 'react'
import { useWallet, WalletId } from '@txnlab/use-wallet-react'
import { Button } from './ui/button'

type DevAccount = { address: string; name?: string }

export function KmdAccountSwitcher() {
  const {
    activeAddress,
    activeWallet,
    activeWalletAccounts,
  } = useWallet()

  const isKmdActive = activeWallet?.id === WalletId.KMD
  const [accounts, setAccounts] = useState<DevAccount[]>([])
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    let cancelled = false
    const load = async () => {
      if (!isKmdActive) {
        setAccounts([])
        return
      }
      setLoading(true)
      try {
        // Prefer built-in list if available
        if (activeWalletAccounts && activeWalletAccounts.length) {
          if (!cancelled) {
            setAccounts(
              activeWalletAccounts.map(a => ({ address: a.address, name: (a as any).name }))
            )
          }
          return
        }
        // Fallback: ask the provider directly
        if (activeWallet && typeof (activeWallet as any).getAccounts === 'function') {
          const fetched = await (activeWallet as any).getAccounts()
          if (!cancelled) setAccounts(fetched)
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    load()
    return () => { cancelled = true }
  }, [isKmdActive, activeWallet, activeWalletAccounts])

  const handleSelect = async (addr: string) => {
    if (!activeWallet) return
    // If the provider supports programmatic switching:
    if (typeof (activeWallet as any).setActiveAccount === 'function') {
      await (activeWallet as any).setActiveAccount(addr)
      return
    }
    // Otherwise, reconnect to open the provider’s selector UI
    await activeWallet.connect()
  }

  if (!isKmdActive) return null

  return (
    <div className="mt-4 space-y-2">
      <div className="text-sm text-muted-foreground">KMD Accounts</div>

      {loading && <div className="text-sm">Loading accounts…</div>}

      {!loading && !accounts.length && (
        <div className="text-sm text-muted-foreground">
          No accounts found. Ensure your KMD wallet has accounts (via Lora/Lute) and you’re connected.
        </div>
      )}

      {!loading && accounts.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {accounts.map((acct) => {
            const isActive = acct.address === activeAddress
            const label =
              acct.name ||
              `${acct.address.slice(0, 6)}…${acct.address.slice(-4)}`
            return (
              <Button
                key={acct.address}
                variant={isActive ? 'default' : 'outline'}
                onClick={() => handleSelect(acct.address)}
                title={acct.address}
              >
                {label}
                {isActive ? ' · Active' : ''}
              </Button>
            )
          })}
        </div>
      )}
    </div>
  )
}
