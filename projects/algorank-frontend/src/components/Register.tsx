import { useState } from "react"
import { useSnackbar } from "notistack"
import { Button } from "@/components/ui/button"
import { Loader2 } from "lucide-react"
import { useWallet } from "@txnlab/use-wallet-react"
import { AppClient } from "@algorandfoundation/algokit-utils/types/app-client"

interface RegisterButtonProps {
  electionClient: AppClient
}

export default function RegisterButton({ electionClient }: RegisterButtonProps) {
  const [loading, setLoading] = useState(false)
  const { enqueueSnackbar } = useSnackbar()
  
    const { activeAddress , transactionSigner} = useWallet()

  const handleRegister = async () => {
    if (!activeAddress) {
      enqueueSnackbar("Connect a wallet first", { variant: "warning" })
      return
    }
    setLoading(true)
    try {
        await electionClient.send.bare.optIn({sender: activeAddress ?? undefined, signer: transactionSigner})
        enqueueSnackbar("Registered: Opt-in successful", { variant: "success" })
    } catch (e: any) {
        enqueueSnackbar(`Opt-in failed: ${e?.message ?? e}`, { variant: "error" })
    } finally {
        setLoading(false)
    }
  }

  return (
    <div>
        <Button onClick={handleRegister} disabled={loading}>
        {loading && <Loader2 className="h-4 w-4 animate-spin" />}
        {loading ? "Registering..." : "Register"}
        </Button>
    </div>
  )
}
