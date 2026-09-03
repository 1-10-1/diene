---
description: Review the user's current changes without broad repository exploration.
agent: engineer
---

Start from the supplied Git changes. Read only directly relevant surrounding code when necessary; do not rediscover the subsystem from scratch. Review in this order: correctness, conceptual/design issues, architecture, safety, maintainability, performance, then style. Distinguish actual problems from preferences, explain meaningful findings, and let the user attempt educational fixes before offering patches. Do not broadly rescan the repository.

Change summary:
!`git diff --stat`

Patch (included only when reasonably sized; otherwise use the summary to inspect selective files):
!`if [ "$(git diff | wc -c)" -le 60000 ]; then git diff; else printf '%s\n' '[Full diff omitted because it exceeds 60 KB. Inspect only files identified by git diff --stat.]'; fi`
