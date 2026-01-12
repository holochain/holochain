# Holochain Data Model Design

## Overview

This document describes the data model used by Holochain for representing and persisting application data.

## Core Concepts

### Agent

An _Agent_ is any entity which can exercise the property of agency within a Holochain network. In practice, this refers to a participant in a distributed application who can perform actions, validate the actions of others, and participate in the peer-to-peer network.

Each agent is identified by an _AgentPubKey_, which is the public half of a cryptographic signing key pair. While called a "key", it functions as a unique identifier within the system. The agent signs any actions they take to demonstrate the origin and authenticity of those actions. 

### Action

An _Action_ is the fundamental unit of authorship in Holochain. Actions authored by a single agent form a hash chain (a.k.a "source chain") that provides an immutable, tamper-evident record of everything an agent has done. Actions come in several types, each of which carries semantic meaning.

### Entry

An _Entry_ is application data that can be stored in Holochain. Entries represent the actual content that applications work with, while actions are defined by the system and 
describe how the entry data should be interpreted.

### Record

A _Record_ is the combination of an _Action_, signed by the agent who authored it, together with its associated entry if applicable. Records are the primary unit of data that applications query and work with.

The _Entry_ part of a _Record_ exists in one of multiple states. An _Entry_ may be _present_ when the entry data is included with the _Action_. It may be _hidden_ when the _Entry_ exists but is marked as private and therefore not shared. Some action types don't permit an _Entry_ at all, in which case the _Entry_ is _not applicable_. Finally, an _Action_ may be stored without its entry by agents other than the author, resulting in a _not stored_ state.

### Chain DHT Operations

Represent chain data, along with a signature, being shared with the network. Each _Action_ generates a specific set of chain operations (such as _CreateRecord_, _AgentActivity_, _CreateEntry_, etc.) that enable different authorities to store and index the data for efficient queries.

### Warrant Operations

Represent evidence of invalid behavior by an agent. When an agent detects that another agent has violated validation rules or forked their chain, they can issue a warrant containing cryptographic proof of the violation. Warrants are routed to the violating agent's activity authority, creating a permanent, queryable record of the misbehavior.

### Location

A location is an unsigned 32-bit number that is derived from a hash. The exact algorithm is not important, as long as it produces a value that is well distributed and all peers are using the same algorithm.

## Actions taken by an agent

### Action types

There are 8 action types: _Dna_, _AgentValidationPkg_, _InitZomesComplete_, _Create_, _Update_, _Delete_, _CreateLink_, _DeleteLink_. The following sections describe their semantic meaning.

#### Genesis

When an agent first joins a DNA network, their source chain must be initialized through a process called _genesis_. Genesis creates the first three actions on every source chain.

The genesis actions are:

1. _Dna_ (sequence 0) - Records the DNA hash of the application this agent is joining.
2. _AgentValidationPkg_ (sequence 1) - Contains the agent's public key and optional membrane proof for network validation.
3. _Create_ (sequence 2) - Creates an _Agent_-typed entry, containing the agent's public key.

These three genesis actions are created automatically by the system during the genesis workflow. The _Dna_ and _AgentValidationPkg_ actions cannot be created through application code, only queried. The _Create_ action with an _Agent_-typed entry can be created by an application but has special meaning in this location.

The _Dna_ action declares what application data model and validation rules the agent is using. It is permitted to include other content when computing a DNA hash but it always includes the hash of the code for the data model and validation rules. The _AgentValidationPkg_ and _Create_ actions "announce" the agent to
the network and set up the expectation that all further actions on this chain will be signed by this agent.

#### Initialization

After genesis completes successfully, an application is allowed to perform initialization steps. This may optionally choose to create actions of other types, to be described below. Once initialization is complete, an _InitZomesComplete_ action is appended to signal that the agent is ready to participate in the network.

The _InitZomesComplete_ is required to guarantee that the application's initialization steps are only run once. This is because, to Holochain, initialization actions are
indistinguishable from agent actions taken at any other time.

