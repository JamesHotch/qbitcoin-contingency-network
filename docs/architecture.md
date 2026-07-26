# Architecture Diagrams

These diagrams describe intended protocol boundaries. Dashed or labeled
research edges may not yet be wired into the running multi-process node.

## Transaction Flow

```mermaid
flowchart LR
    U["Wallet owner selects UTXOs"] --> C["Construct deterministic transaction"]
    C --> S["Create Schnorr and ML-DSA signatures"]
    S --> L["Local policy and encoding check"]
    L --> R["Submit to full node REST or P2P"]
    R --> V["Node validates UTXO, amounts, weight, and both signatures"]
    V --> M["Admit to mempool"]
    M --> B["Miner includes transaction in candidate block"]
    B --> N["Every node validates again"]
```

## Full-Node Validation

```mermaid
flowchart TD
    I["Receive candidate block"] --> F{"Canonical header and body?"}
    F -- "No" --> X["Reject"]
    F -- "Yes" --> P{"Known valid parent?"}
    P -- "No" --> X
    P -- "Yes" --> W{"Target and SHA-256d work valid?"}
    W -- "No" --> X
    W -- "Yes" --> T{"Merkle root and limits valid?"}
    T -- "No" --> X
    T -- "Yes" --> U["Clone parent UTXO state"]
    U --> Q{"Coinbase, inputs, values, maturity, locks, and signatures valid?"}
    Q -- "No" --> X
    Q -- "Yes" --> S{"Supply invariant valid?"}
    S -- "No" --> X
    S -- "Yes" --> C["Index valid branch and cumulative chainwork"]
    C --> G{"Best valid work without crossing active checkpoint?"}
    G -- "No" --> H["Keep as side branch"]
    G -- "Yes" --> A["Activate branch and persist manifest"]
```

## Mining

```mermaid
flowchart LR
    N["Node validates mempool"] --> T["Block-template API"]
    T --> C["Miner builds coinbase and Merkle root"]
    C --> H["Header commits chain ID, version, target, and previous hash"]
    H --> P["CPU miner searches nonce with SHA-256d"]
    P -->|"> target"| P
    P -->|"<= target"| S["Submit complete block"]
    S --> V["Full nodes independently validate"]
    V --> F["Fees and valid subsidy become immature coinbase"]
```

## Competing-Chain Selection

```mermaid
flowchart TD
    A["Two branches share a valid ancestor"] --> B["Validate every block on branch B"]
    A --> C["Validate every block on branch C"]
    B --> D{"Both branches valid?"}
    C --> D
    D -- "No" --> R["Discard invalid branch regardless of work"]
    D -- "Yes" --> W{"Candidate cumulative work is greater?"}
    W -- "No" --> K["Keep current active chain"]
    W -- "Yes" --> G{"Would reorg replace finalized checkpoint?"}
    G -- "Yes, authority active" --> J["Reject activation"]
    G -- "No, or authority retired" --> E["Activate higher-work branch"]
```

## Checkpoint Quorum

```mermaid
flowchart LR
    H["Already-valid historical block"] --> P["Proposal commits chain, work, epoch, stage, time, and prior checkpoint"]
    P --> S0["Signer 0 validates independently"]
    P --> S1["Signer 1 validates independently"]
    P --> SX["Signers 2 through 10 validate independently"]
    S0 --> C["Collect individual ML-DSA signatures"]
    S1 --> C
    SX --> C
    C --> Q{"At least 7 unique valid signatures?"}
    Q -- "No" --> R["No checkpoint"]
    Q -- "Yes" --> N["Node revalidates proposal and active history"]
    N --> F["Install historical fork-choice guard"]
```

## Progressive Authority Reduction

```mermaid
stateDiagram-v2
    [*] --> BOOTSTRAP_FINALITY
    BOOTSTRAP_FINALITY --> REDUCED_FINALITY: "All conditions plus activation delay"
    REDUCED_FINALITY --> OBSERVER_ONLY: "All conditions plus activation delay"
    OBSERVER_ONLY --> RETIRED: "All conditions plus activation delay"
    RETIRED --> RETIRED: "Committee messages ignored"
    note right of BOOTSTRAP_FINALITY
      Delayed historical checkpoints
      7-of-11 development quorum
    end note
    note right of REDUCED_FINALITY
      Deeper and less frequent
    end note
    note right of OBSERVER_ONLY
      Signed observations
      Zero fork-choice power
    end note
    note right of RETIRED
      Proof of work only
      No reverse transition
    end note
```

## Irreversible Sunset

```mermaid
flowchart TD
    Q["Retirement qualification locks"] --> D["Deterministic activation delay"]
    D --> A["Activate RETIRED"]
    A --> C["Clear checkpoint guard"]
    A --> L["Set chainstate retirement latch"]
    A --> I["Certificate handler returns IgnoredRetired"]
    L --> R{"Attempt to reinstall authority"}
    R --> X["Reject with CheckpointAuthorityRetired"]
    X --> H["Only an explicit user-adopted hard fork could change the rule"]
```

## Post-Sunset Proof-of-Work Operation

```mermaid
flowchart LR
    M["Miners produce competing valid branches"] --> V["Nodes validate all consensus rules"]
    V --> W["Compute cumulative chainwork"]
    W --> B["Activate greatest-work valid branch"]
    C["Former committee sends certificate"] --> I["Ignore with zero consensus effect"]
    I -. "No edge to fork choice" .-> B
```

## Hybrid Signature Authorization

```mermaid
flowchart TD
    T["Chain-specific transaction digest"] --> S["Verify BIP340 Schnorr"]
    T --> M["Verify FIPS 204 ML-DSA-65"]
    S --> A{"Schnorr valid AND ML-DSA valid?"}
    M --> A
    A -- "Yes" --> V["Normal spend authorization valid"]
    A -- "No" --> R["Reject spend"]
    T --> L{"Recovery path committed and relative timelock mature?"}
    L -- "No" --> R
    L -- "Yes" --> H["Verify FIPS 205 SLH-DSA recovery signature"]
    H -- "Valid" --> E["Recovery authorization valid"]
    H -- "Invalid" --> R
```
