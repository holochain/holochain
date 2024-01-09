## todo

### short term

* [X] ~~*Need to make the graph make more sense during a normal failure:*~~ [2023-11-08]
    - If there are any failures, make the graph show only the paths which lead to true facts
    - Render the true facts differently from the false ones, even if only by showing a little check mark
    - There's probably no point in showing the graph if there are no truths
* [ ] When the graph terminates in an Any or Every, it would be classy to show the underlying Facts which satisfied it


### mid term

* [X] ~~*Add a "passing" mode where we expect the fact to pass, and we validate the entire tree, and add this to a bunch of unit tests*~~ [2023-11-08]
* [ ] Maybe see entire chains of real events related to a fact. For instance if an op is integrated, see all the times validation was attempted, not just the first.

### future

* [ ] Consider making earlier deps declare their implications rather than the other way around

## limitations

- Must only use one dna hash at a time
- aitia::trace can only handle one type of fact (per serde)


Seems like earlier deps have the info necessary to construct later ones, so this could make more sense. Either way we can construct the entire tree. However if there are multiple starting points then all starting points need to be declared.

There is a theme where each fact is "about" something. Maybe instead of explicitly passing that something around, or transforming it, we can have the "subjects" be outside of the facts. For instance in Holochain, most facts are about an Op and a Node. Ops get introduced by authoring, and Nodes get introduced by initializing.

Maybe queries are actually done thusly: instead of fully constructing a fact, you specify the fact as well as the necessary subjects. So a fact about an Op and a Node needs to have both specified in full. This gets around the need to have different formats for different facts, like a full Op for authoring but only an OpHash for everything else. Introducing Subjects is a distinct action from logging Facts.

