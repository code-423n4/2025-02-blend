# Blend V2 Audit + Certora Formal Verification details
- Total Audit Prize Pool: $125,000 in USDC
  - HM awards: $73,500 in USDC
  - QA awards: $2,800 in USDC
  - Judge awards: $5,000 in USDC
  - Validator awards: $3,200 in USDC 
  - Scout awards: $500 in USDC
  - Mitigation Review: $20,000 in USDC
  - Formal Verification: $20,000 in USDC
    - Real Bug Rules: $4,000 in USDC
    - Coverage Rules: $14,000 in USDC
    - Participation Rules: $2,000 in USDC
- [Read our guidelines for more details](https://docs.code4rena.com/roles/wardens)
- Starts February 24, 2025 20:00 UTC
- Ends March 17, 2025 20:00 UTC

ℹ️ This audit includes **deployed code,** and [the "live criticals" exception](https://docs.code4rena.com/awarding/incentive-model-and-awards#the-live-criticals-exception) therefore applies. Please see the section titled "Live/Deployed Code" for details.

**Note re: risk level upgrades/downgrades**

Two important notes about judging phase risk adjustments: 
- High- or Medium-risk submissions downgraded to Low-risk (QA) will be ineligible for awards.
- Upgrading a Low-risk finding from a QA report to a Medium- or High-risk finding is not supported.

As such, wardens are encouraged to select the appropriate risk level carefully during the submission phase.

## Live/Deployed Code

For the purposes of the ["live criticals" exception](https://docs.code4rena.com/awarding/incentive-model-and-awards#the-live-criticals-exception) and [sensitive disclosure process](https://docs.code4rena.com/roles/wardens/submission-guidelines#how-to-submit-zero-day-or-otherwise-highly-sensitive-bugs), contracts in the following directories should be considered live code, as they have only changed incrementally from the V1 code, which is deployed: 
- `./pool/*`
- `./pool-factory/*`
- `./backstop/*`

All other code in scope is _not_ deployed. Vulnerabilities with a root cause in any part of the codebase _except_ the above-listed directories should therefore be submitted via the standard submission form/process.

## Automated Findings / Publicly Known Issues

The 4naly3er report can be found [here](https://github.com/code-423n4/2025-02-blend/blob/main/4naly3er-report.md).

_Note for C4 wardens: Anything included in this `Automated Findings / Publicly Known Issues` section is considered a publicly known issue and is ineligible for awards._

Any uncovered issues will be logged here: https://github.com/blend-capital/blend-contracts-v2/issues

✅ SCOUTS: Please format the response above 👆 so its not a wall of text and its readable.

# Overview

[ ⭐️ SPONSORS: add info here ]

- See the [Formal Verification Readme]() for FV details.  [C4 TODO]

## Links

- **Previous audits:**  https://github.com/blend-capital/blend-contracts/tree/main/audits for v1 audits.

v2 audit is in progress, no report to share at this time
  - ✅ SCOUTS: If there are multiple report links, please format them in a list.
- **Documentation:** https://docs.blend.capital/
- **Website:** https://www.blend.capital/
- **X/Twitter:** https://x.com/blend_capital
- **Discord:** https://discord.com/invite/a6CDBQQcjW

---

# Scope

[ ✅ SCOUTS: add scoping and technical details here ]

### Files in scope
- ✅ This should be completed using the `metrics.md` file
- ✅ Last row of the table should be Total: SLOC
- ✅ SCOUTS: Have the sponsor review and and confirm in text the details in the section titled "Scoping Q amp; A"

*For sponsors that don't use the scoping tool: list all files in scope in the table below (along with hyperlinks) -- and feel free to add notes to emphasize areas of focus.*

| Contract | SLOC | Purpose | Libraries used |  
| ----------- | ----------- | ----------- | ----------- |
| [contracts/folder/sample.sol](https://github.com/code-423n4/repo-name/blob/contracts/folder/sample.sol) | 123 | This contract does XYZ | [`@openzeppelin/*`](https://openzeppelin.com/contracts/) |

### Files out of scope
✅ SCOUTS: List files/directories out of scope

## Scoping Q &amp; A

### General questions
### Are there any ERC20's in scope?: Yes

✅ SCOUTS: If the answer above 👆 is "Yes", please add the tokens below 👇 to the table. Otherwise, update the column with "None".

Any (all possible ERC20s)


### Are there any ERC777's in scope?: No

✅ SCOUTS: If the answer above 👆 is "Yes", please add the tokens below 👇 to the table. Otherwise, update the column with "None".



### Are there any ERC721's in scope?: No

✅ SCOUTS: If the answer above 👆 is "Yes", please add the tokens below 👇 to the table. Otherwise, update the column with "None".



### Are there any ERC1155's in scope?: No

✅ SCOUTS: If the answer above 👆 is "Yes", please add the tokens below 👇 to the table. Otherwise, update the column with "None".



✅ SCOUTS: Once done populating the table below, please remove all the Q/A data above.

| Question                                | Answer                       |
| --------------------------------------- | ---------------------------- |
| ERC20 used by the protocol              |       🖊️             |
| Test coverage                           | ✅ SCOUTS: Please populate this after running the test coverage command                          |
| ERC721 used  by the protocol            |            🖊️              |
| ERC777 used by the protocol             |           🖊️                |
| ERC1155 used by the protocol            |              🖊️            |
| Chains the protocol will be deployed on | OtherStellar  |

### ERC20 token behaviors in scope

| Question                                                                                                                                                   | Answer |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| [Missing return values](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#missing-return-values)                                                      |   In scope  |
| [Fee on transfer](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#fee-on-transfer)                                                                  |  Out of scope  |
| [Balance changes outside of transfers](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#balance-modifications-outside-of-transfers-rebasingairdrops) | In scope    |
| [Upgradeability](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#upgradable-tokens)                                                                 |   In scope  |
| [Flash minting](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#flash-mintable-tokens)                                                              | In scope    |
| [Pausability](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#pausable-tokens)                                                                      | In scope    |
| [Approval race protections](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#approval-race-protections)                                              | In scope    |
| [Revert on approval to zero address](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#revert-on-approval-to-zero-address)                            | Out of scope    |
| [Revert on zero value approvals](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#revert-on-zero-value-approvals)                                    | Out of scope    |
| [Revert on zero value transfers](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#revert-on-zero-value-transfers)                                    | In scope    |
| [Revert on transfer to the zero address](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#revert-on-transfer-to-the-zero-address)                    | In scope    |
| [Revert on large approvals and/or transfers](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#revert-on-large-approvals--transfers)                  | In scope    |
| [Doesn't revert on failure](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#no-revert-on-failure)                                                   |  In scope   |
| [Multiple token addresses](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#revert-on-zero-value-transfers)                                          | In scope    |
| [Low decimals ( < 6)](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#low-decimals)                                                                 |   In scope  |
| [High decimals ( > 18)](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#high-decimals)                                                              | Out of scope    |
| [Blocklists](https://github.com/d-xo/weird-erc20?tab=readme-ov-file#tokens-with-blocklists)                                                                | In scope    |

### External integrations (e.g., Uniswap) behavior in scope:


| Question                                                  | Answer |
| --------------------------------------------------------- | ------ |
| Enabling/disabling fees (e.g. Blur disables/enables fees) | Yes   |
| Pausability (e.g. Uniswap pool gets paused)               |  Yes   |
| Upgradeability (e.g. Uniswap gets upgraded)               |   No  |


### EIP compliance checklist
N/A

✅ SCOUTS: Please format the response above 👆 using the template below👇

| Question                                | Answer                       |
| --------------------------------------- | ---------------------------- |
| src/Token.sol                           | ERC20, ERC721                |
| src/NFT.sol                             | ERC721                       |


# Additional context

## Main invariants

* User's cannot extract funds (borrow, withdraw) from a pool if they do not meet or exceed the minimum health factor

✅ SCOUTS: Please format the response above 👆 so its not a wall of text and its readable.

## Attack ideas (where to focus for bugs)
## Auctions

The protocol conducts auctions to both process liquidations and pay out backstop interest. Anything that can disrupt / block / break the creation or filling of these auctions has potential to impact the health of a pool and/or it's backstop.

The auction creation system was modified in v2 to allow auction creators to specify the assets in the bid/lot. https://github.com/blend-capital/blend-contracts-v2/issues/3

## Flash Loans

Blend v2 introduces a flash loans endpoint. These are slightly different than EVM based flash loans, but still allow external contracts to be invoked. with borrowed funds, and repaid afterwards, or remain borrowed with the appropriate amount of collateral.

✅ SCOUTS: Please format the response above 👆 so its not a wall of text and its readable.

## All trusted roles in the protocol

Pool
* admin - can add reserves, edit reserve configs, and edit pool config (EXCL. ORACLE)

Backstop
* N/A

Emitter
* N/A

Pool Factory
* N/A

✅ SCOUTS: Please format the response above 👆 using the template below👇

| Role                                | Description                       |
| --------------------------------------- | ---------------------------- |
| Owner                          | Has superpowers                |
| Administrator                             | Can change fees                       |

## Describe any novel or unique curve logic or mathematical models implemented in the contracts:

* Interest uses a capped integral controller to adjust the interest rates to help utilization reach the target utilization rate: https://docs.blend.capital/blend-whitepaper#interest-rates



✅ SCOUTS: Please format the response above 👆 so its not a wall of text and its readable.

## Running tests

1. Setup Soroban: https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup
2. Checkout: https://github.com/blend-capital/blend-contracts-v2
3. Run `make` to install cargo dependencies and build contracts
4. Run `make test` to execute test suite

✅ SCOUTS: Please format the response above 👆 using the template below👇

```bash
git clone https://github.com/code-423n4/2023-08-arbitrum
git submodule update --init --recursive
cd governance
foundryup
make install
make build
make sc-election-test
```
To run code coverage
```bash
make coverage
```
To run gas benchmarks
```bash
make gas
```

✅ SCOUTS: Add a screenshot of your terminal showing the gas report
✅ SCOUTS: Add a screenshot of your terminal showing the test coverage

## Miscellaneous
Employees of Blend and employees' family members are ineligible to participate in this audit.

Code4rena's rules cannot be overridden by the contents of this README. In case of doubt, please check with C4 staff.





# Scope

*See [scope.txt](https://github.com/code-423n4/2025-02-blend/blob/main/scope.txt)*

### Files in scope


| File   | Logic Contracts | Interfaces | nSLOC | Purpose | Libraries used |
| ------ | --------------- | ---------- | ----- | -----   | ------------ |
| /backstop/src/lib.rs | ****| **** | 16 | ||
| /backstop/src/testutils.rs | ****| **** | 183 | ||
| /mocks/mock-pool-factory/src/lib.rs | ****| **** | 9 | ||
| /mocks/moderc3156/src/lib.rs | ****| **** | 15 | ||
| /pool-factory/src/lib.rs | ****| **** | 11 | ||
| /pool-factory/src/test.rs | ****| **** | 282 | ||
| /pool/src/lib.rs | ****| **** | 25 | ||
| /pool/src/testutils.rs | ****| **** | 231 | ||
| **Totals** | **** | **** | **772** | | |

### Files out of scope

*See [out_of_scope.txt](https://github.com/code-423n4/2025-02-blend/blob/main/out_of_scope.txt)*

| File         |
| ------------ |
| ./test-suites/fuzz/fuzz_targets/fuzz_pool_general.rs |
| ./test-suites/fuzz/lib.rs |
| ./test-suites/src/assertions.rs |
| ./test-suites/src/backstop.rs |
| ./test-suites/src/emitter.rs |
| ./test-suites/src/lib.rs |
| ./test-suites/src/liquidity_pool.rs |
| ./test-suites/src/moderc3156.rs |
| ./test-suites/src/oracle.rs |
| ./test-suites/src/pool.rs |
| ./test-suites/src/pool_factory.rs |
| ./test-suites/src/setup.rs |
| ./test-suites/src/snapshot.rs |
| ./test-suites/src/test_fixture.rs |
| ./test-suites/src/token.rs |
| ./test-suites/tests/test_backstop.rs |
| ./test-suites/tests/test_backstop_inflation_attack.rs |
| ./test-suites/tests/test_backstop_rz_changes.rs |
| ./test-suites/tests/test_backstop_swap.rs |
| ./test-suites/tests/test_flashloan.rs |
| ./test-suites/tests/test_liquidation.rs |
| ./test-suites/tests/test_overflow_flag.rs |
| ./test-suites/tests/test_pool.rs |
| ./test-suites/tests/test_pool_inflation_attack.rs |
| ./test-suites/tests/test_pool_interest.rs |
| ./test-suites/tests/test_wasm_happy_path.rs |
| Totals: 26 |

