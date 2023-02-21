
[^1]: Some readers may come to the problems of distributed coordination
    from the framework laid out by the literature on Byzantine Fault
    Tolerance. These axioms and a discussion of why start with them are
    explained in our paper The Players of Ludos: Beyond Byzantium
    \[LINK: [[The Players of
    Ludos]{.underline}](https://docs.google.com/document/d/1HBNgIooElD5widCuX9XmiOzbVIpEF5XXH67mZbnUFjo/edit#)\].

[^2]: Antifragile: Things that Gain from Disorder. Nassim Nicholas Taleb

[^3]: In Reaching Agreement in the Presence of Faults:
    [[https://dl.acm.org/doi/pdf/10.1145/322186.322188]{.underline}](https://dl.acm.org/doi/pdf/10.1145/322186.322188)
    MARSHALL PEASE, ROBERT SHOSTAK, and LESLIE LAMPORT, this single data
    reality is called "interactive consistency" as is about the vector
    of "Private Values" sent by each node.


[^4]: You can think of this somewhat like correspondence chess, but with
    substantial more formality.

[^5]: In many cryptographic systems hash-chains are thought of as having
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


[^6]: CITATION NEEDED

[^7]: https://en.wikipedia.org/wiki/Object-capability\_model

[^8]: We, Neighborhoods, Ad4m (https://ad4m.dev/) \[TODO: insert links
    here\]