Holochain Design Overview: A Game Play Metaphor
===============================================

It may help to understand the design of Holochain through a well known
pattern of agentic collaboration: playing games.

Playing Games
-------------

People define the rules of a *Game* they want to play together. As
*Players* join the *Game* and start playing, they append new *Records* of 
their play to their own *Action* history. Then they update 
the *Game Board* by sharing the *Records* of their *Actions*[^chess] with
other players.

[^chess]: You can think of this somewhat like correspondence chess, but with
    substantial more formality.

The first requirement to create social coherence is ensuring that people
are playing the same game, therefore the very first record in every
Agent's history is the rules of the game by which they agree to play.
Obviously, Players are not in the same game or able to use the same Game
Board if they don't start with the same ruleset. These rules are the
actual computer code that is executed in running the Game, making moves
and validating the Actions of all Players.

System Description
------------------

We can describe the system as Agents, who play Games together by taking
Actions, the Records of which are held in a distributed Ledger that is
built by sharing these Records over a Network with other Agents. We
capitalize terms that comprise the ontological units of the system, and
which are formally described in the later sections.

### Agents

Agents have these properties:

1.  Agents are the only source of Actions in the system, thus Agents are
  the source of agency. All such Actions are uniquely identifiable
  as to which Agent took them, i.e. all Actions are signed 
  by public-key cryptography (see Actions below).

2.  Agents are uniquely addressable by other Agents.

3.  An Agent's address is its public key.

4.  Agents share Records of the Actions they take with other Agents through
  distributed storage so that those Records can be retrieved by
  other Agents reliably.

5.  Agents validate received Actions before storing them.

6.  Agents respond to requests for stored information.

7.  Agents can send messages with arbitrary content directly to other
  Agents.

### Games

Games have these properties:

1.  A Game consists of an Integrity specification with these parts:

    a.  A deterministic description of the structure of any data that is
  used to record some "play" in the game. Such data is called an
  Entry, where the act of generating such data is called an
  Action which is also recorded. Note: both types of data,
  content of the play (Entry) and the meta-data about the play
  (Action) when taken together are called a Record.

    b.  A deterministic description of the validity of the contents of
  an Entry along with the validity of an Agent taking any
  Action, i.e. some Actions may be taken in some contexts but
  not in others, as in making a move out-of-turn, or by an Agent
  not allowed to play a certain role.

    c.  A deterministic description of how Records may be related or
  linked to other Records.

2.  Along with the Integrity specification, a Game also consists of a
  Coordination specification. This specification contains
  instruction sets that wrap Actions into function call units and
  thus serve as an API to the Game. For example, for a blogging
  "Game" one such function call might be "create\_post" which takes
  a number of Actions that atomically create a number of Records to
  the Agent's Source Chain which include an Entry for the post as
  well as links relating the post to other Entries (see below for
  definitions of Actions and Source Chain).

3.  Each instance of the Game is played on its own Game Board which
  exists as a unique and independent network of Agents playing that
  Game, i.e. Games cannot interact with each other directly as all
  action in the system is only taken by Agents. Note that Games can
  be composed together, but only by groups of Agents all playing across 
  multiple games. This at first may seem like a weakness, but it's part of a
  key design decision that contributes to the system's overall
  design goals of evolvability. Essentially this creates the pattern of
  game-within-a-game.  For example a chess tournament is really two games:
  the game of "chess", and the game of "tournament".

In keeping with the metaphor of Game, we also refer to the Integrity
specification as the Validation rules of the Game.

We also refer to both the Integrity and Coordination specifications of a
Game as its DNA because this evokes the pattern of all the "cells" in
the social "body" as being built out of the same instruction set, thus
being the ground of self for that social body.

### Actions (and Entries and Records)

Actions have these properties:

1.  An Action has cryptographic provenance in that it is signed by the
  Agent that took the Action.

2.  Actions are Recorded in a monotonically temporally increasing
  hash-chain by the Agent that takes the Action. We refer to this as
  a hash-chain because each Action includes in it the hash of the
  previous Action, thus creating an untamperable Action history.

3.  Actions are addressed by the hash of the Action.

4.  There are a number of Action types:

```{=html}
<!-- -->
```
1.  CreateEntry: An Action for adding new Game-specific content. We call
  such content an Entry. The Entry may be declared as public, and
  will thus be published by the Agent to the network, or declared as
  private, where publishing is limited to just the Action data and
  not the content. Entries are addressed by their hash, and thus for
  CreateEntry Actions, this hash is included in it. Thus sometimes
  the Action is considered to be "meta-data" where the Entry is
  considered "data"[^headers] .

2.  UpdateEntry: An Action which adds new Game-specific content onto the
  chain that is intended to update previous content.

3.  MarkEntryAsDeleted: An Action which indicates a previous entry
  should be considered deleted.

4.  CreateLink: An Action that unidirectionally links one hash to another

5.  DeleteLink: An Action that indicates a previous link action should
  be considered deleted.

6.  A Record is just a name for both an Action and, when applicable, 
  its Entry, taken together. As an implementation detail, note that for 
  actions other than CreateEntry and UpdateEntry we don't need to 
  address the content of the Action separately, in which case the Record 
  contains no Entry and we simply retrieve all the we simply retrieve all the
  data we need from the recorded Action.

