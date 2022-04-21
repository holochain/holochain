---
name: Bug report
about: Create a report to help us improve
title: "[BUG]"
labels: ''
assignees: ''

---

**Describe the bug**
A clear and concise description of what the bug is, how you encountered it, and
the text of any error you received.

**Expected behavior**
A clear and concise description of what you expected to happen.

**System information:**
 - OS: [e.g. iOS]
 - Browser [e.g. chrome, safari]
 - Holochain and HDK Version (run `hn-introspect` as well as `holochain --build-info`
 from the nix shell and paste output)

**Steps to reproduce**
For isolating the bug, it is helpful to see a failing test or a repo that
reproduces the bug in a fresh hApp. Some suggestions for methods
of writing a reproduction of a bug:

- Write a failing test with [Sweettest](https://docs.rs/holochain/latest/holochain/sweettest/index.html)
- Write a failing test with [Tryorama](https://github.com/holochain/tryorama/)
- Create a minimal reproduction project using `hn-init` and add the code that
produces the bug.

**Screenshots**
If applicable, add screenshots to help explain your problem.

**Additional context**
Add any other context about the problem here.
