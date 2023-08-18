## more thoughts

The whole point of Actions is to be able to refer to function calls declaratively.
With the new proc macro, we don't need to have a literal transition function. We can still just have a bunch of functions which mutate state. The proc macro can generate enums which trigger function calls. Same difference. The "real" version would just be a straightforward implementation. The "action recording" version would be a wrapper around that. Eh, but actually maybe it's fine the way it is.

On the Effect side though, we don't need to fuzz the effects. The important thing is that we can ignore effects, and otherwise manipulate them. One benefit of using an enum is it enforces how effects are expressed. If a fn can only return one effect, then we can't do the equivalent of calling many functions -- we can only do the equivalent of calling a single fn.

What are the invariants again?
- Transitions are completely deterministic. Given a state of the system, the same input always changes the state in the same way.
- The effects produced by a state change are purely deterministic. Given the same state change, the same effect is always produced.
- Neither a transition nor an effect cannot make use of any changing value outside of the state! They can make use of the constant "params" in a ParamState, but can't do things like read a database or anything else outside of the state.
- Also, a given piece of state cannot be mutated by more than one state machine. There should be only one state machine per state. State cannot change between transitions.

### try_initiate

check target expired

state:
- gossip_type
- tuning_params


## Incoming

### initiate

input:
- msg:
    - arc set
    - tiebreaker
    - agent list
- db:
    - get local arc set
    - get local agents

mutation:
- reset initiate tgt if tiebreaker
- record metrics:
    - update current round
    - record accept
- calculate common arc set
- calculate diff (agent or op)
- set common arc set
- set stage = AwaitingAccept (or so)
- add new round to mux (DEFERRED)

effect:
- SEND accept
- SEND diff (agent or op)

new round:
- common arc set
- region set
- remote agents (needed?)

### accept

input:
- msg:
    - arc set
    - agent list
- db:
    - get agent info

mutation:
- record metrics: 
    - latency
    - update current round
    - record initiate
- calculate common arc set
- calculate diff (agent or op)
- set common arc set
- set stage = ExchangingAgentDiff
- add new round to mux (DEFERRED)

effects:
- SEND diff (agent or op)

new round:
- common arc set
- region set
- remote agents (needed?)

## agent diff

input:
- msg:
    - bloom
- db:
    - get agent info

mutation:
- set stage = ExchangingAgentData

effect:
- SEND missing agents

# agent data

input:
- msg:
    - agent data

mutation:
- set stage = ExchangingOpDiff

effect:
- put agent info
- SEND op diff

# op diff

input:
- msg:
    - diff
    - queue id (needed??)

mutation:
- record metrics:
    - update current round
