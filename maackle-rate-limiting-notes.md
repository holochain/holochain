
new workflow and new stage for rate limiting
update app validation to check for this stage
workflow pulls in consecutive sequences of activity ops that are sys valid (but still needs to have path to root)

query 1:
- filter: all activity ops
- filter: all that have been sys validated but not rate limited
- group by agent

query 2, per agent:
- pick min seq that hasn't been rate limited
- iterate (in-database) by seq ascending
- if a fork, pick one randomly, keep walking forward, pick the items that have the prev, to get a sequence of the full fork
  - (if multiple, can retrigger the workflow)
- ~~build the actual chain-linked sequence (following prev action hash)~~
