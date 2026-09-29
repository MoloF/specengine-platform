---
id: CRIT-01
links:
  verifies: [REQ-002]
  uses_term: [GLS-worktree]
---

# Сухой прогон не меняет файлов

После `sync --dry-run` дерево рабочей копии побайтно совпадает с исходным.