#### Creating, updating and deleting data

The _Create_, _Update_, and _Delete_ actions provide create, update and delete semantics for entries in Holochain.

##### Create

The _Create_ action introduces new entry data. The type of that entry data defines how it should be interpreted. Entry types are described later.

##### Update

The _Update_ action declares that a new entry should semantically replace a previous entry. Updates do not modify the original _Entry_ or _Action_ — they create a new _Entry_ and an _Action_ that references the original _Create_ action.

Updates establish a relationship between the old and new versions of data. Applications can query for all updates to a given _Create_ action, allowing them to construct update histories or assign meaning to the "current" version of that data.

Holochain does not implicitly describe how applications should process updates.

##### Delete

The _Delete_ action marks an action as deleted by referencing it. Like updates, deletes do not remove data — they add metadata indicating that the entry should be considered deleted.

A _Delete_ action must reference either a _Create_ or an _Update_ action.

Holochain does not implicitly describe how applications should process deletes.

#### Linking data

The _CreateLink_ and _DeleteLink_ actions provide a mechanism for creating directed, typed relationships between entities. Links allow applications to build graph structures, indexes, and navigable relationships without embedding references directly in entry data.

##### _CreateLink_

The _CreateLink_ action creates a directed link from a base hash to a target hash. These hashes may be of any type, including hashes of actions, entries, agent public keys or even custom hashes. Links are stored on the agent's source chain as an action.

Links are grouped by base hash, so that all links from the same base hash, with the same type, are considered a set.

Links are typed by the application, and may have a tag which permits arbitrary content. The type and tag may be used to filter tags that belong to the same set.

##### _DeleteLink_

The _DeleteLink_ action marks a previously created link as deleted. Like entry deletes, link deletes do not remove the original _CreateLink_ action — they add metadata indicating the link should no longer be considered active.

Unlike for entries, Holochain should automatically consider _DeleteLink_s when fetching links, by filtering the active set to those which haven't been deleted. It should still be possible to retrieve the set of create and delete link actions so that the application can decide what the current set should be.

### Action properties

#### Common properties

All actions (except _Dna_) share four common properties that form the structure of the source chain:

| Property | Rust Type | Description |
|----------|-----------|-------------|
| _author_ | `AgentPubKey` | The public key of the agent who created this action. All actions on a chain must have the same author. |
| _timestamp_ | `Timestamp` | When the action was created in microseconds since UNIX epoch. Timestamps must be monotonically non-decreasing along the chain. |
| _action sequence_ | `u32` | The sequence number of this action in the chain, starting from 0. Sequence number is precisely 1 greater than that of the _previous action_ it points at. |
| _previous action_ | `ActionHash` | The hash of the previous action in the chain, creating the hash-chain structure. |

The _Dna_ action is special. It is always the first action on a chain, so it has no previous action and implicitly has sequence number 0. It does have an author and timestamp.

The following sections describe the additional properties that each action type has.

#### Dna

| Property | Rust Type | Description |
|----------|-----------|-------------|
| _hash_ | `DnaHash` | The hash of the DNA this chain is running. |

#### AgentValidationPkg

| Property | Rust Type | Description |
|----------|-----------|-------------|
| _membrane proof_ | `Option<MembraneProof>` | Optional proof required for network entry, if this DNA supports it. |

#### InitZomesComplete

No additional properties, this is a marker action.

#### Create

| Property | Rust Type | Description |
|----------|-----------|-------------|
| _entry type_ | `EntryType` | The type of entry being created, which are defined below. |
| _entry hash_ | `EntryHash` | Hash of the entry content. |

_App_ entry types must be uniquely identifiable within a DNA to enable correct deserialization. Each _App_ entry type is identified an index:

- _Zome index_: An 8-bit index identifying which integrity zome defines this entry type
- _Entry definition index_: An 8-bit index identifying which entry type within that zome

