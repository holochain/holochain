# holochain_trace

## Structured Contextual Logging (or tracing)
### Why
[Watch](https://www.youtube.com/watch?v=JjItsfqFIdo) or [Read](https://tokio.rs/blog/2019-08-tracing/)

### Intention of this crate
This crate is designed ot be a place to experiment with ideas around
tracing and structured logging. This crate will probably never stabilize.
Instead it is my hope to feed any good ideas back into the underlying
dependencies.

### Usage
There are a couple of ways to use structured logging.
#### Console and filter
If you want to try and filter in on an issue it might be easiest to simply log to the console and filter on what you want.
Here's an example command:
```bash
RUST_LOG='core[a{something="foo"}]=debug' my_bin
```
Or a more simple version using the default `Log`:
```bash
RUST_LOG=trace my_bin
```
##### Types of tracing
There are many types of tracing exposed by this crate.
The [Output] type is designed to be used with something like [structopt](https://docs.rs/structopt/0.3.20/structopt/)
so you can easily set which type you want with a command line arg.
You could also use an environment variable.
The [Output] variant is passing into the [init_fmt] function on start up.
##### Filtering
```bash
RUST_LOG='core[a{something="foo"}]=debug'
```
Here we are saying show me all the events that are:
- In the `core` module
- Inside a span called `a`
- The span `a` has to have a field called `something` that is equal to `foo`
- They are at least debug level.

Most of these options are optional.
They can be combined like:
```bash
RUST_LOG='[{}]=error,[{something}]=debug'
```
> The above means show me errors from anywhere but also any event or span with the field something that's at least debug.

[See here](https://docs.rs/tracing-subscriber/0.2.2/tracing_subscriber/filter/struct.EnvFilter.html) for more info.

##### Json
Sometimes there's too much data and it's better to capture it to interact with using another tool later.
For this we can output everything as Json using the flag `--structured Json`.
Then you can pipe the output from stdout to you're file of choice.
Here's some sample output:
```json
{"time":"2020-03-03T08:07:05.910Z","name":"event crates/sim2h/src/sim2h_im_state.rs:695","level":"INFO","target":"sim2h::sim2h_im_state","module_path":"sim2h::sim2h_im_state","file":"crates/sim2h/src/sim2h_im_stat
e.rs","line":695,"fields":{"space_hashes":"[]"},"spans":[{"id":[1099511627778],"name":"check_gossip","level":"INFO","target":"sim2h::sim2h_im_state","module_path":"sim2h::sim2h_im_state","file":"crates/sim2h/src/s
im2h_im_state.rs","line":690}]}
```
Every log will include the above information expect for the spans which will only show up if there are parent spans in the context of the event.

You can combine filter with Json as well.

###### Tools
Some useful tools for formatting and using the json data.
- [json2csv](https://www.npmjs.com/package/json2csv)
- [jq](https://stedolan.github.io/jq/)
- [tad](https://www.tadviewer.com/)

A sample workflow:
```bash
RUST_LOG='core[{}]=debug' my_bin --structured Json > log.json
cat out.json | jq '. | {time: .time, name: .name, message: .fields.message, file: .file, line: .line, fields: .fields, spans: .spans}' | json2csv -o log.csv
tad log.csv
```
