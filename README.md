# DepositCW20.W22TWT.Nahem

## CW20 Coding Challenge

1. Anytime CW20 tokens are deposited, add a new expiration of 20 blocks higher than the current block height.
2. If a user tries to withdraw CW20 tokens before expiration, return an error.
3. Write a custom error for it.
4. Complete the deposit 20 and withdraw test, use `cw-multi-test` to advance the block height to make sure the expiration is working properly.
