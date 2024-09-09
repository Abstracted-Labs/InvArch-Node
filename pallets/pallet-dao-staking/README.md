# DAO Staking Pallet

## Overview

The DAO Staking Pallet is a pallet designed to facilitate staking towards DAOs within a blockchain network. This pallet introduces a staking mechanism that allows two distinct sets of entities, namely DAOs and Stakers, to participate in the distribution of tokens from a predefined pot. The allocation of rewards is determined based on the amount staked by each entity and the total stake towards each DAO.

### DAOs

DAOs represent virtual accounts identified by unique IDs, which are responsible for registering themselves within the staking ecosystem. The primary role of DAOs is to attract Stakers to lock tokens in their favor. The rewards allocated to DAOs are proportional to the total amount staked towards them by Stakers. However, for a DAO to be eligible for rewards, it must have a total stake above a predefined threshold, thereby becoming `active`.

### Stakers

Stakers are individual accounts that engage in locking tokens in favor of a DAO. Unlike DAOs, Stakers receive a fraction of the rewards based on their own stake.

## Runtime Configuration Parameters

- `BlocksPerEra`: Defines the duration of an era in terms of block numbers.
- `RegisterDeposit`: Specifies the deposit amount required for DAO registration.
- `MaxStakersPerDao`: Limits the maximum number of Stakers that can simultaneously stake towards a single DAO.
- `MinimumStakingAmount`: Sets the minimum amount required for a Staker to participate in staking.
- `UnbondingPeriod`: Determines the period, in eras, required for unbonding staked tokens.
- `RewardRatio`: Establishes the distribution ratio of rewards between DAOs and Stakers.
- `StakeThresholdForActiveDao`: Sets the stake threshold required for a DAO to become `active`.

## Dispatchable Functions

- `register_dao`: Allows DAOs to register themselves in the system.
- `unregister_dao`: Enables DAOs to unregister from the system, initiating the unbonding period for Stakers.
- `change_dao_metadata`: Changes the metadata associated to a DAO.
- `stake`: Allows Stakers to lock tokens in favor of a DAO.
- `unstake`: Unstakes tokens previously staked to a DAO, starting the unbonding period.
- `withdraw_unstaked`: Allows Stakers to withdraw tokens that have completed the unbonding period.
- `staker_claim_rewards`: Allows Stakers to claim available rewards.
- `dao_claim_rewards`: Allows rewards to be claimed for DAOs.
- `halt_unhalt_pallet`: Allows Root to trigger a halt of the system, eras will stop counting and rewards won't be distributed.

## Events

The pallet emits events such as `Staked`, `Unstaked`, `DaoRegistered`, `DaoUnregistered`, and others to signal various operations and state changes within the staking ecosystem.

## Errors

Errors such as `StakingNothing`, `InsufficientBalance`, `MaxStakersReached`, and others are defined to handle exceptional scenarios encountered during pallet operations.

## Example Runtime Implementation

For an example runtime implementation that integrates this pallet, refer to [src/testing/mock.rs](./src/testing/mock.rs).