Together, these two indices form a unique key for each entry type in the DNA. This indexing strategy ensures that entry data can be correctly deserialized to its original type. The indices are stable identifiers — once assigned to an entry definition, they must remain consistent for the lifetime of the DNA.

#### Update

| Property | Rust Type | Description |
|----------|-----------|-------------|
| _updates address_ | `ActionHash` | Hash of the _Create_ action being updated. |
| _updates entry address_ | `EntryHash` | Hash of the entry being updated. |
| _entry type_ | `EntryType` | The type of the new entry. |
| _entry hash_ | `EntryHash` | Hash of the new entry content. |

The _updates address_ allows _Update_ actions to be grouped with the _Create_ action that they update.

#### Delete

| Property | Rust Type | Description |
|----------|-----------|-------------|
| _deletes address_ | `ActionHash` | Hash of the _Create_ or _Update_ action being deleted. |
| _deletes entry address_ | `EntryHash` | Hash of the entry being deleted. |

The _deletes address_ allows _Delete_ actions to be grouped with the _Create_ actions that they delete.

#### _CreateLink_

| Property | Rust Type | Description |
|----------|-----------|-------------|
| _base address_ | `AnyLinkableHash` | The base address of the link, its "from" address |
| _target address_ | `AnyLinkableHash` | The target address of the link, its "to" address. |
| _zome index_ | `ZomeIndex` | Which integrity zome defines this link type. <!-- TODO implementation leak, should be contained within a "link type" but the link type is user defined... seems like it could be merged together with a little care --> |
| _link type_ | `LinkType` | Application-defined link type identifier. |
| _tag_ | `LinkTag` | Arbitrary bytes for application-defined link metadata. |

Link tags have a maximum size of 1 KB (1,000 bytes). This limit is enforced during validation to ensure tags can be efficiently stored and used for filtering queries.

#### _DeleteLink_

| Property | Rust Type | Description |
|----------|-----------|-------------|
| _base address_ | `AnyLinkableHash` | The base from the _CreateLink_ being deleted. |
| _link add address_ | `ActionHash` | The address of the _CreateLink_ action being deleted. |

The _base address_ allows _DeleteLink_ actions to be grouped with _CreateLink_ actions from the same base. The link add address associates the _DeleteLink_ action with a specific _CreateLink_ action.

### Entry Types

Holochain defines four entry types that serve different purposes in the system. Entries represent the actual data content stored in a record.

#### Agent

The _Agent_ entry type contains an agent's public key. An action with this entry type is created automatically during genesis as the third action on every source chain. Further _Agent_ entries are permitted to be created, if that has some meaning assigned by the application.

##### App

The _App_ entry type represents serialized, application-defined data. This is the most common entry type and contains arbitrary data structured according to the application's needs.

Applications create _App_ entries using _Create_ actions and can update or delete them using _Update_ and _Delete_ actions. The content and structure of _App_ entries is entirely determined by the application.

_Size limitation:_ _App_ entries have a maximum size of 4 MB (4,000,000 bytes). This limit is enforced automatically by Holochain. Should an application need to store larger pieces of data, they may consider splitting it across multiple entries or storing references to external content.

##### CapGrant

The _CapGrant_ entry type is a system entry for granting capabilities to other agents. Capabilities allow fine-grained control over who can call specific zome functions.

A CapGrant entry contains:
- _Tag_: A string identifier for this grant.
- _Access_: The access level, one of _Unrestricted_, _Transferable_, or _Assigned_.
- _Functions_: The set of zome functions this grant permits.
- _Secret_: For _Transferable_ and _Assigned_ grants, a secret that must be provided.
- _Assignees_: For _Assigned_ grants, the list of authorized agent public keys.

Access levels:
- _Unrestricted_: Any agent can exercise this capability, without a secret
- _Transferable_: A secret must be provided, but any agent with the secret can use it.
- _Assigned_: Requires both a secret and authorization, defined by the calling agent being in the assignees list.

