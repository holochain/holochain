
System Correctness: Confidence
==============================

In the frame of the Byzantine Generals Problem, the correctness of a
distributed coordination system is analyzed through the lens of "fault
tolerance". In our frame we take on a broader scope and address the
question of the many kinds of confidence necessary for a system's adoption
and continued use. We identify and address the following dimensions
of confidence:

1.  **Fault Tolerance:** the system's resilience to external
  perturbations, both malicious and natural. Intrinsic integrity.

2.  **Completeness/Fit:** the system's *a priori* design elements that
  demonstrate fitness for purpose. We demonstrate this by describing
  how Holochain addresses multi-agent reality binding, scalability,
  and shared-state finality.

3.  **Security:** the system's ability to cope with intentional disruption by
  malicious action, beyond mere detection of faults.

4.  **Evolvability:** the system's inherent architectural affordances
  for increasing confidence over time, especially based on data from
  failures of confidence in the above dimensions.

Our claim is that if all of these dimensions are sufficiently addressed,
then the system takes on the properties of anti-fragility; that is, it
becomes more resilient and coherent in the presence of perturbations
rather than less.

Fault Tolerance
---------------

In distributed systems much has been written about Fault Tolerance
especially to those faults known as "Byzantine" faults. These faults
might be caused by either random chance or by malicious action. For
aspects of failures in system confidence that arise purely from
malicious action, see the [section on Security](#security).

1.  **Faults from unknown data provenance:** Because all data
  transmitted in the system is generated and cryptographically signed by Agents,
  and those signatures are also
  included in the hash-chains, it is always possible to verify any
  datum's provenance. Thus, faults from intentional or accidental
  impostors is not possible. The system cannot prevent
  malicious or incautious actors from stealing or revealing private
  keys, however, although it does include affordances to deal with
  these eventualities. These are discussed under Completeness/Fit.

2.  **Faults from data corruptibility in transmission and storage:**
  Because all state data is stored along with a cryptographic hash
  of that data, and because all data is addressed and retrieved by
  that hash and can be compared against the hash, the only
  possible fault is that the corruption resulted in data that has
  the same hash. For SHA256 hashing (which is what we use), this is
  known to be a vanishingly small possibility for both intentional
  and unintentional data corruption.[^corruption] Furthermore, because all
  data is stored as hash-chains, it is not possible for portions of
  data to be retroactively changed. Agents' Source Chains thus
  become immutable append-only event logs.

  One possible malicious act that an Agent can take is to roll back
  their chain to some point and start publishing different data from
  that point forward. But because the publishing protocol requires
  Agents to also publish all of their Actions to the neighborhood of their own public
  key, any Actions that lead to a forked chain will be easily and
  immediately detected by simply detecting more than one action
  linked to the same previous action.

  It is also possible to unintentionally rollback one's chain.
  Imagine a setting where a hard-drive corruption leads to a restore
  from an outdated backup. If a user starts adding to their
  chain from that state, it will appear as a rollback and fork to
  validators.

  Holochain adds an affordance for such situations in which a
  good-faith actor can add a Record repudiating such an
  unintentional chain fork.

3.  **Faults from temporal indeterminacy:** In general these faults do
  not apply to the system described here because it only relies on
  temporality where it is known that one can rely on it; i.e., when
  recording Actions that take place locally as experienced by an
  Agent. As these temporally recorded Actions are shared into the
  space in which nodes may receive messages in an unpredictable
  order, the system still guarantees eventual consistency (though
  not uniform global state) because of the intrinsic integrity of
  recorded hash-chains and deterministic validation.
  Additionally, see the section on "Multi-agent reality binding (Countersigning)"
  for more details on how some of the use cases addressed by
  consensus systems are handled in this system.


[^corruption]: CITATION NEEDED

Completeness/Fit
----------------

1.  **Multi-agent reality binding (Countersigning)**

The addition of the single feature of Countersigning to Holochain
enables our eventually consistent framework to provide most of the
consensus assurances people seek from decentralized systems.
Countersigning provides the capacity for specific groups of agents to
mutually sign a single state-change on all their respective
source-chains. It makes the deterministic validity of a single Entry
require the cryptographic signatures of multiple agents instead of just
one. Furthermore any slow-downs necessary to add coordinated
countersigned entries are not just localized to the DNA involved,
they are also localized to just the parties involved. The same parties can
continue to interact in other DNAs.

The following are common use cases for countersigning; for a detailed
technical specification, please see the [Countersigning Spec (Appendix B)](hwp_B_countersigning_spec.md).

a.  **Multi-Agent State Changes:** Some applications require changes
  that affect multiple agents simultaneously. Consider the transfer
  of a deed or tracking a chain of custody, where Alice transfers
  ownership or custody of something to Bob and they want to produce an
  **atomic change across both of their source chains**. We must be
  able to prevent indeterminate states like Alice committing a
  change releasing an item without Bob having taken possession yet,
  or Bob committing an entry acknowledging possession while Alice's
  release fails to commit. Holochain provides a countersigning
  process for multiple agents to momentarily lock their chains while
  they negotiate one matching entry that each one commits to their
  chain. An entry which has roles for multiple signers requires
  signed chain Actions from each counterparty to enter the
  validation process. This ensures no party's state changes unless
  every party's state changes.

b.  **Cryptocurrencies Based on P2P Accounting:** Extending the previous
  example, if Alice wants to transfer 100 units of a currency to
  Bob, they can both sign a single entry where Alice is in the
  spender role, and Bob the receiver. This provides similar
  guarantees as familiar double-entry accounting, ensuring changes
  happen to both accounts simultaneously. Someone's balance can be easily computed
  by replaying the transactions on their source
  chain, and both signing parties can be held accountable for any
  fraudulent transfers that break the data integrity rules of the
  currency application. There's no need for global time of
  transactions when each is clearly ordered by its sequence in the
  chains of the only accounts affected by the change.

c.  **Witnessed Authoritative Sequence:** Some applications may require
  an authoritative sequence of changes to a specific data type.
  Consider changes to membership of a group of administrators, where
  Carol and David are both members of the group, and Carol commits a
  change which removes David from the group, and David commits a change which
  removes Carol. With no global time clock to trust,
  whose change wins? An application can set up a small pool of N
  witnesses and configure any change to be the result of a countersigning session
  that requires M optional witnesses (where M > 50% of N). Whichever action the
  witnesses sign first would prevent the other action from being
  signed, because either Carol or David would have been successfully
  removed and would no longer be authorized participate in a countersigning session to remove the other.

d.  **Exclusive Control of Rivalrous Data:** Another common need for an authoritative time sequence involves determining control of rivalrous data such as name registrations. Using M of N signing from a witness pool makes it easy to require witnessing for only rivalrous data types, and forgo the overhead of witnessing for all other data. For example, a Twitter-like app would not need
  witnessing for tweets, follows, unfollows, likes, replies, etc,
  only for registration of new usernames and for name changes. This
  preserves the freedom for low-overhead and easy scaling by not
  forcing consensus to be managed on non-rivalrous data (which
  typically comprises the majority of the data in web apps).

e.  **Generalized Micro-Consensus: Entwined multi-agent state
  change:** Even though Holochain is agent-centric and designed to
  make only local state changes, the countersigning process may be
  seen as an implementation of Byzantine consensus applied to
  specific data elements or situations. Contextual countersigning is
  exactly what circumvents the need for global consensus in
  Holochain applications.

```{=html}
<!-- -->
```
1.  **Scaling:** Holochain's architecture is specifically designed to maintain resilience and performance as both the number of users and interactions increase. Key factors contributing to its scaling capabilities include:
  a. **Agent-centric approach:** Unlike traditional blockchain systems, which require global consensus before progressing, Holochain adopts an agent-centric approach where changes made to an agent's state become authoritative once stored on their chain, signed, and communicated to others via the DHT. As a result, agents are able to initiate actions without delay and in parallel to other agents initiating their own actions.
  b. **Bottleneck-Free Sharded DHT:** Holochain's DHT is sharded, meaning that each node only stores a fraction of the total data, reducing the storage and computational requirements for each participant. At the same time, the storage of content with agents whose public key is "near" the hash of each Action or Entry, in combination with the use of Linking metadata attached to such hashes, transforms the DHT into a graphing DHT in which data discovery is simple in spite of the sparseness of the address space. When the agents responsible for validating a particular state change receive an authoring agent's proposed state change, they are able to a) request information from others in the DHT regarding the prior state of the authoring agent (where relevant), and b) make use of their own copy of the app's validation rules to deterministically validate the change.
  While that agent and its validating peers are engaged with the creation and validation of a particular change to the state of the authors chain, in parallel, other agents are able to author state changes to their own chain and have these validated by the validating peers for each of those changes.  This bottle-neck free architecture allows users to continue interacting with the system without waiting for global agreement.
  With singular actions by any particular agent (and the validation of those actions by a small number of other agents) able to occur simultaneous with singular actions by other agents as well as countersigned actions by particular groups of agents. The network is not updating state globally (as blockchains typically do) but is instead creating, validating, storing and serving changes of the state of particular agents in parallel.
  c. Multiple networks: In Holochain, each application (DNA) operates on its own independent network, effectively isolating the performance of individual apps. This prevents a high-traffic, data-heavy, or processing-heavy app from affecting the performance of other lighter apps within the ecosystem. Participants are able to decide for themselves which applications they want to participate in.
  d. Order of Complexity: "Big O" notation is usually only applied to local computation based on handling `n` number of inputs. However, we may consider a new type of O-notation for decentralized systems which includes two inputs, `n` as the number transactions/inputs/actions, and `m` as the number of nodes/peers/agents/users, as a way of expressing the time complexity for both an individual node and for the aggregate power of the entire network of nodes. Most blockchains are some variant of $\mathcal{O}(n^2*m)$ in their order of complexity. Every node must gossip and validate all state changes. However, Holochain retains a constant $\mathcal{O}(\frac{log(n)}{m})$ complexity for any network larger than a given size $R$, where $R$ is the sharding threshold. As the number of nodes in the network grows, each node performs a static workload irrespective of network size; or expressed inversely, a smaller portion of the total network workload.

