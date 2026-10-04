---
type: feature
---
# Criteria of the exporter

1. __CRT-0001;__ The exporter writes one file per run.
2. __CRT-0002;__ The exporter never overwrites a file.
3. __CRT-0004; Retry__; The exporter retries once
   after a failed write.
   1. __CRT-0005 - Backoff__ - The retry waits one second.

Retries follow CRT-0004 and CRT-0005.
