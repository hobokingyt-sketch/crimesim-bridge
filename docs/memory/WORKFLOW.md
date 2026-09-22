# Working across independent chats

## What is authoritative
The current user directs product changes. Git contains durable implementation, decisions and
checkpoints. Source describes what the program currently does; tests/runs establish only what they
actually exercised. Comments, summaries, filenames and a prior assistant's confidence are not proof.
A new session fetches one commit consistently. Before publishing it rechecks HEAD and reconciles
new commits rather than putting an older tree on top of newer work.

## Load progressively
Start with AGENTS.md and CURRENT.json, choose a module, read its card and then its source.
Read neighboring contracts only when relevant. The full backlog and every old ADR are not startup
reading. `brief` selects the affected backlog/decisions. `impact` reports direct and transitive
consumers as review candidates, not permission requirements. There is no need for a vector database,
new cloud subscription, or a second autonomous coding agent for this repository size.

## Commands for the developer, never the user
`python tools/project_memory.py check` checks registration, paths, references, status schemas,
small startup guidance and source review freshness. Structural failures return nonzero; size and
changed-source review warnings are advisory. `brief --module engine` prints the entry route.
`impact --base <commit>` compares tracked changes to a known base. `pack --module engine --output
<outside-repo.zip>` includes selected source and direct dependency cards, with an explicit inventory
of included/omitted files. `review --module engine --note "Reviewed worker interface changes"`
records a developer review declaration, not a test pass or proof of semantic correctness.

Source packs require a clean checkout unless `--working-copy` explicitly labels a local draft.
A pack's Git HEAD, dirty state and source SHA-256 inventory are separate. Dirty packs must not be
represented as committed builds. Pack generation never runs commands from supplied metadata.
It excludes sensitive filename patterns and generated output; that is a filter, not a secret scanner.
Budget overflow is recorded as omitted, never silently truncated code. Obtain omitted contents before
editing or making claims about them. Unknown module/path input fails clearly instead of widening scope.

## Finish a session
Update CURRENT.json only with the actual task status, tested facts, limits and next task IDs.
Update affected cards when responsibilities or contracts change. Record significant decisions with
reason and tradeoff; mark old decisions superseded, preserving their rationale. Do not append the
whole conversation. Git history holds older checkpoints. A fresh chat should need the current checkpoint,
not a growing archive of handoff reports. Never mark a backlog item done without its acceptance evidence.

## Limits and independence
Repository CI can check consistency but cannot prove that an agent understood the design.
An AGENTS.md file is not an access-control mechanism. Branch protection is not enabled by these files;
required-check rules would be separate GitHub settings. In this pass the memory workflow is an automated
check, not a claim that merges are technically impossible when it fails.

The Python tools are developer-side Bridge-repository tools. The installed app still uses its existing
managed-game Chat Pack implementation. PACK-1 connects scoped game handoffs to the one-button UI.
Do not claim that button integration exists until its end-to-end test and desktop exercise pass.