7.  Subsets of Agents can mutually consent to a single Action by
  atomically recording the Action in their history through
  Countersigning. Countersigning can also be seen as an affordance
  in the system for "micro-consensus" when that is necessary.

[^headers]: In many cryptographic systems hash-chains are thought of as having
    "headers" and "entries". Historically in Holochain development we
    also originally used that nomenclature, but realized that the name
    "header" comes from an implementation detail in building hash
    chains. Ontologically what's actually happening is that in building
    such intrinsic integrity data structures, not only must we record
    the "content" of what is to be written, but we must also record data
    about the act of writing itself, i.e. who is doing the writing, when
    they did so, and what they previously wrote. Thus, in keeping with
    the core ontology of agent-centricity we switched to using the term
    "Action" instead of Header, but we retain the name Entry for that
    which is written.

### The Distributed Ledger

The Ledger when seen systemically as a whole, consists of the collection
of all Records of Actions and their Entries in a Game that have been
taken by all the Agents together. The Ledger is stored in two distinct
forms:

1.  As self-recorded Source Chains of each of the Agent's Actions

2.  As a Graphing Distributed Hash Table that results from the sharing
  and validation of these Actions across Agents, collectively sharing
  portions of the data

The first form ensures the integrity of all data stored in the network
because it creates the coherence of provenance and ordering of local
state. The second form ensures the validity and visibility of that data.

Note, there is never a point or place where the entire state of the
ledger exists in one canonical place or location. It is always
distributed, either as the Source Chain of Actions taken by a single
agent, or broken into parts and stored after validation by other
participating Agents in the system.

#### The Ledger as Local State: Source Chain

An Agent's Source Chain for a Game consists of a hash chain of Records
of Actions taken by the Agent in accordance with the validation rules of
that Game.

A Record consists of an Action which holds context and points to an
Entry which is the content of the Action. The context information held
by the Action includes the Action type (e.g. create/update/delete/link,
etc) a time-stamp, the hash of the previous Action (to create the
chain), and the hash of the Entry.

The first few Records of every Source Chain - called Genesis Records - create a "common ground" for
all the agent's "playing" a Game to be able to verify the Game and its
"players" as follows:

1.  The first Record always contains the full Validation rules of the
  Game, and is hence referred to as the DNA. It's what makes each
  Game unique, and, as part of validation, always allows Agents to
  check that other Agents are playing the same Game.

2.  The second Record is a Game specific Membrane Proof, which allows
  Games to create Validation rules for permissioned access to a
  Game.

3.  The third Record is the Agent's address, i.e. its public key.

4.  The final Genesis Records are any Game specific Records added during
  Genesis followed by an "init-complete" Record indicating the end
  of the Genesis Records.

All subsequent Records in the Source Chain are simply the Actions taken
by that Agent. Note that Source Chains may end with a Closing Record
which points to an opening record in a new Game. 

#### The Ledger as Validated Shared State: Graphing DHT

After Agents record the Actions they take to their Source Chains, they
Publish these Actions to other Agents on the Network. Agents receiving
published data validate it and make it available to other agents to
query, thus creating a distributed database. Because all retrieval
requests are keyed on the hash of Actions or Entries, we describe this
database as a Distributed Hash Table (DHT). Because such
content-addressable stores create sparse spaces in which discovery is
prohibitively expensive, we have extended the usual Put/Get operators of
a DHT to include linking hashes to other hashes, thus creating a
Graphing DHT.

As a distributed database the DHT may be understood as a transform of
Agent's Source-Chain state into a form that makes that data retrievable
by all the other Agents for different purposes. These purposes include:

1.  Retrieval of Agent's Actions and created Entries

2.  Confirmation of "good behavior" by retrieving an Agent's activity
  history which is used to verify that agents haven't forked their
  chains

3.  Retrieval of link information

4.  Retrieval of validation receipts

To achieve this end, we take advantage of the fact that an Agent's
public key (which serves as its address) is in the same numeric space as
the hashes of the data that we want to store and retrieve. Using this
property, we can create a mapping between Agents responsible for holding
portions of the overall data by using a nearness algorithm between the
Agent's public key and the hash of the data to be stored. Agents that
are "close" to a given piece of data are responsible to store it and are
said to comprise a Neighborhood for that data. Hashing creates an
essentially random distribution of which data will be stored with which
Agents. The degree of redundancy of how many Agents should store copies
of data is a per-Game parameter.

Agents periodically gossip with other Agents in their Neighborhood about
the published data they've received, validating and updating their
Records accordingly. This gossip ensures that eventually all Agents
querying a Neighborhood for information will receive the same
information. Furthermore it creates a social space for detecting bad
actors. Because all gossiped data can intrinsically be validated, any
Agents who cheat, including by changing their (or other's) histories,
will be found out, and because all data includes Provenance, any bad
actors can be definitively identified and ejected from the system.

See the [Formal Design Elements](hwp_4_formal.md) section for more information
on how we publish convert Source Chain data and publish it into the collectively stored data
on the DHT, and how this works to provide eventual consistency, and the
sections in [System Correctness](hwp_3_correctness.md) details on detection of
malicious actors.
