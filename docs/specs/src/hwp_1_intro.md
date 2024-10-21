---
title: 'Holochain'
subtitle: 'Distributed Coordination by Scaled Consent not Global Consensus'
author:
 - Eric Harris-Braun
 - Arthur Brock
 - Paul d'Aoust
abstract: |
 We present a generalized system for large-scale distributed coordination that does not rely
 on global consensus, explicating the problem instead through the lens of scaling consent. After
 establishing initial axioms, we illustrate their application for coordination in games, then
 provide a formal outline of the necessary integrity guarantees and functional components needed
 to deliver those guarantees. Finally, we provide a fairly detailed, high-level, technical
 specification for our operating implementation for scaling consent.
documentclass: 'revtex4-1'
---

# Introduction

## Preamble -- A Focus on Practice, Not Just Theory

The original/alpha version of the [Holochain white paper](https://github.com/holochain/holochain-proto/blob/whitepaper/holochain.pdf) took a formal approach to modeling generalized distributed computation schemes and contrasted Holochain's approach with blockchain-based systems. It also provided formal reasoning for the scaling and performance benefits of Holochain's agent-centric approach.

When dealing with distributed systems, however, the application of logical models and formal proofs are often deceiving. This stems from how easy it is to define sets and conditions which are logically solid in theory but fundamentally impossible and unintelligible in practice. Since our primary intent with Holochain is to provide a highly functional and scalable framework for sophisticated decentralized coordination, our focus must be on what is practicable, and resist the pull of the purely conceptual which frequently steers builders into unwieldy architectures.

Note how easy it is to reference a theoretical set like "all living persons" or "all nodes in the network." But in reality, it is impossible to identify that set in the physical world. Even if one could eliminate the fuzzy boundaries in the meaning of "persons" or "living," there is no way to discover and record the information quickly enough about who is dying, being resuscitated, and being born to construct the actual data set. Likewise, no single agent on a network can determine with certainty which nodes have come online, gone offline, or have declared themselves as new nodes. Also, since network partitions are a real, at any moment, one must question which partition is considered "the network," and how to enable a single node or group of nodes to continue operating appropriately even when no longer connected to the main network.

The initial example should be a comparatively easy data set to work with, since it changes relatively slowly. Typically each person undergoes a state change only twice in their life (when they become a living person, and when they cease to be one). However, to power the next web, tools need to handle much more rapid and complex changes. A more apt logical construct might be "all people with just one foot on the ground". Membership in this set changes quite rapidly -- around 1/2 billion times per second[^steps].

[^steps]:Global smartphone data indicates an average of 5,000 steps per day per person. [https://www.aicr.org/news/globally-the-average-person-walks-about-5000-steps-per-day/)]

It should be obvious there is no practical way to work with that data set without requiring actions that either break reality (like freezing time) or asserting a god-like, omniscient being, who not only has instantaneous access to all knowledge (requiring information propagation faster than the speed of light), but also has infinite attention to all states (likely requiring infinite energy). However, since current computing architectures are bound by laws of physics, we should avoid the temptation of injecting such impossible constructs into our theoretical models. A proof that involves simple logical concepts which cannot be reliably worked with in practice is not much of a proof at all.

"Global state" and strategies for consensus about it are exactly one of these dodgy constructs which are easy concepts, but involve a drastic reduction of complexity, agency, and degrees of freedom to reflect a small subset of events happening in physical reality. Yet most current distributed systems undertake the expensive task of having each node construct and maintain this unwieldy global fiction. So although many blockchains run on tens of thousands of processors, they advance in lockstep as if a single-threaded process, and they are only reliable for very simple world models, like moving tokens (subtracting from a number in one address and adding it to the number in another address).

"State" within the local computing context is likely rooted in the concept of the Turing Tape[^turing] or Von Neumann linear memory address architecture[^memory] which assume a single tape head or system bus for making changes to the single local memory space where changes are stored. With the introduction of multi-core processors, programmers encountered the myriad problems of having multiple agents (CPUs) operating on just one shared local state. They developed various strategies to enforce memory safety for concurrent local operations. So, in distributed computing, people extrapolated these local strategies and starting inventing some new ones, **still in the attempt to manage one single state across many physical machines.** The assumption of the need to sustain this simple logical concept of managing one global state persisted, even when that concept was mapped onto a physical topology which made it fundamentally unknowable in practice.

[^turing]: See [https://en.wikipedia.org/wiki/Turing_machine].

[^memory]: See [https://en.wikipedia.org/wiki/Von_Neumann_architecture].

Early influential works in decentralized computing (such as the Byzantine Generals' Problem[^byzantine]) may have also set such expectations. Those papers were written in the context of reaching consensus in finite control systems where there was a known number of sensors and states, and the goal was to reach a unified decision (like nine generals deciding a time for all to attack). Therefore, to be Byzantine Fault Tolerant, seems simply that a system is tolerant of the kinds of faults introduced by the generals problem (messages that are corrupted, misordered, or lost and generals/nodes that are malicious), but most distributed systems have assumed that global consensus must be constructed and maintained in order to reach a unified outcome. In this paper, we will detail some more efficient paths to enable agents to act without a construct of global consensus at all, yet still have strong guarantees that even when nodes act in partitioned groups or individually, they will reach a unified outcome.

[^byzantine]: Some readers may come to the problems of distributed coordination from the framework laid out by the literature on Byzantine Fault Tolerance such as *The Byzantine Generals Problem*: Leslie Lamport, Robert Shostak, and Marshall Pease [https://lamport.azurewebsites.net/pubs/byz.pdf] and *Reaching Agreement in the Presence of Faults*: Marshall Pease, Robert Shostak, and Leslie Lamport [https://dl.acm.org/doi/pdf/10.1145/322186.322188](https://dl.acm.org/doi/pdf/10.1145/322186.322188). These axioms and a discussion of why to start with them are explained in our paper [The Players of Ludos: Beyond Byzantium](https://holochain.org/papers/holochain-players-of-ludos.pdf).

So, while the formalizations from the original Holochain white paper are still valid in theory, this white paper is more concerned with addressing what works in practice. We will start by stating our underlying assumptions as axioms -- each of which correlates to architectural properties of Holochain. And we will take special care not to make grand, categorical statements which cannot be implemented inside the constraints of computational systems bounded by the laws of physics.

## Axioms -- Our Underlying Basis for Coordination

Here we spell out the assumptions upon which we have built our approach to address the challenges of decentralized coordination.

First, let us clarify what we mean by coordination; our goals for coordination are:

* To enable a group to establish ground rules which form the context needed for coordination,
* To enable agents in the group to take effective or correct action inside that context,
* To protect agents and the group from incorrect actions any agent may take.

### Axioms for Multi-agent Coordination Through Scaled Consent

1. Agency is defined by the ability to take individual action.
2. "State" is persisted data that an agent can access through introspection.
3. It is easy to agree on a starting state, so the ground rules for coordination must be established up front.
4. It is hard to maintain a unified, dynamic, shared state across a network, because of the constraints of physics.
5. Since only local time is knowable, non-local ordering is constructed by explicit reference.
6. Agents always act based only on their state; that is, data they can access locally.
7. Incorrect actions taken by an agent must harm only themself.
8. Long-term coordination must include the ability to orchestrate changes to ground rules.

### Detailed Axioms and Architectural Consequences

... which have informed the architecture of Holochain in the following ways...

**Agency is defined by the ability to take individual action.** Each agent is the sole authority for changing their state; the corollary of this is that an agent _cannot_ have authority to change other agents' states. Since Holochain uses cryptography to eliminate many types of faults, this primarily means constructing a public/private key pair and using it to sign state changes recorded on an append-only log of the agent's actions. The log contains only actions of this agent, and writing to it (changing their own state), then sharing their state changes, is essentially the only authority (in terms of authorship) an agent has.

**"State" is persisted data, local to an agent, that the agent can access through introspection.** Since there is no "god" agent that can introspect data which theoretically exists somewhere in the system across the network, agents must share state changes and be able to discover and query such information regardless of from whom it originated. Once an agent holds the data (whether because they authored it or received via networked communication) it is now part of their introspectable state. To act on such data, an agent still must verify whether it is true/false, complete/incomplete, authentic/simulated, isolated/connected, etc. However, the "state" of a Holochain app does not generally include ephemeral, non-persisted data such as what network sessions with other agents one may currently have open, although Holochain itself uses that kind of data to drive coordination.

**It is easy to agree on a starting state, so the ground rules for coordination must be established up front.** The very first entry in an agent's state log for an app is the hashed reference to the code which establishes the grammar of coordination for that app. This code defines data structures, linking patterns, validation rules, business logic, and roles which are used and mutually enforced by all participating agents. The hash of this first entry defines the space and methods of coordination -- agents only need to coordinate with other agents who are "speaking the same language." This establishes an intentional partition in support of scalability, because an agent doesn't need to interact with all agents running Holochain apps, only the agents operating under the same ground rules -- simplifying and focusing overhead for validation and coordination.

**It is hard to maintain a unified, dynamic, shared state across a network, because of the constraints of physics.** In a distributed and open system, which enables autonomous agents to make valid changes to their state, one cannot expect any one agent to know the state of all other agents across whatever partitions they may be in and making whatever realtime changes they may. Such an assumption requires either centralization or omniscience about unreachable data. However, it is feasible to ensure strong eventual consistency, so that when any agents interact with the same data, all will converge to matching conclusions about the validity of any state change it represents.

**Since only local time is knowable, non-local ordering is constructed by explicit reference.** In the physical universe, entities experience time only as a local phenomenon; any awareness of other entities' experience of time comes from direct interaction with them. Thus, "global time" as a total ordering of causal events does not exist and entities' interactions form only partial orderings.

In Holochain, "direct interaction" comes in the form of explicit hash references to other data. The first simple structure for constructing order by reference is that each agent's action log is structured as an unbroken hash chain, with every new action containing the hash of the prior action. (Timestamps are also included, but are understood to be an expression of local time only.) When agents make state changes that rely on their prior state, the chain automatically provides explicit sequence. When an agent's action has logical dependencies on another agent's data, they must reference the hash of those action(s) to provide explicit ordering. In almost every application, there is no need to construct absolute time or sequencing to guarantee correct action. If the problem cannot be restructured to eliminate all rivalrous state data, Holochain provides tools to implement conflict resolution or micro-consensus for that small subset of data for which it remains useful.

**Agents always act based on their state; that is, data they can access locally.** Since agents must act on what they know, they should be able to act *as soon* as they have have whatever local knowledge they need to take an action, with the assurance that any other nodes validating their action will reach the same conclusion. There is no reason to wait for other agents to reach a state *unless that is the confidence threshold required* to take that particular action. It is possible to architect agent-centric solutions to most decentralized problems which are many orders of magnitude more efficient than managing global, data-centric consensus. For example, this even includes building cryptocurrencies structured as peer-to-peer accounting instead of global tracking of token locations, enabling the transacting agents (the only ones who are changing their states) to validate each other's actions and countersign a transaction independent from the rest of the network, who will validate it when they receive it after it is done.

**Incorrect actions taken by an agent must harm only themself.** We mentioned in the goals of collaboration that incorrect actions must not harm other agents or the integrity of the group. This is accomplished via the validation that occurs during publishing and gossip. When a node receives a data element which it is supposed to store and serve as part of the architecture of global visibility into local state changes, the receiving agent must validate it before integrating and serving it to others. For the previous example of a cryptocurrency, if the sender did not have enough credits in their chain state for the amount they are sending, the transaction would fail validation. The validating agents mark this action invalid, add both parties who signed the transaction to their blocked list, and publish a "warrant" letting others know about the corrupt nodes so other agents can block them. These warrants function as an immune system which protects individuals and the group from malicious or corrupt actors. The agents did not need to be prevented from taking the bad action, because they only changed their own state, and the bad action becomes the proof needed for the warrant to protect others from it.

**Long-term coordination must include the ability to orchestrate changes to ground rules.** Coordination cannot be effective without comprehension of the real-world context within which it is happening. However, agents cannot fully comprehend their context at first; understanding comes with interaction over time. And as the agents interact with and within their context, they may need to evolve that context, as they encounter new new situations which were not comprehended when the current ground rules were established. Any "grammar"[^grammatic] in which ground rules are written must be expressive enough to write rules that address the needs of the problem domain as well as a capacity to evolve rules in response to changing comprehension and context. In Holochain, the ground rules for a group are expressed in executable code. Its tools include an affordance for agents to migrate to a new group with a new set of rules, as well as the ability for an agent to "bridge" their presence in two different groups via cross-space function calls.

[^grammatic]: We use the term "grammatic" as a way to generalize from the usual understanding of grammar which is linguistic.  Where grammar is often understood to be limited to language, grammatics points to the pattern of creating templates with classes of items that can fill slots in those templates. This pattern can be used for creating "grammars" of social interaction, "grammars" of physical structures (we would call Christopher Alexander's "A Pattern Language" for architecture an example of grammatics), and so on.

Building a distributed coordination framework starting from these axioms results in a system that empowers agents to take independent and autonomous action with partial information as soon as they have whatever they need to ensure it is a correct action. This constitutes a significant departure from the frame of thinking that Byzantine Fault Tolerance traditionally assumes: that a construction of complete consensus is required *before* an agent can act.

## From Global Consensus to Scaled Consent

*Normally one uses AXIOMS to reaason upon and create some conclusions they intend to demonstrate or prove... should that be what happens in this section?*  **Two levels enabling evolution ??**

demand the property of anti-fragility, that is, they must perform better under perturbation[^antifragile]. 

[^antifragile]: Antifragile: Things that Gain from Disorder. Nassim Nicholas Taleb

We also start from the understanding that social spaces are inherently uncertain. Thus, coordination/collaboration is never about deterministic certainty but simply about the capacity to remove sufficient uncertainty to provide confidence for action, which is always contextual. Such confidence indicates **social coherence**. This notion of social coherence is the single most important design goal of Holochain: to create the tooling that in contextually appropriate ways leads to increasing social coherence.

In distributed systems, it is absolutely fundamental to understand that every action taken by an agent in any social context happens because that agent has crossed a confidence threshold of some sort. From its own point of view, that the given action is appropriate to take. Stated another way: agentic assessment of the social context and its coherence allows agents to act. This is always true, whether through centralized coordination or a Byzantine Generals' Problem approach or by blockchain consensus algorithms.

Given the above, we propose a very simple approach to creating tooling capacity for building increasing certainty: **enable validated global visibility, on demand, of local state**. In this approach, we distinguish between *authorship*, which is about local state changes initiated by agents, and *responsibility*, which is about distributing the workload of validating and serving records of local state changes across the participants in the network. This approach requires that we:

1. Ensure that all agents can *reliably* see what's going on; i.e., offer a framework for adding to and querying a collectively held database in which there is a minimum or "floor" of certainty regarding the contents and authorship of data even in the presence of an unbounded number of adversaries.

2. Ensure that all agents know the "ground-rules"; i.e., offer a framework for composing many small units of social agreement in which players can add elements of deterministic certainty into their interactions, yielding an appropriate level of certainty ranging from arbitrarily low to arbitrarily high.

The first point we deliver through various types of **Intrinsic Data Integrity**. We use a number of cryptographic methods to create self-proving data of various types:

* **Provenance**: An agent's network address is their public key. Thus, when interacting with agents it's possible to have deterministic confidence in whom one is interacting with because there is no identity layer between network locations subject to attack surface. I.e., unlike a web address, you don't need a domain name certificate associated with the domain name to become confident of "whom" you are talking to.

* **Signatures**: Because provenance is a public key, it's also easy to create self-proving authenticity. All messages sent, and all data committed to chains, is signed by agents using their public key. Thus any agent can immediately, and with high confidence, verify the authenticity of messages and data.

* **Hashes**: All data on our DHT is addressed by its hash. Thus, when retrieving data it's possible to have deterministic confidence that it hasn't been tampered with by whoever was storing or relaying it.

* **Monotonicity**: The system is both structurally and logically monotonic. Structurally, local state is append-only and shared state can only grow. Data can be marked as deleted, but it is never actually removed from the history of the agent who authored it. Logically, once any peer has identified that state change is invalid, no peers should identify it as valid.

* **Common Genesis**: The Validation Rules and joining criteria of an application are the first entry in every chain. This provides a mechanism for self-proving, shared ground rules. Any agent can examine the chain of any other agent all the way back to the source and thus have high confidence that they have actually committed to play by the same rules.

Building upon this floor, we deliver the second point through the ability to compose various types of **Validation Rules**. Validation rules create certainty in the following dimensions, with some examples:

* **Content**: a string does not exceed a maximum length
* **Structure**: an entry consists of a certain set of types of data[^content-structure]
* **Sequence**: someone can not spend credits they have not already received
* **Process**: a transaction must be approved and signed by a notary
* **Behavior**: one does not take an action more frequently than a certain rate
* **Dependency**: an editor can only make changes if another agent has given them prior authorization

[^content-structure]: While Per Martin-Löf [demonstrated](https://en.wikipedia.org/wiki/Intuitionistic_type_theory) that values can be unified with classical types into a single [dependent type theory](https://en.wikipedia.org/wiki/Dependent_type), thus showing that content and structure can be equivalent and share a single calculus, here we distinguish the two in order to speak a language that is more familiar to programmers.

The two domains of Intrinsic Data Integrity and Validation Rules, and their component dimensionality, amounts to what we might call a "Geometry of Certainty". Inside the clarity of such a geometry, the gradients of certainty become both much more visible, and much easier to build appropriately for. Thus it provides a context of agents being able to scale up their consent to play together in ways that meet their safety requirements. This is why we call our approach "Scaling Consent." It is what enables coherent collaborative action without first achieving global consensus.