1.  **Shared-state Finality:** Many blockchains approximate chain
  finality by assuming that the "longest chain wins." That strategy
  does not translate well to agent-centric chains, which are simply
  histories of an agent's actions. While there is no concern about
  forking global state because a Holochain app doesn't have one, we
  can imagine a situation where Alice and Bob have countersigned a
  transaction, then Alice forks her source chain by later
  committing an Action to an earlier sequence position in her chain.
  If the timestamp of this new, conflicting Action precedes the
  timestamp of the transaction with Bob, it could appear that Bob had knowingly participated in
  a transaction with a malicious actor, putting his own integrity in question. This can even happen
  non-maliciously when someone suffers data loss and restores from a
  backup after having made changes that were not included in the
  backup. While the initial beta version of Holochain does not offer
  fork finality protections for source chains, later versions will
  incorporate "meta-data hardening" which enables gossiping peers
  to tentatively solidify a state of affairs when they see that gossip
  for a time window has calmed and neighbors have converged on the
  same state. After this settling period (which might be set to
  something between 5 to 15 minutes) any later changes which would
  produce a conflict (such as forking a chain) can be rejected, preserving the legitimacy of state changes that were made in good faith.

Security
--------

The system's resilience to intentional gaming and disruption by
malicious actors will be covered in depth in future papers, but here we
provide an overview.

