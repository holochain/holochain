---
title: 'Holochain'
subtitle: 'Distributed Coordination by Scaled Consent not Global Consensus'
author:
 - Eric Harris-Braun
 - Arthur Brock
abstract: |
 In this paper we present a frame and a specification for a generalized system for
 large scale distributed coordination that does not rely on global consensus. We start
 with some axioms; we proceed to describe the consequences of these axioms, looking
 at the problem through the lens of scaling consent; we present an informal description
 of the system; we provide a more formal outline of the necessary integrity guarantees
 and system components needed to deliver those guarantees; and finally we conclude with
 a high-level yet sufficiently detailed technical specification of our implementation
 of such a system.
documentclass: 'revtex4-1'
---

# Introduction

## Preamble

The alpha version of the [Holochain white paper](https://github.com/holochain/holochain-proto/blob/whitepaper/holochain.pdf) took a very formal approach to describing generalized distributed computation schemes and carefully compared the approach taken by Blockchain-based systems with the approach taken by Holochain as well as providing formal reasoning for the benefits of Holochain's agent-centric approach.

In this second version, we describe Holochain on its own terms by first providing the highest level view of our approach and our starting axioms. Then, at the next level of detail we provide an informal system description using the metaphor of games. Then we dive into deeper detail with a formal system description including the specific context and assumptions we come from, the integrity guarantees Holochain offers, and a formal state model and discussion of security and safety concerns. Finally, at the most detailed level, we provide an implementation specification in Appendix A.

## Axioms

We begin by stating our axioms[^byzantine] regarding the nature of coordination:

[^byzantine]: Some readers may come to the problems of distributed coordination from the framework laid out by the literature on Byzantine Fault Tolerance. These axioms and a discussion of why to start with them are explained in our paper [[The Players of Ludos: Beyond Byzantium](https://docs.google.com/document/d/1HBNgIooElD5widCuX9XmiOzbVIpEF5XXH67mZbnUFjo/edit#).

1. **Coordination arises from agents**: Starting from the same ground-rules, and then any single agent's ability to act *as soon* as they have confirmed to their satisfaction that other agent's previous actions conform to those ground rules. (Thus, in our frame, coordination looks like alignment heading in the same direction, rather than agreement proceeding in lockstep.)

2. **Coordination is grammatic[^grammatic]**, in that it comes from embodying a geometry that removes uncertainty, and embodying an ability to compose different coordinative subsystems that have different ground-rules.

[^grammatic]: We use the term "grammatic" as a way to generalize from the usual understanding of grammar which is linguistic.  Where grammar is often understood to be limited to language, grammatics points to the pattern of creating templates with classes of items that can fill slots in those templates. This pattern can be used for creating "grammars" of social interaction, "grammars" of physical structures (we would call Christopher Alexander's "A Pattern Language" for architecture an example of grammatics) and so on.

Axiom 1 arises from the insight that we cannot fight against the physical reality of different experiences by different nodes in the context of networked interaction nor fight against the real-world impossibility of determining global temporal order because of the non-existence of simultaneity. Instead, any coordination system must align its ontology with the truth that **global state actually does not exist**. Thus we start with what does actually exist: local temporal state. This local state can be shared with, and validated by, others in conformity with pre-defined ground-rules. In so doing we can still achieve difficult and complex coordination safely (including in the context of problems on the scale of global monetary transactions) without the costs and bottlenecks that arise from starting from the ontology of a single shared global state. Holochain is an implementation of a system using this alternate frame.

Axiom 2 arises from the insight that systems for successful large scale coordination demand the property of anti-fragility, that is, they must perform better under perturbation[^antifragile]. Coordination happens in the context of fundamentally dynamic environments in which the coordinating elements are changed by the fact of their coordination; that is, coordination is a co-evolutionary context. We claim by this axiom that what meets the challenge of anti-fragility in such contexts is composable sub-systems, in which the composition comes out of a grammar that embodies the actual dimensionality of the problem subdomains (i.e., their geometry)[^embodiment], and by which agents in that context can react powerfully to perturbations because the available composability is dimensionally aligned.

[^antifragile]: Antifragile: Things that Gain from Disorder. Nassim Nicholas Taleb

[^embodiment]: Insofar as our compute-powered platforms are meant to solve problems in particular domains, we take it as critical that the ways those problems show up in the platform actually meet the dimensionality of the problem space. By this we mean that the independent variables or ontological entities that are part of the problem space are reflected in the compute system. That reflection we call embodiment in the system. A generalized platform for creating applications that solve problems must therefore embody this higher-level dimensionality of the problem space of "generalized application creation" itself, and in our case, it must do so in an evolvable manner. Our use of the term "geometry" here is similarly intended to help elucidate the notion of dimensionality, in that geometries distinguish independent directions of motion and the relations between them.

The core axiom (though not explicitly stated as such) of the Byzantine Generals' Problem is that coordination starts *after* "consensus on state", i.e., that the Lieutenants can't execute their plan until they have followed the consensus algorithm and arrived at a single data reality[^faults]. This axiom leads system designers to figure out how to implement machinery for **Global Consensus**. Our axioms lead us, instead, to implement tooling for **Scaling Consent** as an alternate solution to the Byzantine Generals' Problem.

[^faults]: In [Reaching Agreement in the Presence of Faults](https://dl.acm.org/doi/pdf/10.1145/322186.322188), MARSHALL PEASE, ROBERT SHOSTAK, and LESLIE LAMPORT, this single data reality is called "interactive consistency" and is about the vector of "Private Values" sent by each node.

## From Global Consensus to Scaled Consent

In distributed systems, we believe that it is absolutely fundamental to understand that every action taken by an agent in any social context happens because that agent has crossed a confidence threshold of some sort, from its own point of view, that the given action is appropriate to take. Stated another way: agentic assessment of the social context and its coherence allows agents to act. This is always true, be it as solved in the Byzantine Generals' Problem problem or by Blockchain consensus solutions.

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

Building upon this floor, we deliver the second point through the ability to compose various types of **Validation Rules**. Validation rules create certainty in the following dimensions, with examples:

* **Content**: a string does not exceed a maximum length
* **Structure**: an entry consists of a certain set of types of data[^content-structure]
* **Sequence**: someone can not spend credits they have not already received
* **Process**: a transaction must be approved and signed by a notary
* **Behavior**: one does not take an action more frequently than a certain rate
* **Dependency**: an editor can only make changes if another agent has given them prior authorization

[^content-structure]: While Per Martin-Löf [demonstrated](https://en.wikipedia.org/wiki/Intuitionistic_type_theory) that values can be unified with classical types into a single [dependent type theory](https://en.wikipedia.org/wiki/Dependent_type), thus showing that content and structure can be equivalent and share a single calculus, here we distinguish the two in order to speak a language that is more familiar to programmers.

The two domains of Intrinsic Data Integrity and Validation Rules, and their component dimensionality, amounts to what we might call a "Geometry of Certainty". Inside the clarity of such a geometry, the gradients of certainty become both much more visible, and much easier to build appropriately for. Thus it provides a context of agents being able to scale up their consent to play together in ways that meet their safety requirements. This is why we call our approach "Scaling Consent." It is what enables coherent collaborative action without first achieving global consensus.
