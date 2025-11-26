---
name: Release Checklist
about: Release preparation checklist
title: "[RELEASE]"
labels: ''
assignees: ''

---

```mermaid
flowchart TB
    %% Start flow
    Start{Start Upgrade}

    Start --> Binaries[binaries]
    Start --> WindTunnel[wind-tunnel]
    Start --> MatthmeBinaries[matthme/holochain-binaries]
    Start --> Holonix1[Holonix: nix, holochain]
    Start --> HcSpinRustUtils[hc-spin-rust-utils]

    %% binaries
    Binaries --> Complete

    %% wind-tunnel
    WindTunnel --> Complete

    %% matthme binaries
    MatthmeBinaries ------> Kangaroo[kangaroo-electron]

    %% hc-spin-rust-utils
    HcSpinRustUtils --> Kangaroo
    HcSpinRustUtils ---> HcSpin

    %% kangaroo
    Kangaroo --> AppToolsComplete

    %% holonix
    Holonix1 --> JSClient[holochain-client-js]
    Holonix1 --> HttpGw[http-gw]

    %% http-gw
    HttpGw --> Complete

    %% holochain-client-js
    JSClient --> Tryorama[tryorama]
    JSClient --> HcSpin[hc-spin]

    %% hc-spin
    HcSpin --> AppLibsComplete

    %% tryorama
    Tryorama --> AppLibsComplete

    %% app libraries complete
    AppLibsComplete{App Libraries Complete!}
    AppLibsComplete --> Scaffold[scaffolding]

    %% scaffold
    Scaffold --> Holonix2[Holonix: hc-spin, hc-scaffold, playground]
    
    %% holonix (again)
    Holonix2 --> AppToolsComplete{App Tooling Complete!}

    %% app tools complete
    AppToolsComplete --> DinoAdventure[dino-adventure]
    AppToolsComplete --> Documentation[Documentation: App Upgrade Guide, Compatibility Table, Developer Portal]

    %% dino-adventure
    DinoAdventure --> DinoKangaroo[dino-adventure-kangaroo]

    %% dino-adventure-kangaroo
    DinoKangaroo --> Complete

    %% documentation
    Documentation --> Complete

    Complete{Upgrade Complete!}

    %% styling 
    style Start fill:blue,color:white
    style Complete fill:green,color:white
    style AppLibsComplete fill:green,color:white
    style AppToolsComplete fill:green,color:white
```

## Task Assignments

Assign people to be responsible for each stage in the release flow by replacing `@` with GitHub handles.

Assign one person to be responsible for the process overall, by assigning them to the ticket.

### Stage 1

Assigned to @

- [ ] binaries
- [ ] wind-tunnel
- [ ] matthme/holochain-binaries
- [ ] Holonix: nix, holochain
- [ ] hc-spin-rust-utils

### Stage 2

Assigned to @

- [ ] kangaroo-electron
- [ ] holochain-client-js 

### Stage 3

Assigned to @

- [ ] hc-spin
- [ ] tryorama

**App Libraries Complete**

### Stage 4

Assigned to @

- [ ] scaffolding

### Stage 5

Assigned to @

- [ ] holonix

**App Tooling Complete**

### Stage 6

Assigned to @

- [ ] dino-adventure
- [ ] Documentation: App Upgrade Guide, Compatibility Table, Developer Portal

### Stage 7

Assigned to @

- [ ] dino-adventure-kangaroo

**Upgrade Complete**