## Distributed Hash Table Operations (DHT Ops)

Holochain also has a distributed data model, of DHT Operations (ops) that form each DNA's Distributed Hash Table (DHT). These are what get shared on the network and validated. An op is one of two possible kinds, chain ops and warrant ops.

When an agent creates a record on their source chain, that data is then made available on the DNA's Distributed Hash Table (DHT) as DHT chain operations (ops). Holochain's primary data model of actions, entries and records is already content addressable, and the op model adds metadata to describe Holochain's sharding model. Sharding works by distributing data across peers in the network, with enough structure to enable efficient data retrieval.

### Op locations

A location is computed for each `AgentPubKey`. That location is the agent's location on the network. Each agent stores some number of data locations, starting from their location, up to the maximum size of a location.

| Op group | Op type | Authority | Location source |
|----------|---------|-----------|-----------------|
| Chain | _AgentActivity_ | Agent | Agent address |
| | _CreateRecord_ | Record | Action address |
| | _UpdateRecord_ | Record | Original action address |
| | _DeleteRecord_ | Record | Original action address |
| | _CreateEntry_ | Entry | Entry address |
| | _UpdateEntry_ | Entry | Original entry address |
| | _DeleteEntry_ | Entry | Original entry address |
| | _CreateLink_ | Link | Link base address |
| | _DeleteLink_ | Link | Deleted link's base |
| Warrant | _InvalidChainOp_ | Agent | Agent address of the agent who authored invalid data |
| | _ChainFork_ | Agent | Agent address of the agent who forked their chain |

The transformation from source chain actions to DHT operations serves several purposes:

**Selective distribution:** Different authorities receive only the data they need to perform their role. The entry authority receives the entry content, while the agent activity authority receives only the action without the entry.

**Enabling queries:** DHT operations establish the indices and metadata needed for efficient queries. Links are stored at their base address, updates are indexed at the original entry, and agent activity is indexed by agent public key.

### Chain ops

Each action type is mapped to a specific set of DHT operations. Records are mapped after committing to a source chain, but the mapping can be done at any later point, by any agent, and still be valid. The following table shows which operations are generated for each action:

| Action Type | Ops Produced |
|-------------|---------------------|
| **Dna** | _CreateRecord_, _AgentActivity_ |
| **AgentValidationPkg** | _CreateRecord_, _AgentActivity_ |
| **InitZomesComplete** | _CreateRecord_, _AgentActivity_ |
| **Create** | _CreateRecord_, _AgentActivity_, _CreateEntry_ |
| **Update** | _CreateRecord_, _AgentActivity_, _CreateEntry_, _UpdateRecord_, _UpdateEntry_ |
| **Delete** | _CreateRecord_, _AgentActivity_, _DeleteRecord_, _DeleteEntry_ |
| **CreateLink** | _CreateRecord_, _AgentActivity_, _CreateLink_ |
| **DeleteLink** | _CreateRecord_, _AgentActivity_, _DeleteLink_ |

Note that every action produces a:
- _CreateRecord_ op meaning that every action is content addressable by its own action hash, and
- _AgentActivity_, which means that every action is sent to the author's authorities.

Each chain op contains the source action, and a signature of that action by its author. Note that the entry data is not signed, but entries are always referenced by their hash in an action so the signature of the action covers the entry data as long as the entry hash is checked.

Whether an entry is included in an op depends on the type of action, content of the action and the type of op. As described for _Action_s and _Record_s above, some _Action_s do not have an associated entry and entries may also be hidden. The _AgentActivity_ op never carries an entry, and neither do the _DeleteRecord_, _DeleteEntry_, _CreateLink_ or _DeleteLink_ op types.

### Chain Operations

There are nine types of chain operations, each serving a specific purpose in the distributed data model:

#### AgentActivity