Many factors contribute to a system's ability to live up to
the varying safety and security requirements of its users. In general,
the approach taken in Holochain is to provide affordances that take into
account the many types of real-world costs that result from adding
security and safety to systems such that application developers can
match the trade-offs of those costs to their application context. The
integrity guarantees listed in the formal system description detail the
fundamental data safety that Holochain applications provide. Some other
important facets of system security and safety come from:

1.  Gating access to functions that change local state, for which Holochain
  provides a unified and flexible Object Capabilities model

2.  Detecting and blocking participation of bad actors, including attempts to flood a DHT with otherwise valid data, for which
  Holochain provides the affordances of validation and warranting.

3.  Protection from attack categories

4.  Resilience to human error

### Gating Access via Cryptographic Object Capabilities

To use a Holochain application, end-users must trigger Zome Calls that
effect local state changes on their Source Chains. Additionally, Zome
Functions can make calls to other Zome Functions on remote nodes in the
same app, or to other DNAs running on the same Conductor. All of these
calls must happen in the context of some kind of permissioning system.
Holochain's security model for calls is based on the
Object-capability[^object_capability] security model, but augmented for a distributed
cryptographic context in which we use cryptographic signatures to prove
the necessary agency for taking action.

[^object_capability]: https://en.wikipedia.org/wiki/Object-capability\_model

