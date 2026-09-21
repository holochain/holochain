# holochain_cli_client

Provides the `hc-client` binary, a command line client for interacting with Holochain conductors.

Note that this crate also provides a library, which is built into the `hc` binary, which is the preferred way to use
this functionality from the command line.

## Installing apps with membrane proofs

Install-time membrane proofs can be supplied as raw files with repeatable
`--membrane-proof ROLE=PATH` flags. The path is resolved from the command's
current working directory, and the file bytes are passed unchanged to the
role's genesis workflow:

```shell
hc client call --port 12345 install-app ./app.happ \
  --membrane-proof role-1=./proof-role-1.bin \
  --membrane-proof role-2=./proof-role-2.bin
```

Each role may be supplied only once. A flag conflicts with a non-null proof for
the same role in the positional role-settings YAML, and flags cannot target an
existing-cell role. Use the positional arguments to provide a role-settings
file when modifiers or inline proof sources are needed:

```shell
hc client call --port 12345 install-app ./app.happ proof-seed ./roles.yaml
```

The YAML file may use `base64` or a file `path` for proof bytes. YAML file paths
are resolved relative to the YAML file, independently of the command's current
directory. The same source forms are available for `init_properties`.

## Agent-bound proofs

Some applications issue proofs for a particular agent. The workflow is:

1. Run `hc client call --port 12345 new-agent` and decode the JSON string it
   prints to obtain the agent key.
2. Request a proof for that key from the application's external proof issuer.
3. Save the issued proof bytes to a file such as `proof.bin`.
4. Install with the same conductor, agent key, and proof file:

   ```shell
   hc client call --port 12345 install-app ./app.happ \
     --agent-key "$AGENT_KEY" \
     --membrane-proof role-1=./proof.bin
   ```

Proof issuance is application-specific and is not implemented by this CLI.
Sandbox-generated conductors create their own agent keys, so a proof can be
reused only when it is valid for the generated agent. For applications that
allow installation before their agent proof exists, see
[Deferred membrane proofs](#deferred-membrane-proofs).

## Deferred membrane proofs

Applications whose manifest allows deferred membrane proofs can be installed
without proofs and completed later with `hc client provide-memproofs`:

```shell
hc client provide-memproofs --port 12345 my-app \
  --membrane-proof role-1=./proof.bin
hc client provide-memproofs --port 12345 another-app \
  --membrane-proofs ./proofs.yaml
hc client call --port 12345 enable-app my-app
```

The `--membrane-proofs` file is a direct role-to-source map, rather than a
role-settings document:

```yaml
role-1:
  base64: AQID
role-2:
  path: ./proof.bin
```

`--port` selects the conductor's admin websocket. The command uses it to find
the app, checks that the app is awaiting proofs, and then connects to an app
interface using the `sandbox` origin. It reuses a compatible existing
interface or creates one when necessary. No signing credentials or `.hc_auth`
file are needed because providing proofs is an app request, not a signed zome
call. After a successful submission, the app is disabled with a
`NotStartedAfterProvidingMemproofs` status; explicitly enable it with the
admin command above. The command prints updated app information as JSON.

Proofs must be valid for the agent key created during installation. Obtain that
key from the install output or `list-apps`, request the proof from the
application's external issuer, and submit the issued bytes. If a connection
failure leaves the result ambiguous, use `hc client call --port 12345 list-apps`
to check the app status before retrying. An explicit `{}` YAML map is valid for
an application that can complete genesis without proofs.
