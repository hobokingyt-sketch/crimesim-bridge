# Project Armor III: bounded worker execution

SAFE-2 introduces the production bridge-worker crate and one Godot invocation gateway.
Parent-controlled deadlines, whole-job termination on Windows, bounded first/last stdout
and stderr, guarded startup cleanup, and off-thread desktop commands replace blocking
Command::output calls. Normal Play is not timed out. No game features or installer released.

Armor II / SAFE-1 was accepted and merged after all current-code checks passed, including
Windows run 35696987625. See CURRENT.json and the pull request for exact verification status
of this pass; source implementation is not evidence of executed Windows/installer tests.

Read docs/WORKERS.md, the engine module card and relevant source for continuation.
