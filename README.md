<div align="center">
<img src="https://github.com/Abstracted-Labs/Brand-Assets/blob/main/branding/png/brand_colored_text_white.png">
</div>

<div align="Center">

<h2> DAO Infrastructure for the Polkadot Ecosystem </h2>

Built using [Rust](https://github.com/rust-lang/rust) & the [Polkadot SDK](https://github.com/paritytech/polkadot-sdk).<br>
<br>
[![Substrate version](https://img.shields.io/badge/Substrate-v3.0.0-E6007A?logo=Parity%20Substrate)](https://github.com/paritytech/substrate/releases/tag/v3.0.0)
[![Medium](https://img.shields.io/badge/Medium-InvArch-E6007A?logo=medium)](https://invarch.medium.com/)
[![License](https://img.shields.io/github/license/InvArch/InvArch?color=E6007A)](https://github.com/Abstracted-Labs/InvArch/blob/main/LICENSE)<br>
</div>

<!-- TOC -->

<!-- /TOC -->
---
<div align="Center">
 
<h3>InvArch Network Overview</h3>

InvArch is a public network governed by a global community, accessible to everyone, allowing<br>
people to collaborate, share ownership, & function as multichain organizations!<br>


| Protocol | Description |
| -- | ----- |
| `dao_manager` | Deploy DAOs controlled via fungible tokens, NFTs, or KYC (WIP). Operators can define Custom DAO governance configurations, retroactively adjust DAO members, and dynamically determine voting power & DAO permissions. Members can submit self-executing proposals to a DAO for all or some members to vote on whether to approve or reject the action(s). DAO accounts can hold a diverse treasury of assets, send/receive & bridge/transfer these tokens to other accounts, and execute transactions using these assets on accessible protocols. | WIP |
| `dao_staking` | Through a system funded by network inflation: 1) DAOs can register to the network to apply for network funds through a permissionless & community-driven process. 2) Network token holders can stake their tokens towards registered DAOs to signal which they would like to see be supported by the network. 3) Stakers receive the same rate of rewards regardless of which DAO(s) they stake towards; however, the amount of rewards DAOs receive is determined by their proportional share of support throughout the entire protocol & only after attaining a minimum amount of support. |
| `xvm` | A hybrid smart contract platform featuring support for both EVM & WASM smart contracts. This protocol supports both the Ethereum API & Polkadot API, in addition to various Web3 wallets from across the industry such as [MetaMask](https://metamask.io/), [Phantom](https://phantom.app/), [Coinbase](https://www.coinbase.com/wallet), [Talisman](https://www.talisman.xyz/), [SubWallet](https://www.subwallet.app/), and [Nova Wallet](https://novawallet.io/) - and more! |
| `governance` | Self-executing on-chain governance controlled by network token holders. |

</div>

## How to Contribute Code

Please send a [GitHub Pull Request to InvArch](https://github.com/Abstracted-Labs/InvArch/pull/new) with a clear list of what you've done (read more about [pull requests](http://help.github.com/pull-requests/)) & ensure all your commits are atomic (one feature per commit). Always write a clear log message for your commits. One-line messages are fine for small changes, but bigger changes should look like this:<br>

    $ git commit -m "A summary of the commit."
    > 
    > "A paragraph describing what changed and its impact."
    
Also, please make sure to update tests as appropriate.

### Non-Technical Contributions

If you haven't already, join the community in InvArch [Discord](https://discord.gg/invarch) and inquire about how you can get involved! Please be aware that any members who send spam, advertisements of topics unrelated to InvArch, or solicitation requests in the server will be removed and banned.

### Additional Resources
• [InvArch Developer Console](https://polkadot.js.org/apps/?rpc=wss%3A%2F%2Finvarch-rpc.dwellir.com#/explorer)<br>
• [Polkadot Parachain Template](https://github.com/paritytech/polkadot-sdk/tree/master/templates/parachain)<br>

### License

• [GPL](https://github.com/Abstracted-Labs/InvArch/blob/main/LICENSE)<br>

