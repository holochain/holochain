# kitsune_dht_arc

ArcInterval subcrate for kitsune-p2p.

"DHT arcs" are continuous regions of the wrapping `u32` DHT location space. Each Kitsune Agent maintains its own storage arc, starting at the agent's location and extending "to the right" by some specified amount. This crate defines types for expressing these arcs, the logic for updating them over time, and intersections and union operations on sets of arcs.

License: Apache-2.0
