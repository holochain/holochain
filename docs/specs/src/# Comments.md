# Comments

from commit `40651179`

## hwp_1_intro.md

* L166 and L185: Validation rules are a part of intrinsic data integrity, as pointed out on L209.
* L170 - L172: Structure and content are essentially the same thing -- how do you semantically and rationally make sense of a blob. But then those are probably two separate things in people's minds because dependent/constrained types aren't really a thing except in Idris and SQL.
* L189 and L197: currently the network message signing key is separate from the agent address, which only signs ops and RPC payloads -- IOW, contents of some messages, but not the message itself.
* L218 - L219: a geometry of certainty feels at odds with a 'gradient' of certainty, which I assume is alluding to the gradient of _confidence_ outside, or in a separate plane from, the bounds of the space defined by the geometry of certainty.

## hwp_2_overview.md

* L66 and L73: Both seem to be saying the same thing. Structure/contents of an entry and an action. Suggested rewrite:
    ```pandoc
    a.  A deterministic description of the content and structure of any data that is
  used to record some "play" in the game. Such data is called an
  Entry., where the act of generating such data is called an
  Action, which is also recorded. Note: both types of data,
  the content of the play (Entry) and the meta-data about the play
  (Action), when taken together are called a Record.

    b.  A deterministic description of the validity of the context in which an Entry is written.
  The act of generating of an entry is called an Action, which is also recorded.
  E.g., some Actions may be taken in some contexts but
  not in others, as in making a move out-of-turn, or an Agent
  not being allowed to play a certain role.
    ```
* After L80: add this:
    ```pandoc
   d. A deterministic description of the validity of the act of updating or deleting Actions and their corresponding Entries or Links; e.g., an Action may only be updated or deleted by its author, or a Link may only be deleted if its timestamp is less than one hour before the Action that deletes it.

   e. A deterministic description of the validity of an Agent's claim to be permitted to join a Game. This claim is called a Membrane Proof, and can cointain such data as a secret password or a signed certificate from another Agent who has the authority to admit them to the Game.
    ```
* L108-L109: We're swiftly moving away from the idea of a DNA encompassing a coordinator.
* L129-L131: What is this HTML code block with a comment in it?!
* L132: Does this actually create an indented list?
* L152: Add this:
    ```pandoc
   6.  InitZomesComplete: An action which announces that the Agent has completed any necessary preparations and is ready to play the Game.
   7.  AgentValidationPkg: An action which presents an Agent's Membrane Proof, or the data that proves that they have permission to join a Game.
   8.  Dna: An action which contains the hash of the DNA's code, demonstrating that the Agent possesses a copy of the rules of the Game and agrees to abide by them.
   9.  OpenChain: An action which indicates that an Agent is continuing their participation in this Game from another Source chain or an entirely different Game.
   10. CloseChain: An action which indicates that an Agent is ending their participation in this Game, and may be continuing their participation in another Source Chain or an entirely different Game.
    ```
* L199: It could be argued that there's nothing preventing one agent from attempting to collect all state and make it into something coherent, but canonicity doesn't exist as an intrinsic property of the system.
* L209-211: Turn it into a bullet list, add any important fields not mentioned
* L268: I changed hash to address, because some ops have an address that is different from the hash of their data.
* L273: Not a per-game parameter -- yet. Will it become one?

## hwp_3_correctness.md

* L1: remove blank line
* L13: 'Intrinsic integrity' has so far been used to describe deterministic self-validating integrity. Here it seems to be used more generally for all kinds of provable integrity, incl ability to reach consensus?
* L20: I'm just assuming this is what's being said here. Otherwise, not sure how it differs from fault tolerance.
* L43-44: The signatures are _not_ included in the hash chains anymore; in Holochain-Rust only the entry was signed, which meant there was never any provenance checking on the chain itself. Now anything that needs provenance is wrapped at transport time in a `SignedHashed<T>`.
* L65: "or, alternatively, to simultaneously publish two Actions that share a parent."
* L69: "The only hope for a malicious Agent is to successfully engineer a partition in which their victim cannot see the other branch of the fork."
* L77-79: not yet
* L90: A hyperlink would be great
* L93: extra newline
* L95: What's the thing we want to cite? a definition of Byzantine faults?
* L100: it'd be great to make this hyperlinkable for L90
* L110-111: seemed to make more sense to drill down to ever smaller circles of participants
* L142-144: Revise to something like: "There's no need for global time of
  transactions when each is clearly ordered by its sequence in the
  chains of the only accounts affected by the change. The connections between source chains formed by countersigning means that each transaction is at the tip of a causal graph of events that is sufficient to prove the transaction's validity."
* L171: Point E doesn't feel like a use case; it feels like a recap.
* L179-181: Another one of those weird html code blocks with a comment.
* L183-186: Kinda convoluted. I think it should follow this sequence:
    1. Agent-centric approach
        * parallel state changes
        * parallel integration of ops
    2. sharding for reduced load on any peer
        * parallel gets
    2. parallel integration (AP system)
    3. parallel gets (AP again)
* L191-211: I've made changes but I still feel like it's unclear and depends on a deep knowledge of the subject.
* L236-238: How does Holochain do either of these things?
* L239: add "Allowing agents to block communications from other agents, which can be used to implement individual or collective banning of agents who produce data whose acceptability cannot be evaluated by a deterministic function"
* L240: this header breaks with the previous Markdown style; I'd like to make the formatting consistent and idiomatic across the entire doc.
* L282: not sure if the warrant goes to the neighbours. Still up in the air AFAIK (see sticky warrants)
* L285: majority consensus isn't about validation; it's about making alternative possibilities go away. Validation happens prior to consensus. (Although I guess it could be argued that consensus sort of proves an action's validity, as other validators will reject a winning block that fails validation.)
* L293-305: sounds prejudiced and a little sarcastic.
* L302-304: conflates two kinds of coherence -- validity vs finality. Rewrite to focus on finality via saturation vs consensus.
* L307 onward: mention risks that Sybil attacks present, in addition to eclipse attacks:
    * validating invalid data
    * spurious warrants (similar/subcategory of spam attacks?)
    * information hiding (only 100% possible via eclipse)
    The consequence of the first two is that the immune system to efficiently neutralise threats is compromised, forcing everyone to drop to 0-of-n trust. The third creates temporary or total partitions, preventing a victim from discovering chain forks
* l354: I don't know that any of these are actually implemented?

## hwp_4_formal.md

* L23: another comparison to blockchains, and it gets it wrong -- blockchains do the same thing as what we claim Holochain does, which is validate things according to shared rules. The 'unlike' part is that Holochain does global visibility of local state.
* L38: distinction needs to be made about _what_ subset of the DNA gets hashed
* L53: data can be checked using the agent key, but network messages use another keypair. Some network messages do add a signature, but not all of them do.
* L91-92: Coordinator DNA