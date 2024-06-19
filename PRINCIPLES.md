# Holochain core team development principles

This is a living document of principles that we, the Holochain core team, have agreed to uphold in the course of our work on Holochain. The purpose of the doc is to identify the shared principles we want to uphold, which we can refer to, add to, and modify as we go. The intention is not to be a prescription for how we make every decision, but rather a set of guiding principles towards which we strive to move ever closer.

# Documentation

Crate-level docs should describe how that crate is organized into modules, similar to how the introduction of a book often lays out the structure of the book and gives a high-level description of the contents of each of its chapters.

Module-level docs are always up to date with the code in that module.

# Testing

## Test organization

Test code should be as close as possible to the code it is testing.

Unit tests should be in a submodule of the code under test.

It may be hard to know where to find a tests of certain functionality since many of our tests are integration tests. The principle we'd like to follow is that it should be possible to discover where to look for a given test by reading the crate-level docs. The docs of one crate or module should describe the structure of what's below, including tests, so that we can hone in on whatever we want to find with the help of the docs.