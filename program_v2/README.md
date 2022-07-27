# SOL DID Client 

A typescript client library for registering, manipulating and resolving DIDs
using the `did:sol` method.

## Disclaimer
With Version `^3.0.0`, the `@identity.com/sol-did-client` has been updated to use a new
authoritive program on the Solana blockchain (Address: `didso1...`). DIDs that have a
persisted state with the legacy program `idDa4...` remain valid unless they are transferred
to the new program. The `did:sol` resolver will resolve DIDs with the following priority:

1. Persisted DID on `didso1...`
2. Persisted DID on `idDa4...`
3. Non persisted DID (results in a generative DID).

The version `<3.0.0` of the `sol-did-client` will remain available [@identity.com/sol-did-client-legacy
](https://www.npmjs.com/package/@identity.com/sol-did-client-legacy), but technically a
resolution with the legacy program does no longer return an authoritive result. (Since it
ignores the state of the new program). Therefore, for DID resolution a library update is
strongly encouraged. The `legacy` did sol client is also exported in the new libary as `LegacyClient`.

Please see below on how to migrate your DIDs to the new program.

## Features
The `sol-did-client` library provides the following features:

1. A W3C [DID core spec (v1.0)](https://www.w3.org/TR/did-core/) compliant DID method and resolver operating on the Solana Blockchain.
2. TS Client and CLI for creating, manipulating, resolving `did:sol`.
3. Generic support for Verification Methods of any type and key length.
4. Native on-chain support for `Ed25519VerificationKey2018`, `EcdsaSecp256k1RecoveryMethod2020` and `EcdsaSecp256k1VerificationKey2019`. This means DID state changes can be performed by solely providing a valid secp256k1 signature to the program, though it still requires a permissionless proxy.
5. On-Chain nonce protection for replay protection.
6. Dynamic (perfect) Solana account resizing for any DID manipulation.
7. Permissionless instruction to migrate any `did:sol` state to the new authoritive program.
8. A web-service driver, compatible with [uniresolver.io](https://unresolver.io) and [uniregistrar.io](https://uniregistrar.io).
9. A [did-io](https://github.com/digitalbazaar/did-io) compatible driver.
10. Based on the versatile [anchor framework](https://github.com/coral-xyz/anchor).
11. Improved data model (`enum` for types and `bit-flags` for certain properties.)
12. Introduced `OWNERSHIP_PROOF` to indicate that a Verification Method key signature was verified on-chain.
13. Introduced `DID_DOC_HIDDEN` flag that hides a Verification Method from the DID resolution.
14. Account size can grow beyond transaction size limits, which is an improvement from the legacy program.


## Command line tool
The client library comes with a command line tool `sol` that resolves and manipulates
DIDs.

### Installation
```shell
yarn global add @identity.com/sol-did-client # or npm install -g @identity.com/sol-did-client
```
### Usage

#### Resolve a DID
```shell
sol did:sol:ygGfLvAyuRymPNv2fJDK1ZMpdy59m8cV5dak6A8uHKa # resolves a DID on mainnet-beta
...
sol did:sol:devnet:6fjuEFDTircJVNCQWYe4UHNfbYrU1a4sEr8FQ5w7d8Fx # resovles a DID on devnet
````


## Client library
### Installation
In TS/JS project:
```shell
yarn add @identity.com/sol-did-client # or npm install @identity.com/sol-did-client
```

### Usage - Setup and Resolution

#### Create a Service

[//]: # (TODO: This should be updated after Andrews branch makes it in)
```ts
  const authority = Keypair.generate();
  const cluster: ExtendedCluster = 'localnet';

  // create service for a did:sol:${authority.publicKey}
  const service = await DidSolService.build(
    authority.publicKey,
    cluster,
    new NodeWallet(authority)
  );
```
Note, a service downloads the IDL for the anchor program dynamically from chain. Therefore
if the program works with other DIDs on the same cluster, a new service should be created
from an existing one:

```ts
  const anotherAuthority = Keypair.generate();
  const anotherService = await service.build(anotherAuthority.publicKey, new NodeWallet(anotherAuthority))
```

### Resolving a DID:
```ts
  const didDoc = await service.resolve();
  console.log(JSON.stringify(didDoc, null, 2));
```

### Information About DID Resolution
`did:sol` DIDs are resolved in the following way:
1. `Generative` DIDs are DIDs that have no persisted DID data account. (e.g. every valid Solana account/wallet is in this state).
This will return a generative DID document where only the public key of the account is a valid Verification Method.
2. `Persisted` DIDs are DIDs that have a persisted DID data account. Here the DID document represents the state that is found
on-chain.


## Usage - `did:sol` Manipulation
The following are all instructions that can be executed against a DID.
When manipulating a DID one generally needs three authoritive elements:

1. An `authority`, a (native) Verification Method with `Capability Invocation` flag, that is allowed to manipulate the DID.
2. A `fee payer`, a Solana account that covers the cost of the transaction execution.
3. A `(rent) payer`, a Solana account that covers an (eventual) initialization or resizing of the DID data account.

Generally all these entities are required for a successful DID manipulation (NB: A `rent payer` is needed only if the DID account size 
needs to scale up, requiring additional rent). Often all these accounts are represented by the same account, but this is
in no way a requirement. For the example, this allows for the implementation of a permissionless proxy that just satisfies
`2.` and `3.` in order to submit an authority-signed instruction/transaction to the chain.

Generally a manipulative DID operation has the following from:

```ts
service.OPERATION(...params): DidSolServiceBuilder
```

where each operation return as a builder that configures certain aspects of how the operation is translated or
executed.

For example:
````ts
  await service
    .addVerificationMethod({
      fragment: 'eth-address',
      keyData: Buffer.from(ethAddress),
      methodType: VerificationMethodType.EcdsaSecp256k1RecoveryMethod2020,
      flags: VerificationMethodFlags.CapabilityInvocation | VerificationMethodFlags.Authentication,
    },
      nonAuthority.publicKey
    )
  .withAutomaticAlloc(nonAuthority.publicKey)
  .withPartialSigners(nonAuthority)
  .withSolWallet(feePayerWallet)
  .withEthSigner(authorityEthKey)
  .rpc();
````
adds a new Verification Method to the DID, sets the authority (`1`) as `nonAuthority`, which in this example is actually
NOT a valid authority. It also uses `nonAuthority` as a `rent payer` (`3`) via `withAutomaticAlloc` and uses
`solWallet` as a wallet interface signer of the transaction that covers the transaction fee (`2`). Lastly, the 
instruction is signed by `authorityEthKey` itself, which IS an actual authority (`1`) on the DID and permits the
update. Finally `rpc()` creates the instruction(s) and transaction, and then sends it to the chain. It is a terminal method
of the builder and needs to be awaited (returning a promise of the signature string).

Here is a breakdown of all exposed builder functions:

1. `withAutomaticAlloc(payer: PublicKey): DidSolServiceBuilder`: Automatically enables a perfect resize of the DID data account.
    If required, this will generate an additional `initialize` or `resize` instruction that is executed before the actual
    service intruction bringing the account to the required size.
2. `withEthSigner(ethSigner: EthSigner): DidSolServiceBuilder`: Sets an EthSigner that implements the following interface:
```ts
export type EthSigner = {
  publicKey: string;
  signMessage: (message: Bytes | string) => Promise<string>;
};
```
This signs all (supported) instructions with the provided signMessage interface, which needs to adhere to [EIP-191](https://eips.ethereum.org/EIPS/eip-191).If the DID contains a matching Verification Method of type `EcdsaSecp256k1RecoveryMethod2020` or `EcdsaSecp256k1VerificationKey2019`, with a capability invocation flag, no Solana Authority (`1`) is required.
3. `withConnection(connection: Connection): DidSolServiceBuilder`: Overrides the Solana connection used for `rpc()`.
4. `withConfirmOptions(confirmOptions: ConfirmOptions): DidSolServiceBuilder`: Overrides Solana's ConfirmationOptions used for `rpc()`.
5. `withSolWallet(solWallet: Wallet): DidSolServiceBuilder`: Overrides the Solana wallet interface with the following interface:
```ts
export interface Wallet {
  signTransaction(tx: Transaction): Promise<Transaction>;
  signAllTransactions(txs: Transaction[]): Promise<Transaction[]>;
  publicKey: PublicKey;
}
```
that is used to sign the transaction within `rpc()`.
6. `withPartialSigners(...signers: Signer[]): DidSolServiceBuilder`: Sets partialSigners to sign the transaction in `rpc()`.
7. `async rpc(opts?: ConfirmOptions): Promise<string>`: Terminal method that creates the instruction(s), builds and signs the transaction
    and sends it to the chain. Furthermore, it translates the chain-specific error code into a human-readable error message.
8. `async transaction(): Promise<Transaction>`: Terminal method that creates the instruction(s) and builds the transaction.
   (Eth signing will be applied if applicable, but no Solana Transaction handling is performed)
9. `async instructions(): Promise<TransactionInstruction[]>`: Terminal method that creates and returns the instruction(s) (if applicable, Eth signing will be used).


### Init a DID Account
Generally, all DID operations can be performed with `withAutomaticAlloc(payer: PublicKey)`, which automatically creates a
DID data account of the required size. However, the API still manually initializes a DID account of any size.
Allocating a sufficiently sized account upfront foregoes the use of any payers for subsequent operations.

```ts
  await service.initialize(10_000, payer.publicKey).rpc();
```
The `initialize` operation does not support  `withAutomaticAlloc` OR `withEthSigner`. Using `initialize` without an
argument sets the default DID authority as `payer` and sizes it to the minimal initial size required.

### Resize a DID Account
Generally all DID operations can be performed with `withAutomaticAlloc(payer: PublicKey)`, which automatically creates
or resizes a DID data account meeting size requirements. However, the API still supports manual resizing a DID account to any size.

```ts
  await service.resize(15_000, payer.publicKey).rpc();
```

### Add a Verification Method
This operation will add a new Verification Method to the DID. The `keyData` can be a generically sized `UInt8Array`, but
logically it must match the methodType specified.

```ts
    await service.addVerificationMethod({
      fragment: 'eth-address',
      keyData: Buffer.from(ethAddress),
      methodType: VerificationMethodType.EcdsaSecp256k1RecoveryMethod2020,
      flags: VerificationMethodFlags.CapabilityInvocation | VerificationMethodFlags.Authentication,
    })
  .withAutomaticAlloc(authority.publicKey)
  .rpc();
```

### Remove a Verification Method
Removes a Verification Method with the given `fragment` from the DID. Note, at least one valid Verification Method with
a `Capability Invocation` flag must remain to prevent lockout.

```ts
  await service
  .removeVerificationMethod('eth-address')
  .withAutomaticAlloc(authority.publicKey)
  .rpc();
```

### Set Flags of a Verification Method
This sets or updates the flag on an existing Verification Method. 

**Important** if the flag contains `VerificationMethodFlags.OwnershipProof`this transaction MUST use the same authority as the Verification Method (e.g. proving that the owner can sign with that specific Verification Method). `VerificationMethodFlags.OwnershipProof` is supported with the following `VerificationMethodTypes`:
- `Ed25519VerificationKey2018`
- `EcdsaSecp256k1RecoveryMethod2020`
- `EcdsaSecp256k1VerificationKey2019`

```ts
// Note in this example the 'default" VM must match authority.publicKey
  await service
    .setVerificationMethodFlags('default', 
      VerificationMethodFlags.CapabilityInvocation | VerificationMethodFlags.OwnershipProof, 
      authority.publicKey)
    .withAutomaticAlloc(authority.publicKey)
    .rpc();
```

### Add a Service
This operation sets a new service on a DID. `serviceType` are strings, not enums and can therefore be freely defined.

```ts
  await service
    .addService(
      {
        fragment: 'service-1',
        serviceType: 'service-type-1',
        serviceEndpoint: 'http://localhost:3000',
      })
    .rpc();
```

### Remove a Service
This operation removes a service with the given `fragment` name from the DID.

```ts
  await service.removeService('service-1')
    .withAutomaticAlloc(nonAuthority.publicKey)
    .rpc();
```

### Update the Controllers of a DID
This operation sets or updates the controllers of a DID. This overwrites any existing controllers.

```ts
  await service
    .setControllers([
      `did:sol:localnet:${Keypair.generate().publicKey.toBase58()}`,
      `did:ethr:${EthWallet.createRandom().address}`,
    ])
    .withAutomaticAlloc(authority.publicKey)
    .rpc();
```
Technically `did:sol` controllers are verified to be valid Solana account keys and stored accordingly, while all other
types of DID controllers persist as strings.

### Update All Properties of a DID
This operation allows bulk updates to all changeable properties of a DID. Please note, that this is a more destructive operation
and should be handled with care. Furthermore, by overwriting all Verification Methods it removes ANY existing `VerificationMethodFlags.OwnershipProof`,
which are not allowed to be specified within the bulk update.

```ts
    await service
      .update({
        controllerDIDs: [
          `did:sol:localnet:${Keypair.generate().publicKey.toBase58()}`,
          `did:ethr:${EthWallet.createRandom().address}`,
        ],
        services: [
          {
            fragment: 'service-1',
            serviceType: 'service-type-1',
            serviceEndpoint: 'http://localhost:3000',
          },
          {
            fragment: 'service-2',
            serviceType: 'service-type-2',
            serviceEndpoint: 'http://localhost:3001',
          }
        ],
        verificationMethods: [
          {
            fragment: 'default',
            keyData: authority.publicKey.toBytes(),
            methodType: VerificationMethodType.Ed25519VerificationKey2018,
            flags: VerificationMethodFlags.CapabilityInvocation | VerificationMethodFlags.Authentication,
          },
          {
            fragment: 'eth-address',
            keyData: Buffer.from(ethAddress),
            methodType: VerificationMethodType.EcdsaSecp256k1RecoveryMethod2020,
            flags: VerificationMethodFlags.CapabilityInvocation | VerificationMethodFlags.Authentication,
          }
        ],
      })
    .withAutomaticAlloc(authority.publicKey)
    .rpc();
```
Note, the `default` Verification Method can never be removed. Therefore not setting it within the update Method will
cause all flags to be removed from it.

### Close a DID Account
This transactions closes a DID account. With that it implicitly reverts to its generative state.

```ts
  await service.close(rentDestination.publicKey).rpc();
```

The rent for the DID data account will be return to `rentDestination`.

### Migrate a Persisted Legacy DID to the New Program
Legacy DIDs are resolved with the current `did:sol` resolver, however, if you want to migrate a legacy DID
(e.g. because you want to make use of all the new features), you can do so in the following way:

In order to migrate a DID, the DID needs a persisted DID data account in a legacy program, and cannot have a persisted DID data account in the new program (e.g. DID was not migrated already).

If no persisted state exists on either program it is a `generative` DID that does not need a migration.
The prerequisites can be checked with `await service.isMigratable()`

Actual migration is done the following way:
```ts
// check if DID in service can be migrated.
const canMigrate = await service.isMigratable()

if (canMigrate) {
  // migrate DID
  await service
    .migrate(nonAuthoritySigner.publicKey, legacyAuthority.publicKey)
    .withPartialSigners(nonAuthoritySigner)
    .withSolWallet(legacyAuthorityWallet)
    .rpc();
}
```
Note, that the migrate function works with a `nonAuthoritySigner` meaning ANYONE
can migrate any DID to the new program. But don't worry, since the migration keeps the state,
you can be happy if someone else does it for you.

In this example however, `nonAuthoritySigner` is the `rent payer` for the new account.

Furthermore, canMigrate takes an optional `legacyAuthority` argument. If specified, it closes the legacy DID account automatically and returns the rent to the new account `rent payer`(`nonAuthoritySigner` in this example). Since legacy DIDs are automatically allocated **a lot** of space, and newly migrated DIDs are optimally space efficient, migrating to a new DID can actually make you `SOL`back.


## Contributing

Note: Before contributing to this project, please check out the code of conduct and contributing guidelines.

Sol-DID uses [nvm](https://github.com/nvm-sh/nvm) and [yarn](https://yarnpkg.com/)

```shell
nvm i
yarn
```

## Running the tests

### E2E tests

Install Solana locally by following the steps described [here](https://docs.solana.com/cli/install-solana-cli-tools).
Also install Anchor by using the information found [here](https://book.anchor-lang.com/getting_started/installation.html)

```shell
anchor test
```

