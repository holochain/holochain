# Keep the high-level lints at the top of this list,
# and add fine-grained exceptions to the bottom.

''
  -A clippy::nursery \
  -A clippy::cargo \
  -A clippy::pedantic \
  -A clippy::restriction \
  -D clippy::style \
  -D clippy::complexity \
  -D clippy::perf \
  -D clippy::correctness \
  -D clippy::dbg_macro \
  \
  -A clippy::redundant_pattern_matching \
  -A clippy::collapsible_else_if
''