Access is thus mediated by Capability Grants of four types:

-   Author: only the agent owning the source change can make the zome
  call

-   Assigned: only the specified public key holders can make the zome call, as verified by a signature on the function call payload

-   Transferrable: anybody with the given secret can make the zome call

-   Unrestricted: anybody can make the zome call (no secret nor proof of
  authorized key needed to use this capability)

All zome calls must be signed and also take a required capability claim
parameter that MUST be checked by the system for making the call. Agents
record capability grants on their source chains and distribute their corresponding secrets as
applicable according to the application's needs. Receivers of secrets can
record them as private capability claim entries on their chains for
later lookup and use. The "agent" type grant is just the agent's public key.

### Validation & Warranting

We have already covered how Holochain's agent-centric validation and
intrinsic data integrity provides security from malicious actors trying
to introduce invalid or incorrect information into an Application's
network, as every agent can deterministically verify data and thus
secure itself. It is also important, however, to be able to eject
malicious actors from network participation who generate or propagate invalid data, so as to proactively secure the network against the resource drain that future such actions from those actors may incur.

As agents publish their actions to the DHT, other agents serve as validators. When validation passes, they send a validation receipt back to the authoring agent, so they know the network has seen and stored their data. When validation fails, they send a negative validation receipt, known as a warrant, back to the author and their neighbors so the system can propagate these provably invalid attempted
actions. This also flags the offending agent as corrupted or malicious so that other nodes can block them and stop interacting with the offending agent. Every node can confirm the warrant for themselves, as it is justified by the shared deterministic validation rules, of which all agents have a copy.

This enables a dynamic whereby any single honest agent can detect and report any invalid actions. So instead of needing a majority consensus to establish reliability of data (an "N/2 of N" trust model), Holochain enables "one good apple to heal the bunch" with a "1 of N" trust model for any data you acquire from agents on the network.

For even stricter situations, apps can achieve a "0 of N" trust model, where no external agents need to be trusted, because nodes can always validate data for themselves, independent of what any other nodes say.

### Security from Attack Categories

#### Consensus​ ​Attacks