The _AgentActivity_ operation is sent to the agent activity authority (agents near the agent's public key) for every action an agent commits.

**Purpose:**
- Maintain a complete activity history for each agent.
- Enable queries for all actions by a specific agent.
- Allows chain forks to be detected by agent authorities.

#### CreateRecord

The _CreateRecord_ operation is sent to the action authority (agents near the action hash) to store the complete record.

**Purpose:**
- Store the record for this action.
- Enable queries for the record by its action hash.
- Collect changes to content (updates, deletes).

#### UpdateRecord

The _UpdateRecord_ operation is sent to the action authority when a record is updated.

**Purpose:**
- Index updates with the record being updated.
- Enable queries for all updates to a specific record.

#### DeleteRecord

The **DeleteRecord** operation is sent to the action authority when an action is deleted.

**Purpose:**
- Index deletions for the action being deleted.
- Enable queries for deletes to a specific record.

#### CreateEntry

The _CreateEntry_ operation is sent to the entry authority (agents near the entry hash) when a new entry is created.

**Purpose:**
- Store the entry content at its content-addressed location.
- Enable queries for the entry by its hash.

#### UpdateEntry

The **UpdateEntry** operation is sent to the entry authority when an entry is updated.

**Purpose:**
- Index updates for the entry being updated.
- Enable queries for all updates to a specific entry.

#### DeleteEntry

The **DeleteEntry** operation is sent to the entry authority when an action with an entry is deleted.

**Purpose:**
- Index deletions with the entry being deleted.
- Enable queries to discover if _Create_ or _Update_ entries have been deleted.

#### CreateLink

The _CreateLink_ operation is sent to the link base authority when a link is created.

**Purpose:**
- Index links grouped by their base.
- Enable efficient queries for all links from a specific base
- Provide the foundation for graph traversal and relationship queries

#### DeleteLink

The _DeleteLink_ operation is sent to the link base authority when a link is deleted.

**Purpose:**
- Index link deletions at the base.
- Enable queries to filter out deleted links.

### Warrant Operations

In addition to chain operations that represent valid data, the DHT also propagates **warrant operations** that represent evidence of invalid behavior. Warrants allow agents to share proof that another agent has violated the integrity rules of the application.

#### Purpose of Warrants

Warrants serve several critical functions in maintaining network integrity:

**Distributed validation accountability:** When an agent detects invalid data during validation, they can issue a warrant to inform other agents about the violation.

**Evidence-based claims:** Warrants contain validatable proof of the invalid behavior, allowing any agent to independently verify the claim.

**Authority notification:** Warrants are delivered to the agent activity authority of the agent who authored the invalid data, creating a permanent record accessible to all agents querying that agent's activity.

**Consensus without coordination:** Warrants enable the network to reach eventual consensus about invalid data without requiring synchronous agreement or central arbitration.

#### ChainIntegrityWarrant

The _ChainIntegrityWarrant_ is currently the only type of warrant operation. It provides evidence that an agent has violated chain integrity rules.

**Warrant contents:**
- **Proof**: The evidence of wrongdoing (one of two types below)
- **Author**: The agent who discovered the violation and issued the warrant
- **Timestamp**: When the warrant was issued
- **Warrantee**: The agent who committed the violation
- **Signature**: The warrant author's signature over the warrant

**Authority:** Agent public key of the warrantee (the agent who committed the violation)

**Types of chain integrity violations:**

1. **InvalidChainOp**
   - Contains: The invalid action (hash and signature), the action's author, and the chain op type that was being validated.
   - Proves: An agent authored an action that fails validation rules.
   - Example: An _Create_ action with an entry that contains an application-defined `user_name` field that is supposed to be restricted to 30 characters, but is actually 70.

2. **ChainFork**
   - Contains: Two actions with the same sequence number (both with hashes and signatures) and their author.
   - Proves: An agent created a fork in their source chain by committing two different actions at the same sequence position.
   - Example: Two different _Create_ actions both claiming to be sequence number 15.
