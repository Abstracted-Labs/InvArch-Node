# Rings Pallet

## Overview

The Rings pallet provides a cross-consensus message (XCM) abstraction layer for DAO Management, enabling them to manage assets effortlessly across multiple chains. It abstracts XCM complexities, facilitating easier handling of cross-chain transactions.

## Key Features

- **Maintenance Mode**: Chains can be put under maintenance, restricting certain operations to ensure system integrity during upgrades or when issues are detected.
- **Cross-chain Calls**: Enables sending XCM calls to other chains, allowing for a wide range of interactions.
- **Asset Transfers**: Supports transferring fungible assets between accounts across different chains.
- **Asset Bridging**: Facilitates the bridging of assets between chains, enhancing liquidity and asset interoperability.

## Traits Overview

The pallet utilizes traits to abstract chain and asset locations:

- [`ChainList`]: Provides an interface for referencing chains and retrieving their [`MultiLocation`] or main asset.
- [`ChainAssetsList`]: Offers an interface for referencing chain assets and obtaining their [`MultiLocation`] or parent chain.

## Dispatchable Functions

### `set_maintenance_status`

Sets the maintenance status of a chain. Requires `MaintenanceOrigin` authorization.

- `chain`: The chain to modify.
- `under_maintenance`: The desired maintenance status.

### `send_call`

Allows sending a XCM call to another chain. Can be initiated by a DAO.

- `destination`: The target chain.
- `weight`: The call's weight.
- `fee_asset`: The asset used for fee payment.
- `fee`: The fee amount.
- `call`: The call data.

### `transfer_assets`

Allows transfers of fungible assets to another account in the destination chain.  
**Requires asset and fee_asset to be located in the same chain**.

- `asset`: The asset to transfer.
- `amount`: The amount to transfer.
- `to`: The recipient account.
- `fee_asset`: The asset used for fee payment.
- `fee`: The fee amount.

### `bridge_assets`

Allows bridging of assets to another chain, with either the DAO account or a third-party account as the beneficiary.

- `asset`: The asset to bridge and its origin chain.
- `destination`: The destination chain.
- `fee`: The bridging fee.
- `amount`: The amount to bridge.
- `to`: Optional beneficiary account on the destination chain. (Defaults to the DAO account)

## Events

- `CallSent`: Emitted when a XCM call is sent to another chain.
- `AssetsTransferred`: Emitted when assets are transferred to another account on a different chain.
- `AssetsBridged`: Emitted when assets are bridged to another chain.
- `ChainMaintenanceStatusChanged`: Indicates a change in a chain's maintenance status.

## Errors

- `SendingFailed`: Emitted when sending a XCM message fails.
- `WeightTooHigh`: Emitted when the call's weight exceeds the maximum allowed.
- `FailedToCalculateXcmFee`: Emitted when calculating the XCM fee fails.
- `FailedToReanchorAsset`, `FailedToInvertLocation`: Errors related to asset reanchoring or location inversion.
- `DifferentChains`, `ChainUnderMaintenance`: Indicate issues with the target chain or maintenance status.

This pallet serves as a foundational component for building cross-chain solutions within the InvArch ecosystem, streamlining asset management and interoperability across diverse blockchain environments.