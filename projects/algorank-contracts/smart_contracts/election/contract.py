from algopy import ARC4Contract, String, GlobalState, UInt64, Account, Bytes, LocalState, Txn
from algopy.arc4 import abimethod, baremethod


class Election(ARC4Contract):
    def __init__(self) -> None:
        # ---- Global storage
        self.commitment_sum = GlobalState(UInt64(0))
        self.verifier_pk = GlobalState(Account)

        # ---- Local storage
        self.ballot_ipfs = LocalState(Bytes)
        self.verified = LocalState(UInt64)
    
    @baremethod(allow_actions=["OptIn"])
    def register(self) -> None:
        pass

    @abimethod()
    def cast_ballot(self, ipfs_hash: Bytes) -> String:
        account = Txn.sender
        result, exists = self.verified.maybe(account)
        if (exists):
            return String("Ballot already sent!")

        self.ballot_ipfs[account] = ipfs_hash
        self.verified[account] = UInt64(0)
        return String("Ballot cast!")
        

    @abimethod()
    def verify_ballot(self, for_account: Account, new_commitment_sum: UInt64) -> String:
        result, exists = self.verified.maybe(for_account)
        if (not exists):
            return String("Account has not cast a ballot!")
        if (result == 1):
            return String("This ballot is already verified!")
        
        self.commitment_sum.value = new_commitment_sum
        self.verified[for_account] = UInt64(1)
        return String("Verified ballot!")