Overview:
You're issuing a remittance stablecoin. It needs a protocol-level fee on every transfer (issuer revenue),
a way to force new accounts frozen until KYC clears, on-chain metadata so wallets don't have to trust
an off-chain registry, and the ability to close the mint if it's ever decommissioned.

Tasks:
1. Create a mint stacking TransferFeeConfig, MetadataPointer (pointed at the mint itself), DefaultAccountState (frozen),
and MintCloseAuthority, all sized correctly via ExtensionType::try_calculate_account_len,
with every extension-init instruction ordered before InitializeMint.

2. Write a transfer function that uses transfer_checked_with_fee, not transfer or transfer_checked,
and computes the expected fee via calculate_epoch_fee(current_epoch, amount) rather than a cached rate.

3. Read account/mint state exclusively through StateWithExtensions, never raw unpack.

4. Implement the unfreeze path: freeze authority thaws an individual account after "KYC,"
separate from any mint-level default-state change.


Now consider this scenario: Regulators require the issuer to be able to seize funds from sanctioned wallets. Users want transfer
amounts hidden from public view. Both requirements land on the same mint. (Since confidential transfers can't be added after creation, re-issue the mint carrying forward the same extension set, now adding confidentiality and a seizure authority, identify the gap between the two).

5. Re-issue the mint with PermanentDelegate (seizure authority) and confidential transfers enabled with approve_policy = manual.

6. Implement the full confidential lifecycle end-to-end: ConfigureAccount (owner-only, distinct from ATA creation-by-anyone),
DepositConfidentialTokens, ApplyPendingBalance, a confidential Transfer, and WithdrawConfidentialTokens,
including applying pending balance before withdrawal.

Extension challenge (optional - More advanced):
Build a minimal on-chain program that receives delegated authority via Approve and executes transfers on the user's
behalf (the agent/automation pattern). Have the user enable CPI Guard on their account afterward and confirm the
delegated-transfer path still works unmodified.

Written finding, not code: what happens if a sanctioned user moves their balance into the confidential
system before the permanent delegate acts?

Finding: once the user deposits into pending and applies into available confidential balance,
the funds become unseizable. PermanentDelegate moves only plaintext balances, while confidential
balances move only with owner-signed ZK proofs the delegate cannot forge. The delegate can still
freeze and seize the user's public balance, so seizure must win the race to action. Privacy, once
entered, is a one-way shield (auditor keys allow viewing, never seizing).