This whole category of attack starts from the assumption that consensus
is required for distributed systems. Because Holochain doesn't start
from that assumption, the attack category really doesn't apply, but it's
worth mentioning because there​ ​are​ ​a​ ​number​ ​of​ ​attacks​ ​on​
​blockchain​ ​which​ threaten confidence in the reliability of the chain data through
collusion between some majority of nodes. ​The​ ​usual
thinking​ ​is​ ​that​ ​it​ ​takes​ ​a​ ​large​ ​number​ ​of​ ​nodes
​and​ ​massive​ ​amounts​ ​of​ ​computing​ ​power or financial
incentives​ ​to prevent​ ​undue​ ​hijacking​ ​of​ ​consensus.​
​However,​ ​since​ ​Holochain's data coherence doesn't derive from all
nodes awaiting consensus,​ ​but​ rather ​on​ deterministic
validation, nobody​ ​ever​ ​needs​ ​to​ ​trust​ ​a​ ​consensus​
​lottery.​ ​

#### Sybil Attacks

Since Holochain does not rely on any kind of majority consensus, it is already less vulnerable to Sybil Attacks, the creation of many fake colluding accounts which are typically used to overwhelm consensus of honest agents. And since Holochain enables "1 of N" and even "0 of N" trust models, Sybils cannot entirely overwhem honest agents' ability to determine the validity of data.

Additionally, since Holochain is a heterogeneous environment in which every app operates on its own isolated network, a Sybil Attack can only be attempted on a single app's network at a time. For each app, an appropriate membrane can be defined on a spectrum from very open and permissive to closed and strict by defining validation rules on a Membrane Proof.

A membrane proof is passed in during the installation process of an agent's instance of the app, so that the proof can be committed to the agent's chain just ahead of their public key. An agent's public key acts as their address in that application's DHT network, and is created during the genesis process in order to join the network. Other agents can confirm whether an agent may join by validating the membership proof.

A large variety of membrane proofs is possible, ranging from none at all, loose social triangulation, or an invitation from any current user, to stricter invitation lists, proof-of-work requirements, or a kind of proof-of-stake showing the agent possesses and has staked some value which they lose if their account gets warranted.

We generally suggest that applications may want to enforce some kind of membrane against Sybils, not because consensus or data integrity are at risk but because carrying a lot of Sybils makes unnecessary work for honest agents running an application. We cover more about this in the next section.

<!-- [WP-TODO v2: talk about spamming attacks and weighing] -->

#### Denial-of-Service Attacks

Holochain is not systemically subject to denial-of-service attacks
because there is no central point to attack. Because each application is
its own network, attackers would have to flood every agent of every
application to carry out a systemic denial-of-service attack; to do
that would require knowing who all those agents are, which is also not
recorded in one single place. One point of vulnerability is the bootstrap servers
for an application. But this is not a systemic vulnerability, as each
application can designate its own bootstrap server, and they can also
be arbitrarily hardened against denial-of-service to suit the needs of
the application.

#### Eclipse Attacks

\[WP-TODO: ACB REVIEW\]
An Eclipse Attack is an attack in which an honest node's immediate peers are all dishonest, blocking or modifying communication between the honest node and the larger network. This attack is specific to gossip-based peer-to-peer networks such as Bitcoin, Holochain, and DHTs like IPFS. While this attack can never be fully prevented, it can be mitigated. As an example, Bitcoin nodes only connect to [one peer per /16 IP block](https://en.bitcoin.it/wiki/Weaknesses#Sybil_attack).

Holochain reduces the impact of Eclipse Attacks by providing basic, built-in guarantees of data integrity. Each piece of data carries its author's signature, so adversaries can never tamper with others' data.

However, intrinsic data integrity merely protects the integrity of data which can be seen. Even though Holochain can guarantee that data hasn't been tampered with, adversaries in an Eclipse Attack could still make life miserable for an honest node by blocking the transmission of data. In general, Holochain's approach to the complexities of distributed computing is to provide affordances and capabilities that can be scaled appropriately according to what is appropriate for the specific use case of the application in question. Thus we provide some basic capacities at the base layer and assume that individual applications will also add hardening appropriate to their context. Built-in mitigation strategies include:

- Provide a bootstrap server that provides a large number of randomly chosen peers to which a node can connect.
- Avoid connecting with too many peers in a certain IP block, as with Bitcoin.
- Allow DNAs to specify known peers that can act as ‘harbour pilots' so that a node's introduction to a new DHT is facilitated. This process can be extended to take advantage of existing human, or digital, trust, and reputation factors.
- Nodes can ask their initial peers for assurances of trust based on reputation or identity verification.

Application developers can take steps to further protect their users:

- Implement a Membrane Proof appropriate for the social context in which the application will be used. An Eclipse Attack is more likely to be successful with a high number of Sybils in a network.
- Design validation rules requiring high-stakes actions to carry proof of their author's reputation, ideally by referring to data outside of the DHT. This doesn't prevent an Eclipse Attack, but it does give an honest node the power to detect suspicious peers and reject data originating from them.


### Human​ ​Error

There are some aspects of security, especially those of human error,
that all systems are subject to. People​ ​will​ ​still​ ​lose​ ​their​
​keys,​ ​use​ ​weak​ ​passwords,​ ​get​ ​computer​ ​viruses, etc.​ ​​
But, crucially, in the realm of "System Correctness" and "confidence,"​
the question that needs addressing is how the system interfaces with
mechanisms to mitigate against human error. Holochain provides
significant tooling to support key management in the form of its ​core​
​Distributed​ ​Public Key​ ​Infrastructure (DPKI) and DeepKey app built
on that infrastructure. Among other things, this tooling ​provides​
​assistance​ ​in​ ​managing​ ​keys,​ ​managing​ ​revocation​ ​methods,​
​and reclaiming​ ​control​ ​of​ ​applications​ ​when​ ​keys​ ​or​
​devices​ ​have​ ​become​ ​compromised. \[WP-TODO: ACB\] \[Need to be able
to refer to external docs on DeepKey and DPKI\]

Evolvability
------------

For large-scale systems to work well over time, we contend that specific
architectural elements and affordances make a significant difference in
their capacity to evolve while maintaining overall coherence as they do
so:

1.  **Subsidiarity:** From the Wikipedia definition: "*Subsidiarity is a
    principle of social organization that holds that social and
    political issues should be dealt with at the most immediate (or
    local) level that is consistent with their resolution.*"
    Subsidiarity enhances evolvability because it insulates the whole
    system from too much change, while simultaneously allowing change
    where it is needed. Architecturally, however, subsidiarity is not
    easy to implement because it is rarely immediately obvious what
    level of any system is consistent with an issue's resolution.

    In Holochain, the principle of subsidiarity is embodied in many ways,
    but crucially in the architecture of app instances having fully
    separate DNAs running on their own separate networks, each also
    having clear and differentiable Integrity and Coordination
    specifications. This creates very clear loci of change, both at
    the level of when the integrity rules of a DNA need to change, and
    at the level of how one interacts with a DNA. This allows
    applications to evolve exactly in the necessary area by updating
    only the DNA and DNA portion necessary for changing the specific
    functionality that needs evolving.

2.  **Grammatic composability:** Highly evolvable systems are built of
    grammatic elements that compose well with each other both
    "horizontally", which is the building of a vocabulary that fills
    out a given grammar, and "vertically" which is the creation of new
    grammars out of expressions of a lower level grammar. There is
    much more that can be said about grammatics and evolvability, but
    that is out of scope for this paper. However, we contend that the
    system as described above lives up to these criteria of having
    powerful grammatical elements that compose well as described. DNAs
    are essentially API definitions that can be used to create a large
    array of micro-services that can be assembled into small
    applications. Applications themselves can be assembled at the User
    Interface level. A number of frameworks in the Holochain ecosystem
    are already building off of this deep capacity for
    evolvability that is built into the system's architecture[^evolvability].

3.  **Membranics:** \[WP-TODO: EHB\]

[^evolvability]: We, Neighborhoods, Ad4m (https://ad4m.dev/) \[WP-TODO: insert links
    here\]
