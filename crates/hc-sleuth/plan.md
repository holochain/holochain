
A collection of facts, with connections between facts.
Facts can have multiple causes, combined with arbitrary boolean logic.

A fact check can pass or fail.

If it fails, we can generate two reports:
1. Single: Find the nearest cause which passes
2. Comprehensive: Find all nearest causes which pass
Let's just do Comprehensive so we get both

If the initial check passes, then we're good. As a followup step, we can run every check to see if there's a failure anywhere, even though there shouldn't be (this is good for the tool's test suite too).

If it fails, we want to find all nearest passing checks.
Optional: gather all causes into a graph. This deduplicates if there is any possibility of dupes.
BFS that graph. If any individual cause passes, terminate.
If an AND cause is encountered, we keep searching all paths upstream of any failed causes
If an OR cause is encountered, 

search the entire causal tree until we find a Pass at every path.
We check causes breadth-first.
If combined with AND, we can terminate after finding a single failure.
If combined with OR, we can only terminate if all fail.
If the cause(s) pass, check each of their causes in the same way.

If it passes, we should check all of our causes:
If combined with AND, we can only terminate if all fail