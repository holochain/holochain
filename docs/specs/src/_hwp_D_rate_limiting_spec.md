# Appendix D: Rate Limiting

## Context

### Problem: DHTs are a public good so people can take advantage by spamming data

* Flood the network with garbage creating degraded service for other users
* Force users to hold garbage data indefinitely locking up storage
* Force authorities to verify garbage data thrashing CPU
* Fill sequential logic causing delays for back of the queue

### Solution: Rate limits

* Actions have A units of weight
* Apps can define their own weights in-wasm for app entries
* System entry weights and rate limiting is defined by the system
* A bucket of B units may fill to allow bursts of activity
* Every X millis Y units is restored to the bucket
* There are many buckets definable by the happ to tailor rate limits to different usage patterns for different system components

### Prior art:

- AWS has a [reasonable bucket rate limit](https://docs.aws.amazon.com/apigateway/latest/developerguide/api-gateway-request-throttling.html), every resource/account has tailored rate limits
- Rust libs such as [this](https://docs.rs/throttle/0.1.0/throttle/) and [this](https://github.com/visig9/slottle)

### Why do this in Holochain core?

There are several reasons to do this in Holochain's core and not assume/rely on applications to be doing this for themselves:

- The system needs to implement limits anyway, so this is more about exposing key parameters to the wasm for something that is already happening anyway, in order to make it more effective
- The only way an app could do this is to build a heavy validation package for every entry and have the authorities calculate the rate limit from that, whereas (see below) we can have the rates calculated efficiently by the headers only on the agent activity neighbourhood
- Providing a consistent interface that will address 90%+ of app developer needs in a couple of simple callbacks should help eliminate app bugs
    - Bugs in this logic can easily cripple an app / DHT
    - This type of logic SHOULD be implemented for EVERY app to protect from network spam
- If core handles this then we know it will be implemented in such a way that an honest agent never accidentally exceeds the rate limit
-

### Sybils

Important note that rate limits do nothing to protect against sybils.

A sybil attack involves generating many agents, _each of whom have their own rate limit_.

As an attacker, if I can create S sybils every Z millis then my Y/X throughput becomes `S/Z * Y/X` throughput, which is probably a lot more than the network can handle if `Z` is small and `S` is large.

Rate limits allow each network to express and enforce its own "fair use policy" for _non-sybil_ agents.

Applications SHOULD implement anti-sybil measures, but rate limiting is NOT a substitute for implementing a proper joining membrane.

That said, a rate limit coupled with an effective sybil mitigation strategy may mitigate certain damage caused by temporary/limited breakdowns in that defense more effectively than the anti-sybil measure alone.

## Specification

### Weight

Broadly we have:

- Class A: System elements that MUST be allowed to happen, and are finite so cannot be abused
    - DNA
    - AgentValidationPkg (joining proof)
    - InitZomesComplete
    - OpenChain
    - CloseChain
    - Create: Agent
    - Delete: CapGrant
    - Delete: CapClaim
- Class B: Access grants that SHOULD be allowed to happen, but are infinite/abusable, the app DOES NOT control the weight
    - Create: CapGrant
    - Create: CapClaim
    - Update: CapGrant
    - Update: CapClaim
- Class C: Links are only headers but still infinite/abusable, the app MAY provide a weight
    - CreateLink
    - DeleteLink (bounded by existant CreateLink)
- Class D: CRUD are headers and entries (mostly) so are very abusable, the app SHOULD provide a weight
    - Create: App
    - Update: App
    - Delete: App

#### A: System mandatory: Critical

Class A elements are considered weightless by the system, i.e. weight 0.

Even though these elements may be arbitrarily expensive to process, the overall system relies on them to function. If a Class A element was rejected due to rate limiting the system could be put into an unrecoverable state as it would be unclear how/when to back off and retry these processes.

Importantly these elements already cannot be spammed by their nature, they do not require additional rate limiting. For example, DNA can only exist once by definition.

Notably the deletion of CapGrants and CapClaims is never rate limited:

- Deletion is always 1:1 with a Create/Update so rate limited creation implies rate limited deletion
- Rejecting the revocation of access would imply forcing the agent to accept incoming connections they explicitly denied, this is unacceptable for a secure system

However, the deletion entries are private so the network has no way of knowing _which_ deletes line up with what create/updated.

The network does know that in aggregate the following invariant must hold true:

`(deletes + updates) <= (creates + updates)`

Because:

`update = create + delete`

`deletes <= creates`

At the limit where everything is deleted:

`deletes + update-deletes = creates + update-creates`

And each `update-delete` and `update-create` is a single entry therefore:

`(deletes + updates) <= (creates + updates)`

The agent activity neighbourhood can enforce this invariant for each agent.

Note: This is only true of the special case of cap entries as it is invalid for a cap Update/Delete to reference an entry on a different chain so we have a bounded linear history. In the general case entries _can_ reference other chains so the above invariant does not hold. See below for how other entries are handled.

##### Note re: Agent

Many Agents can be created unlike the other Class A entries.

The rate limiting here is implied because all Agent keys are referencing entries in the `deepkey` happ.

Deepkey itself uses rate limiting as specced in this document to limit the rate of Agent entries, so implicitly this is Class A _and_ has rate limit logic.


#### B: System priority: Security

Class B elements are access control so considered critical to the system and have high priority but are abusable.

Access control changes are ideally never rate limited:

- False positives leading to inappropriate rate limiting could be very harmful to the user
- These elements are private entries so the overhead to the network is only the headers (e.g. assigned grants cannot flood the network with garbage keys because this lives in the private entry)
- Still, there must be _some_ limit, the network cannot accept infinite headers

The _rate_ of cap grants is limited but weight and size (common to all entry headers) are ignored.

As this is entirely a question of network health, a happ should never be intentionally (un)limiting access controls. The validation can be done by the system and it is not configurable.

That said, there is diminishing returns in the marginal utility of allowing more cap grants per-agent. As long as an agent can manage their access, headroom adds zero value to users but non-zero additional liability to the network.

We can set a large bucket with a slow recharge so that an abuser hits the bucket limit after many creates but before overwhelming the network then is stuck sending a few dozen bytes a minute or similar. Normal users will never hit the bucket limit.

Notably the update of CapGrants and CapClaims _is_ rate limited:

- Which may be a surprise/concern as an Update is a Delete (see above)
- But an Update is also a Create, which makes it unbounded

**Question: How much is "too much" for access control headers sent to an arbitrary network?**

**Corollary: How little is "too little" for an arbitrary happ with arbitrary but reasonable requirements?**

#### C: Low risk: Links

Class C elements are links created by the application.

These are relatively low risk to the network because:

- They are only headers
- They have bounded size:
    - link tag is max ~500 bytes
    - base/target are both ~32 bytes each
    - entry type is link
- Deletes are 1:1 with a Create so we can think of a Create/Delete pair as a single "thing" to be concerned about

Technically 2+ agents can delete the same link creation even though a delete is a tombstone. A single agent MUST NOT be able to delete the same link twice, this is directly enforceable (unlike delete access) by system validation and the network because link deletion is a public element. (remember that rate limits are not a sybil defense)

But still, we cannot accept infinite links, so there must be some limit.

Further, an application may want to restrict links much more than the network would be able to accept technically. It may simply create a poor user experience to allow link spam.

As link _validation_ can be arbitrarily heavy on compute, the application SHOULD assign a high weight to links that will trigger expensive validation.

The system will enforce a maximum _rate_ of link creation which is _half_ the safe maximum the network could technically accept, under the assumption that every create implies a delete.

Creation of a link will be rejected if _either_ the system rate limit is hit _or_ the application weight limit is hit.

Deletion of a link will _only_ be rejected if the application weight limit is hit.

Unlike access controls, there is no assumption/intuition that there could be "enough" links for all valid applications. There are many use-cases (e.g. indexing large data sets for search) that would appreciate as much as we can squeeze out of the network.

**Question: What is the "safe maximum" for links?**

**Question: Should the "safe maximum" be configurable per-DHT to allow for dedicated hardware to opt-in to heavily linked networks?**

#### D: High risk: CRUD entries

Class D elements are CRUD elements created by the application.

These are high risk to the network because:

- The system knows nothing about their use/intent
- They have unbounded size
    - Well, technically 16mb per entry but this was a somewhat arbitrary decision to match websocket limits rather than a calculated rate limiting/defense
    - wasm itself goes up to 4GB in memory, so in theory a compressed entry that is 16mb could be uncompressed to something much larger during processing
    - This only applies to Creates/Updates as Deletes are only a reference/tombstone with a known size
- There is no 1:1 relationship with Updates and Deletes to a Create, there is an arbitrarily branching tree of Updates with Deletes at the leaves
- We expect relatively heavy validation/compute for CRUD elements

Entries are the whole point of the network so we have to be as permissive as is reasonable. Entries are also the heaviest and most arbitrary things on the network so are also the biggest liabilities.

The system can set a baseline limit of both the _rate_ of entries and _size_ in bytes of data that the network can safely handle.

Applications SHOULD SIGNIFICANTLY restrict entries beyond this theoretical maximum with strict weights.

There is also an invariant for all entry deletes as per deleting links and capgrants etc. that a single agent can only delete any given entry once. System validation MUST ensure deletion is unique per agent/entry combination.

Applications SHOULD identify opportunities to bound entry data:

- Use fixed arrays etc. instead of open ended data structures
- Set absolute caps on how many entries of a type can be created, if appropriate
- Use references to instead of copies of data, along with appropriate dependency resolution in validation
- Invalidate all entries that do not add value, do not simply allow meaningless cruft to accumulate

Applications MUST set weights with the expectation that bad actors may tamper with timestamps etc. in order to squeeze out whatever wiggle room they have, so assume many users spamming using the maximum rate limit over a long period of time.

Applications MUST consider network, storage and CPU usage when setting weights.

Applications MUST consider that storage is permanent, and network and validation overheads replay via. gossip as network topologies shift.

**Question: What is the "safe maximum" for both rate and size of entry data?**

**Question: Should the "safe maximum" be configurable per-DHT to allow for dedicated hardware and niche use-cases?**

Let's consider a bucket with 100MB capacity with a drain rate of 1MB/sec

### Buckets

A rate limit bucket is a technique to allow for _stricter_ rate limiting over long periods by facilitating _looser_ "bursts" over short periods.

The bucket starts empty and has a fixed size B, e.g. 100 units. Each time a user performs an action they fill A units of their bucket, e.g. 5 for a like and 10 for a comment. Every X milliseconds the bucket empties Y units from itself. When the bucket is full no more actions can be performed, they are rejected by the system. When the bucket is empty no more emptying can be performed, i.e. it can never have more than B (100) units of space.

The intuition is that for many use-cases, an honest human user will interact heavily with the system while actively using the software and then mostly or completely stop while they do something else. A bot/script on the other hand will be persistent and try to abuse the network 24/7/365. A malicious user may try to manipulate timestamps to squeeze out a few extra actions.

Bots and timeswizzlers will tend towards the theoretical maximum of Y per X millis worth of actions being spammed. Therefore Y/X should be as conservative as the app can bear without unreasonably degrading the user experience.

Humans can benefit from a 10x, 100x or even 1000x+ bucket B than the baseline Y/X for actions that are spikey in nature.

#### Many buckets

The appropriate size and recovery speed of a bucket is contextual.

The bucket algorithm is a simple and computationally efficient way to achieve deterministic limiting amenable to our validation.

There are many buckets available to a happ, each with its own bucket index/position/id.

These are configured with a callback analogous to the entry defs callback.

```rust=
struct RateLimit {
    bucket_max: u32,
    units_per_drain: u32,
    millis_per_drain: u32,
}

#[hdk_extern]
fn rate_limits() -> Vec<RateLimit> {
 // ...
}
```

Class C and D elements can be assigned to an arbitrary rate limit bucket upon creation by the `weigh` callback (below).

Class A and B and system rate limiting ignores happ buckets and has their own internal limit tracking.

#### Bucket normalization

Each bucket MUST normalize the units of weight such that element weights will always fit inside a `u8`. This is to minimise the data required in the headers to represent rate limit logic.

By allowing many bucket definitons the intent is that happ developers can define min/max values for many different components of the application to ensure `u8` normalized weights are meaningful.

For example, a single entry type that holds image files could be:

- A single 1x1 pixel square for a tiled background
- A small vector
- A small lossy jpg
- A large lossless png

The range between the smallest and largest image could be 10_000x or more, clearly far too large a range to represent in a `u8`.

However, this app may have a 1MB image size limit (much less than the 16mb raw entry size limit).

In this case the app can set aside a bucket for images where the `u8` weight unit represents a ( 1MB / 255 ) range of image sizes. The difference of 1 unit of weight is 4kb of data.

We SHOULD probably write a nice HDK function to help normalize `u32` data within a range to a `u8` as this is the most common measurement of things (e.g. `usize` is `u32` in wasm, so all `.len()` calls are too).

The intuition is that the more accurately the happ can model different rate limits within a `u8` for weight and `u8` for bucket ID, the _stricter_ an application can be re: rate limits. For only 2 bytes of overhead per element, without degrading end-user functionality.

### Validation

Enforcing the rate limit is split between the agent's activity neighbourhood and the entry authorities.

#### Callback

We do NOT want to add additional complexity to the existing validation callbacks, that are slated for potential future refactors already (to make simpler).

We do NOT want to add logic to the entry/link commit function call directly because this would introduce the possibility for diverging/inconsistent weights, and would complicate a major workhorse function.

We can add a `weigh` callback that accepts either a `CreateLink` header or `Entry`:

```rust=
enum WeighInput {
    Link(CreateLink),
    Entry(Entry),
}

struct WeighOutput(BucketId, Weight);

struct BucketId(u8);

struct Weight(u8);

#[hdk_extern]
fn weigh(weigh_input: WeighInput) -> WeighOutput {
    match weigh_input {
        Link(create_link) => ...,
        Entry(entry) => ...,
    }
}
```

This way the weight logic can be centralised in the happ, and the honest Authority and Agent are guaranteed to agree on weights regardless of how the link/entry creation is handled.

The return of `weigh` is both the bucket to assign the weight to and the weight.

This means the weight is only meaningful in context of the returned bucket. For example, a weight of `100` in bucket `1` could be _more_ permissive than a weight of `5` in bucket `2` if the former has higher throughput.

##### Circular weigh logic

!There is a bit of circular logic in the above example!

How can a `create_link` header be passed in to the `weigh` function if the result of `weigh` is going to be included in the `create_link` header??

I think ultimately this is an implementation detail that can be nutted out while coding it.

A few (non-mutually-exclusive) options to resolve this:

- Not actually pass in a literal `CreateLink` header but instead pass in a more limited struct that only has base/target/tag (probably the preferred approach atm)
- Have the happ wasm short-circuit the host when creating a link/entry similar to how `entry_type!` macro directly calls the `crate::entry_defs` function, and pass the weight to host on creation, in addition to the `weigh` callback triggering during validation (maybe do this anyway, but not sure if needed)
- Populate the header with default options and have the `weigh` function _transform_ an input header to an output header (not very idiomatic for us, we don't do this anywhere else, but it is "functional" style)


#### Agent

The agent:

- Has full access to their own chain
- Should use an accurate clock but may be a timeswizzler
- Has the same rate limit algorithm as everyone else

As the agent has all their headers and calculate their own timestamps and weights, they know their rate limit at all times and have zero excuse for exceeding it.

If the agent commits an entry to their chain that violates the rate limiting rules, that chain is permanently/irreversibly broken.

The agent neighbourhood will keep the over-limit header as proof of bad behaviour. The agent cannot move past this point with _any_ new headers, and cannot fork/rollback their chain as this is also disallowed.

The happ logic will need to return a new possible `ExternResult` that can fail for all writes (likely a host error as a string for now, but a proper enum variant in the future) due to rate limiting.

#### Agent neighbourhood

The agent's neighbourhood:

- Receive all headers authored by the agent (the whole chain)
- Can refuse to progress the agent's chain until they are satisfied with existing headers
- Can compare their own timestamps against the agent published timestamps to mitigate timeswizzling
- DO NOT validate elements

All the header weights can be taken from the headers and used in the rate limit calculation. The bucket status as at any given header can be calculated efficiently (without network requests or validation logic) and deterministically by iterating from that header and comparing timestamps to weights.

Note this means the authoring agent sets the timestamp, so detecting bad timestamps is NOT something that can be done at this layer of validation. It SHOULD be attempted to mitigate bad times elsewhere in validation.

Using the timestamps as per the authored, signed, hashed header gives us a much stronger proof of malicious intent in the case that a rate limit violation is detected.

The rate limit weight and timestamp are both included in the header hash so manipulation is equivalent to a chain fork.

The agent neighbourhood DOES NOT have the ability to validate that the weight is correct where set by the app. The authorities need to independently verify this.

#### Element Authorities

Authorities:

- DO NOT have local access to all headers/the author chain
- Can reject an individual element in isolation

Authorities cannot efficiently calculate whether an element is rate limited and they should NOT try to do so over the network.

The authority MUST validate that the app-defined weight is correct for all links and crud entries on an individual basis.

As long as the weights are correct the agent neighbourhood can handle the actual rate limit enforcement.

### Resource usage and Header changes

Adding data to the Header is something to be very careful about:

- Headers are cloned a lot throughout internal workflows
- Headers are included in a lot of network traffic

Much more so than entries are.

The main goal is that the rate limiting logic is simple and powerful enough so that devs can use it "drop in" 90% of the time and achieve 10x or more OOMs worth of resource limiting than the header bytes consume.

Said another way, the expected efficiency improvement of the vast majority of happs due to rate limiting, as a global, emergent optimisation should outweigh the additional linear per-header cost by at least 10x+.

Consider 3 cases:

- No rate limit in core
- Naive limit across all entries equally or per-type
- Individually weighted element level limit

#### No throttling

This is the current situation.

Any data limits are "enforced" by the GUI which is not really an enforcement.

The only reason this works is because we are doing limited test runs with semi-trusted participants.

In reality it's super easy for someone to write a bot that floods any DHT and breaks it.

#### Naive rate limiting

Naive rate limiting is anything that we can determine either at compile time for the app or as a once-off/lazy runtime callback (e.g. per entry type).

We basically set a linear rate limit of the _number_ of elements.

For example:

- The system can set a limit of items based on gossip limits on the network
- The app can set a more strict limit per-entry type
- Links can also have a more strict limit set according to the type of the base entry
- The rate limit of items implies LIMIT x 16mb max entry data

Pros:

- No additional Header data
- Super simple
- Predictable rate limiting

Cons:

- No ability to rate limit based on per-entry considerations
    - Have to assume worst case for every element which means we either need to be looser than necessary on bad actors or stricter than we'd like on good actors
    - No ability to rate limit _data_ based on serialized entries, so we cannot for example allow users to burst a few large entries
- No ability to model domain-specific usecases from information within an entry
- Not expressive at commit/runtime, so I expect many happs to roll their own rate limiting because the core implementation would be underpowered for a lot of use cases (anything where the rate is informed by the entry content), so ultimate effect is to complicate/diverge happ code

#### Per entry rate limit

In this case we make a judgement call about each element as to how much of the bucket it consumes. This is all as described above.

Some changes to headers are required.

There are two `u8` values needed to represent app weight and bucket.

A single `u8` value is needed to represent system measurement of the serialized entry data as bytes, normalized from 0..16mb as per standard bucket normalization.

Some changes to headers are needed:

- Class A: No change, rust can derive `0` from type information
- Class B: Same changes as Class D as cap grants/claims are entries
    - `rate_bucket` is always `255`
    - `rate_weight` is always `0`
    - `rate_bytes` is always `0`
    - Bucket, weight and bytes are ignored for cap grants/claims as they have their own rate limiting logic
- Class C: Links don't need a `rate_bytes` but do need weights
    - An application defined `rate_bucket` and `rate_weight` both as `u8` added to link create headers by the `weigh` callback
    - no change to delete headers
    - No `rate_bytes` data
- Class D: Entries need `rate_bytes` and also weights/buckets
    - A `rate_weight` field as `u8` added to `Create`, `Update`, `Delete` headers, as anything the application wants
    - A `rate_bytes` field as `u8` added to `Create`, `Update`, `Delete` headers

Pros:

- Maximum ability/expressivity for happ devs to lock down rate limits
- Relatively minimal additions to HDK, only 2x simple global callbacks
- Additional data in headers can be as little as 3 bytes per header
- Expressive enough that happ devs should (i hope) rarely need to roll their own rate limits, so overall effect is to simplify/standardise happ code
- Allows links to be rate limited on their tag and target, not just inherit from the base

Cons:

- Additional data in headers
- Additional callbacks
- More complex than naive option

