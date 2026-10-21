---
class: spec
status: draft
scope: [crates/ferry]
links:
  verifies: [ROUTE-CROSSING]
---

# Ferry check

Checks the crossing rule against the code.

## Acceptance criteria

### The ferry waits for the window {#CHK-01}

Ask for a departure before the window opens: none is given.

### Slack widens the window {#CHK-02}

Ask at the window's end plus the margin: a departure is given.
