# Fuzzing Engine

## Design

### Input 

- The fuzzing input is a sequence of message calls (executed at the initial state (may be the empty state or a forked state)). 
    - Message call denotes an invocation to contracts being fuzzing tested.

- There are two types of message calls, performed by two different kinds of accounts/contracts (see below), respectively. 

### Two kinds of contracts/accounts:
- Subjects: the contracts to be fuzz tested, (those we'd like to find bugs in)
- Outsiders: the contracts/accounts that call the subject contracts. (Usually, composers are attackers in the fuzzing)
    - Composers can either be 
        - an EOA, which initiates the top-level call to the subject contracts, or
        - an attacker-controlled contract, which are called by the subject contract (via callback) and may call the subject contracts nestedly.

### Corpus

Corpus is a set of inputs (message call sequence). 
Each message call contains a pointer pointing to the pre-execution state and post-execution state. 
(We will have a pool of states).

### Mutation

The mutation of inputs will be possible on the entire message call sequence (in contrast to ItyFuzz's last-transaction mutation).

