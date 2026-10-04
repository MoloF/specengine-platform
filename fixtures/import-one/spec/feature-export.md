---
kind: feature
status: shipped
---
# Export

- **AC-001:** Export writes one file per run.
- **AC-002:** Export never overwrites a file.
- **AC-004: Retry**: Export retries once
  after a failed write.
  - **AC-005: Backoff**: The retry waits one second.

Retries follow AC-004 and AC-005.
