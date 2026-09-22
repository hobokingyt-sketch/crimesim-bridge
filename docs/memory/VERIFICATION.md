# Armor I verification

Baseline source tree matched GitHub tree b4c2287a623f257bcda3813139ceedbb8b18918f.

Local checks executed on Linux:
- 31 Python behavior tests passed for repository memory and source handoffs.
- Memory structural check passed across eight registered modules.
- Existing repository, transaction model, recovery model, runtime model and pipeline-contract checks passed.
- Frontend JavaScript syntax check passed.

The behavior suite mutates a disposable repository copy. It tests broken routes, duplicate ownership,
unregistered source, missing evidence and acceptance criteria, obsolete decision references, advisory
size/freshness warnings, scoped archive contents/hashes, omitted full files, secret filename exclusion,
path/symlink refusal, dirty-source provenance, no handoff overwrite, and source mutation during packing.
It does not execute commands from a registry as a side effect of inspection or packaging.

Hosted memory run 35693072715 passed on both ubuntu-latest and windows-latest for
commit a0ab0aed5aec404cdaca9311092d29db5d82f84c. Both jobs passed structure, behavior and
frontend syntax steps. This dated evidence is not a claim about untested future commits.
The Rust application and existing Windows installer workflow were not modified in this pass.
No new installer, runtime crash-safety certification or desktop scoped-pack button is claimed.

Last observed original Windows run: 35690061931 at base 56589ae4858da3809de2ef637fda4b72d17ebdc4.
Compilation and real Godot E2E passed, but the job was cancelled during installer build;
installer verification and publication were skipped.
