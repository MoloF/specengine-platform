---
id: CRIT-01
class: canon
links:
  verifies: [REQ-002]
  uses_term: [GLS-worktree]
owner: owner
reviewed: 2026-09-20
---

# Сухой прогон не меняет файлов

После `sync --dry-run` дерево рабочей копии побайтно совпадает с исходным.
