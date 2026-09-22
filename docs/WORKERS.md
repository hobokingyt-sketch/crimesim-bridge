# Bounded Godot workers (SAFE-2)

## Scope and code route
`src-tauri/src/core/worker.rs` is the only execution gateway for Godot version, import,
script harness, export, and exported-build smoke checks. It uses `crates/bridge-worker`.
`play.rs` launches a user-requested game; gameplay is not a short-lived build worker.
No engine version, game feature, package schema or simulation architecture changes here.

## Deadline policy
Budgets are developer-owned, not values accepted from an update ZIP:
- Version: 15 seconds.
- Import: 180 seconds.
- Script harness: 60 seconds.
- Export: 300 seconds.
- Exported smoke: 30 seconds.
- Forced shutdown/recovery: another bounded 5 seconds.
These are initial policies, not sacred limits. Adjust a stage deliberately with evidence
when larger assets need it; do not disable the watchdog to make a broken test pass.
Godot `--quit-after` remains useful for normal completion, not as a hang detector.
Timeouts return failed validation. Promotion cannot follow an unconfirmed shutdown.

## Windows lifetime ownership
Windows 10+ creates a named Job Object with KILL_ON_JOB_CLOSE. The process is attached
at creation through STARTUPINFOEX / PROC_THREAD_ATTRIBUTE_JOB_LIST. There is no runnable
spawn-then-attach gap. Only the three standard I/O handles are inherited; the job handle
and the workspace lock are not passed to children. Child processes belong to the same job.
The Bridge polls process status and job accounting, not just the console launcher's status.
A zero-exit launcher with a still-running child is not a successful completed worker.
On deadline/cancellation the entire owned job is terminated and verified empty.
On abrupt Bridge death, closing its last job handle kills associated workers.

`state/worker_guard.json` is written before spawn. Before project recovery, the next
Bridge opens that named job and verifies/forces shutdown without guessing a reused PID.
Missing named job is clean; malformed metadata, access denial or a nonempty job after
the cleanup deadline keeps the guard and blocks mutation. Data is retained for diagnosis.
This is a recoverable interlock, not an extra approval step for normal development.

## Bounded output
A single supervisor thread alternates nonblocking reads of both pipes, bounded per turn.
No unbounded output buffer, queue, reader join or EOF wait is used. Each stream retains
at most 256 KiB of original bytes (first/last portions); counters and an explicit omission
marker distinguish truncation from missing output. UTF-8 decoding uses replacement for
partial/binary text, so the text encoding may use more bytes than the raw capture budget.
Fatal Godot markers are scanned across all received chunks, including omitted middle data.
Fixed per-stage log files are replaced, not appended indefinitely. An interrupted run can
leave a small 'result pending' log; streaming crash-log persistence is not claimed.
Other files that arbitrary project code writes are outside this stdout/stderr cap.

## UI and recovery
Desktop command work runs on Tauri's blocking pool rather than the webview thread.
The existing exclusive workspace lock still serializes protected operations. A worker guard
suppresses READY. Recovery cannot restore/delete candidate files until workers are quiescent.
Library cancellation is tested; there is no new desktop Cancel button in this pass.

## Verification boundaries
The watchdog suite runs the production worker library against a purpose-built misbehaving
executable: silent hang, dual-stream flood, child/grandchild lifetimes, inherited-pipe leaks,
closed-output children, cancellation, missing executables, Unicode/quoting, repeat use and
unchanged committed source/build after timeout. Windows additionally kills the supervisor
process and confirms job-tree cleanup and repeated named-job recovery. The existing real
Godot E2E must separately pass with the gateway integrated before acceptance.
Linux process groups support CI/normal timeouts; they do not claim Windows' kernel-backed
parent-death guarantee. Persisted Windows worker guards intentionally fail closed on Linux.
Not a sandbox, protection from hostile same-user code, or kernel/disk/antivirus hang proof.
Synchronous OS calls and scheduler failure cannot have hard real-time guarantees.

## Primary references
- https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects
- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute
- https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-peeknamedpipe
- https://doc.rust-lang.org/std/process/struct.Child.html
- https://docs.godotengine.org/en/stable/tutorials/editor/command_line_tutorial.html
