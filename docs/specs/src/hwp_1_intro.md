---
title: 'Holochain'
subtitle: 'Distributed Coordination by Scaled Consent not Global Consensus'
author:
 - Eric Harris-Braun
 - Arthur Brock
 - Paul d'Aoust
abstract: |
 We present a generalized system for large scale distributed coordination that does not rely 
 on global consensus --  explicating the problem through the lens of scaling consent; After 
 establishing initial axioms, we illustrate their application for coordination in games, then 
 provide a formal outline of the necessary integrity guarantees and functional components needed 
 to deliver those guarantees. Finally, we provide a fairly detailed, high-level, technical 
 specification for our operating implementation for scaling consent.
documentclass: 'revtex4-1'
---

# Introduction

## Preamble -- A focus on practice, not just theory

The original/alpha version of the [Holochain white paper](https://github.com/holochain/holochain-proto/blob/whitepaper/holochain.pdf) took a formal approach to modeling generalized distributed computation schemes and contrasted Holochain's approach with Blockchain-based systems. It also provided formal reasoning for the scaling and performance benefits of Holochain's agent-centric approach.

When dealing with distributed systems the application of logical models and formal proofs are often deceiving. This stems from how ridiculously easy it is to define logical/conceptual sets and conditions which are fundamentally impossible and unintelligible in practice. Since our primary intent with Holochain is to provide a highly functional and scalable framework for sophisticated decentralized coordination, our focus must be on what is practicable, and resist the pull of the purely conceptual which frequently steers builders into unwieldy architectures.

Note how easy it is to reference something like "All living persons" or "All nodes in the network." But in actuality, it is impossible to identify that set in the real physical world. Even after eliminating fuzziness in the terms as they may apply to fetuses, or brain-dead people on life-support, or people undergoing resuscitation, because it is fundamentally an open system, there is no way to discover and record the information quickly enough about who is dying and being born to construct the actual data set. Likewise, no sngle agent can know with certainty which nodes have gone offline, come online, or have declared themselves as new nodes, and since network partitions are a real, one must question which network is being referring to when it's partitioned. Wouldn't we want agents in each and every partition to be able to continue to function as well as possible in spite of being partioned?

Actually, the previous example should have been a rather easy data set work with, since it changes so slowly -- typically each person undergoes a state change only twice in their life (when they become a living person, and when they cease to be one). The kinds of tools we need for powering the next web need to handle much more rapid and complex changes. A more apt logical construct might be "All people with just one foot on the ground" which might function more like "Changes of state by each agent." Membership in this set changes quite rapidly -- around 1/2 billion times per second[footnote], since when walking, there is a moment in each step where a person only has one foot on the ground.

It should be obvious there is no way in practice to work with that data set without requiring actions that either break reality (like freezing time) or asserting a god-like, omniscient being, who not only has instantaneous access to all knowledge (requiring information propagation faster than the speed of light), but also has infinite attention to be aware of all of this simultaneously (likely requiring infinite energy). However, since our current computing architectures are bound by laws of physics, we should avoid the all-to-easy temptation of injecting such impossible constructs into our model. A proof that involves simple logical concepts which cannot be reliably constructed in practice, is not much of a proof at all.

"Global state" and strategies for consensus about it, are exactly one of these slippery constructs which is an easy concept, but cannot adequately reflect events happening in physical reality without drastic reduction of complexity, agency, and degrees of freedom. Yet most current distributed systems undertake the expensive task of having each node construct and maintain this unwieldy fiction. Hence blockchains may run on thousands of processors, but advance in lockstep as a single threaded process, and they are mostly reliable for a very simplified world models, like moving tokens (subtracting from a number in one address and adding it to the number in another address).

"State" within the local computing context may have its origins in the concept of the Turing Tape[footnote] or Von Neumann linear memory address architecture[footnote]. With the introduction of multi-core processors, programmers discovered the myriad of problems with performing simultaneous operations even on this local state, so various strategies were developed to enforce safety in concurrent local operations. In distributed systems, people have extrapolated some of these local strategies and invented some new ones, **still in the attempt to manage a single state.** The simple logical concept of managing a global state persists, even when the topology the concept has been mapped onto has made it unknowable in practice.

Early influental works in decentralized computing (such as the Byzantine Generals Problem[footnote]) may have also set expectations. Those papers were written in the context of reaching consensus in finite control systems where there was a known number of sensors and states, and the goal was reach a unified decision (like nine generals deciding a time for all to attack). Therefore, to be Byzantine Fault Tolerant[footnote], most distributed systems have assumed[footnote] that global consensus must be constructed in order to reach a unified action. In this paper, we will detail some more efficient paths to enable agents to act which do not require a construct of global consensus at all.

So, while the formalizations from the original Holochain white paper may still be valid in theory, this white paper is concerned with what works in practice. So we will start by stating our underlying assumptions as axioms -- each of which correlates to architectual properties of Holochain. And we will take special care not to make grand, categorical statements which cannot be implemented inside the constraints of computional systems bounded by the laws of physics.

## Axioms -- Our underlying basis for coordination

Here we spell out the assumptions upon which we have built our approach the challenge of decentralized coordination.

First, let us clarify what we mean by coordination, our goals for coordination are:
 - To enable a group to establish ground rules which form the social context for whatever is being coordinated,
 - To enable agents in the group to take effective or correct action in that context,
 - To protect agents and the group from incorrect actions any agent may take.

### Axioms for multi-agent coordination through scaled consent

1. Agency is defined by the ability to take individual action.
2. "State" is persisted data that an agent can access through introspection.
3. It is easy to agree on a starting state -- so the grammar for coordination must be established up front.
4. It is hard to maintain a unified dynamic shared state across a network (because of the constraints of physics).
5. Since only local time is knowable -- non-local ordering is constructed by explicit reference.
6. Agents always act based on their state -- data they can access locally.
7. (non unified action & accountability for it?) 
8. protections from bad actions? Two levels... Evolvable agreements... app updates and data migration. anti-fragility

### Detailed Axioms and Architectural Consequences

... which have informed the architecture of Holochain in the following ways...

**Agency is defined by the ability to take individual action.** Each agent is the sole authority for changing their state. Since Holochain uses cryptography to eliminate many types of faults, this means constructing a public/private keypair and signing state changes to an append-only log of the agent's actions. The log contains only actions of this agent, and writing to it (changing their own state), then sharing their state changes is the essentially only authority an agent has. 

**"State" is persisted data that an agent can access through introspection.** Since there is no "god" agent that can introspect data which theoretically exists somewhere in the system across the network, agents must share state changes, and have visibility to discover and query state change information regardless of from whom it originated. Once an agent holds the data (whether because they authored it or received via coordination protocols) it is now part of their introspectable state. To act on such data, an agent still must verify whether it is true/false, complete/incomplete, authentic/simulated, isolated/connected, etc. However, the "state" of a Holochain app does not generally include ephemeral, non-persisted data such as what network sessions with other agents one may currently have open, although Holochain itself uses that kind of data to drive coordination. This is similar to how a program delegates concerns of mouse clicks, key presses, and file loads to the underlying operating system which directs those data streams to the correct program.

**It is easy to agree on a starting state -- so the grammar for coordination must be established up front.** The very first entry in an agent's state log for an app is the hashed references to the code which establishes the grammar of coordination for that app. This code defines data structures, linking patterns, validation rules, business logic, and roles which are used and mutually enforced by all participating agents. The hash of this first entry defines the space and methods of coordination -- agents only need to coordinate with other agents who are "speaking the same language." This establishes an intentional partition in support of scalability, because an agent doesn't need to interact with all agents running Holochain apps, only the agents operating under the same ground rules -- simplifying and focusing overhead for validation and coordination.

**It is hard to maintain a unified dynamic shared state across a network (because of the constraints of physics).** In a distributed and open system, which enables autonomous agents to make valid changes to their state, one cannot expect any one agent to know the state of all other agents across whatever partitions they may be in and making whatever realtime changes they may. Such an assumption requires either centralization or omniscience about unreachable data. However, it is feasible to ensure strong eventual consistency, so that when any agents interact with the same data, all will converge to matching conclusions about the validity of any state change it represents. 

**Since only local time is knowable -- non-local ordering is constructed by explicit reference.** The first simple structure for constructing order by reference is that each agent's action log is structured as an unbroken hash chain, with every new action's header containing the hash of the prior action. Timestamps are also included, but must be understood to not be any kind of authoritative universal time. When agents make state changes that rely on their prior state, the chain automatically provides explicit sequence. When an agent's action has logical dependencies another agent's data, they must reference the hash of those action(s) of to provide explicit ordering.

**Agents always act based on their state -- data they can access locally.**


7. (non unified action & accountability for it?) 
8. protections from bad actions?


3. // Agents consenting to a shared set of rules is the foundation for coordination.  (It is easy for agents to start with the same ground rules / static agreements up front are easy.  // evidence of having the rules is that they're playing by the rules // shared social context establishe up fronState and sequence are both localt

4. It is hard to maintain agreement about dynamic and changing state, especially across partitionable networks with incomplete and massively simultaneous action

5. Agents can share information about state and state changes, which happen across relativistic time, and unreliable communication media. A system needs to be able to function without requiring an omniscient global perspective which is unreliable to construct.









We begin by stating our axioms[^byzantine] regarding the nature of coordination:

[^byzantine]: Some readers may come to the problems of distributed coordination from the framework laid out by the literature on Byzantine Fault Tolerance. These axioms and a discussion of why to start with them are explained in our paper [[The Players of Ludos: Beyond Byzantium](https://docs.google.com/document/d/1HBNgIooElD5widCuX9XmiOzbVIpEF5XXH67mZbnUFjo/edit#).

Axiom 1. **Coordination arises from agents**: Starting from the same ground rules, any single agent is able to act *as soon* as they have confirmed to their satisfaction that other agent's previous actions conform to the rules. (Thus, in our frame, coordination looks like alignment heading in the same direction, rather than agreement proceeding in lockstep.)

Axiom 2. **Coordination is grammatic[^grammatic]**, in that it comes from embodying a geometry that removes uncertainty, and embodying an ability to compose different coordinative subsystems that have different ground rules.

[^grammatic]: We use the term "grammatic" as a way to generalize from the usual understanding of grammar which is linguistic.  Where grammar is often understood to be limited to language, grammatics points to the pattern of creating templates with classes of items that can fill slots in those templates. This pattern can be used for creating "grammars" of social interaction, "grammars" of physical structures (we would call Christopher Alexander's "A Pattern Language" for architecture an example of grammatics) and so on.

**Axiom 1: TODO: State and sequence are both local** -- arises from the insight that we cannot fight against the physical reality of different experiences by different nodes in the context of networked interaction nor fight against the real-world non-existence of global temporal order because order is relative to vantage point. Instead, any coordination system must align its ontology with the truth that **global state does not actually exist**. Thus we start with what does actually exist: local temporal state. Changes to this local state can be shared with, and validated by, others in conformity with pre-defined ground rules. In so doing we can still achieve difficult and complex coordination safely (including in the context of problems on the scale of global monetary transactions) without the bottlenecks from maintaining the expensive fiction of a single shared global state. Each agent in a Holochain application is the sole author of their local sequence and state, and their changes are validated when shared with the coordiation layer..

**Axiom 2: TODO: NEED A BRIEFLY PHRASED AXIOM** -- arises from the insight that systems for successful large scale coordination demand the property of anti-fragility, that is, they must perform better under perturbation[^antifragile]. Coordination happens in the context of fundamentally dynamic environments in which the coordinating elements are changed by the fact of their coordination; that is, coordination is a co-evolutionary context. We claim by this axiom that what meets the challenge of anti-fragility in such contexts is composable sub-systems, in which the composition comes out of a grammar that embodies the actual dimensionality of the problem subdomains (i.e., their geometry)[^embodiment], and by which agents in that context can react powerfully to perturbations because the available composability is dimensionally aligned.

[^antifragile]: Antifragile: Things that Gain from Disorder. Nassim Nicholas Taleb

[^embodiment]: Insofar as our compute-powered platforms are meant to solve problems in particular domains, we take it as critical that the ways those problems show up in the platform actually meet the dimensionality of the problem space. By this we mean that the independent variables or ontological entities that are part of the problem space are reflected in the compute system. That reflection we call embodiment in the system. A generalized platform for creating applications that solve problems must therefore embody this higher-level dimensionality of the problem space of "generalized application creation" itself, and in our case, it must do so in an evolvable manner. Our use of the term "geometry" here is similarly intended to help elucidate the notion of dimensionality, in that geometries distinguish independent directions of motion and the relations between them.

The core axiom (though not explicitly stated as such) of the Byzantine Generals' Problem is that coordination starts *after* "consensus on state", i.e., that the Lieutenants can't execute their plan until everyone has followed the consensus algorithm and arrived at a single data reality[^faults]. This axiom leads system designers to figure out how to implement machinery for **Global Consensus**. Our axioms lead us, instead, to implement tooling for **Scaling Consent** as an alternate solution to the Byzantine Generals' Problem.

[^faults]: In [Reaching Agreement in the Presence of Faults](https://dl.acm.org/doi/pdf/10.1145/322186.322188), MARSHALL PEASE, ROBERT SHOSTAK, and LESLIE LAMPORT, this single data reality is called "interactive consistency" and is about the vector of "Private Values" sent by each node.

## From Global Consensus to Scaled Consent

*Normally one uses AXIOMS to reaason upon and create some conclusions they intend to demonstrate or prove... should that be what happens in this section?*

In distributed systems, it is absolutely fundamental to understand that every action taken by an agent in any social context happens because that agent has crossed a confidence threshold of some sort. From its own point of view, that the given action is appropriate to take. Stated another way: agentic assessment of the social context and its coherence allows agents to act. This is always true, whether through centralized coordiation or a Byzantine Generals' Problem approach or by Blockchain consensus algorithms.

We also start from the understanding that social spaces are inherently uncertain. Thus, coordination/collaboration is never about deterministic certainty but simply about the capacity to remove sufficient uncertainty to provide confidence for action, which is always contextual. Such confidence indicates **social coherence**. This notion of social coherence is the single most important design goal of Holochain: to create the tooling that in contextually appropriate ways leads to increasing social coherence.

Given the above, we propose a very simple approach to creating tooling capacity for building increasing certainty: **enable validated global visibility, on demand, of local state**. In this approach, we distinguish between *authorship*, which is about local state changes initiated by agents, and *authority*, which is about distributing the responsibility of validating and making visible those state changes across the participants in the network. This approach requires that we:

1. Ensure that all agents can *reliably* see what's going on; i.e., offer a framework for adding to and querying a collectively held database in which there is a minimum or "floor" of certainty regarding the contents and authorship of data even in the presence of an unbounded number of adversaries.

2. Ensure that all agents know the "ground-rules"; i.e., offer a framework for composing many small units of social agreement in which players can add elements of deterministic certainty into their interactions, yielding an appropriate level of certainty ranging from arbitrarily low to arbitrarily high.

The first point we deliver through various types of **Intrinsic Data Integrity**. We use a number of cryptographic methods to create self-proving data of various types:

* **Provenance**: An agent's network address is their public key. Thus, when interacting with agents it's possible to have deterministic confidence in whom one is interacting with because there is no identity layer between network locations subject to attack surface. I.e., unlike a web address, you don't need a domain name certificate associated with the domain name to become confident of
  "whom" you are talking to.

* **Signatures**: Because provenance is a public key, it's also easy to create self-proving authenticity. All messages sent, and all data committed to chains, is signed by agents using their public key. Thus any agent can immediately, and with high confidence, verify the authenticity of messages and data.

* **Hashes**: All data on our DHT is addressed by its hash. Thus, when retrieving data it's possible to have deterministic confidence that it hasn't been tampered with by whoever was storing or relaying it.

* **Monotonicity**: The system is both structurally and logically monotonic. Structurally, local state is append-only and shared state can only grow. Data can be marked as deleted, but it is never actually removed from the state history. Logically, once a state change has been validated, it should never be able to become invalid.

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
