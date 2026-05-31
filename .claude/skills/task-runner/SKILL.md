---
name: task-runner
description: Execute one task file from tasks/NN_*.md end-to-end via the full enterprise team workflow — branch off main, dispatch task-orchestrator (architect → engineer → QA → security → reviewer → product), run Verify section, commit conventional, open PR. Trigger when the user says "run task NN", "implement task NN", "do task NN", or invokes /task-runner NN.
---

# Task Runner

End-to-end driver for one numbered task in `tasks/`. Delegates the full team workflow to the `task-orchestrator` subagent.

## When to invoke

User says one of:
- "run task 05"
- "implement task 11"
- "do task 17"
- `/task-runner 09`

## Workflow

### 1. Locate the task file
Glob `tasks/{NN}_*.md`. Read it. Extract:
- **Goal**
- **Dependencies**
- **Acceptance Criteria**
- **Implementation Steps**
- **Files**
- **Tests**
- **Verify**
- **Done When**

### 2. Verify dependencies
For each dependency listed (e.g. "Depends on Task 04"), grep `git log --oneline` for that task slug. If missing, abort with a message naming the missing predecessor.

### 3. Branch off main
```bash
git checkout main
git pull --ff-only
git checkout -b task/{NN}-{slug}
```

### 4. Dispatch task-orchestrator
Use the Agent tool with `subagent_type: task-orchestrator`. Pass:
- Task number
- Path to task file
- Layers touched (frontend / backend / proxy / db / infra)
- Any context the user added in their message

The orchestrator runs the full team workflow:
1. Architects (parallel) — design notes
2. Engineers (parallel per layer) — implementation
3. Quality (parallel) — QA + security + code review + specialist reviewers
4. Product gate — features.md verification
5. Verify section run — capture evidence under `.evidence/{NN}/`

### 5. Open PR
After orchestrator returns clean, invoke `gh pr create` with the template from CLAUDE.md:

```
## Goal
{from task file}

## Changes
- {one bullet per Implementation Step}

## How to verify
{from task Verify section}

## Evidence
{paste screenshot links / curl output from .evidence/}
```

### 6. Report
Return PR URL plus a checklist confirming each *Done When* item.

## Hard rules

- Never skip the Verify section
- Never merge without architect + reviewer sign-off
- Never proceed if a dependency task is not merged
- Never run a destructive git op without confirming with the user
- Never commit `.env` or any file blocked by the PreToolUse hook
