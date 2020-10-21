# Changelog
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

{{ version-heading }}

### Added
- Added App Validation workflow that runs app validation as authority [#330](https://github.com/holochain/holochain/pull/330)
- Added validation package to entry defs see for usage [#344](https://github.com/holochain/holochain/pull/344)
- BREAKING: get_details and get_links_details return SignedHeaderHashed instead of the header types [390](https://github.com/holochain/holochain/pull/390)
- BREAKING: ZomeInfo now returns the ZomeId [390](https://github.com/holochain/holochain/pull/390)
- Implemented the `emit_signals` host function [#371](https://github.com/holochain/holochain/pull/371), which broadcasts a signal across all app interfaces (fine-grained pub/sub to be done in future work)
- get_details on a HeaderHash now returns the updates if it's an entry header

### Changed

### Deprecated

### Removed

### Fixed

### Security

