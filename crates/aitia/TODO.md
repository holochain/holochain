* [ ] Consider making earlier causes declare their implications rather than the other way around

Seems like earlier causes have the info necessary to construct later ones, so this could make more sense. Either way we can construct the entire tree. However if there are multiple starting points then all starting points need to be declared.

There is a theme where each fact is "about" something. Maybe instead of explicitly passing that something around, or transforming it, we can have the "subjects" be outside of the facts. For instance in Holochain, most facts are about an Op and a Node. Ops get introduced by authoring, and Nodes get introduced by initializing.

Maybe queries are actually done thusly: instead of fully constructing a fact, you specify the fact as well as the necessary subjects. So a fact about an Op and a Node needs to have both specified in full. This gets around the need to have different formats for different facts, like a full Op for authoring but only an OpHash for everything else. Introducing Subjects is a distinct action from logging Facts.

