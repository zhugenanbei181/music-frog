# Environment-blocked board procedures

> 层级：L3

Every entry of `todos/CODE_TODO_BLOCKED.md` (the environment-blocked board,
29 entries) is an *executable procedure*, not a prose note. This directory
holds one script per blocker class; every script prepares and runs everything
that can run on this machine, separates the environment-dependent step and
prints exactly what remains.

The board stays the single source of truth for the entries themselves. This
document only maps an entry to the command that closes it.

## Result vocabulary and exit codes

| Marker | Meaning |
| --- | --- |
| `OK` | the step ran on this machine and passed |
| `SKIP` | the step is not applicable in the current mode |
| `PENDING` | the step needs the missing environment; it is listed under "remains" |
| `GAP` | a real repository gap another owner must land first; neither a script defect nor environment evidence |
| `FAIL` | a real defect; the script exits non-zero |

| Exit code | Meaning |
| --- | --- |
| `0` | every local step passed; environment legs may still be pending |
| `1` | a real defect was found |
| `2` | the requested mode could not start because its environment is absent |

`--dry-run` never exits `2`: it runs every local step and reports the pending
legs. `--strict` promotes a `GAP` to a failure.

## Procedures

| Script | Purpose |
| --- | --- |
| `scripts/blocked/arm64-device-evidence.sh` | prepares the ARM64 candidate build, proves the ELF boundary of the packaged driver and drives the physical-device evidence run |
| `scripts/blocked/tag-ci-evidence.sh` | validates the controlled `v*` tag release locally and prints the exact tag and run to request |
| `scripts/blocked/release-keystore-evidence.sh` | validates the release signing wiring without a secret and performs the real signature checks once the secret exists |
| `scripts/blocked/crates-publish-order.sh` | runs the ordered crates.io rehearsal, separates the publish-order gate from a real packaging defect and stops at the first defect |
| `scripts/blocked/foreign-target-evidence.sh` | runs the foreign-target cross-check legs this machine can run and records the rest as explicit environment legs |
| `scripts/blocked/check-blocked-coverage.py` | proves that every board entry has a procedure and that no procedure script is unmapped |
| `scripts/blocked/run-all.sh` | runs the coverage checker and every procedure in dry-run mode |
| `scripts/blocked/publish_chain.py` | the ordered dry-run driver used by `crates-publish-order.sh`; it reuses the repository release guard as the only classifier |
| `scripts/blocked/lib.sh` | the shared result vocabulary, evidence directory handling and exit rules |

All evidence lands in `target/blocked-evidence/<procedure>/` (git-ignored).
The scripts only read the repository; they never mutate tracked files and they
are idempotent.

### Running them

```bash
# every procedure in dry-run mode plus the coverage checker
bash scripts/blocked/run-all.sh

# the coverage checker alone
python3 scripts/blocked/check-blocked-coverage.py

# one procedure
bash scripts/blocked/arm64-device-evidence.sh --dry-run
bash scripts/blocked/tag-ci-evidence.sh --dry-run
bash scripts/blocked/release-keystore-evidence.sh --dry-run
bash scripts/blocked/crates-publish-order.sh --dry-run
bash scripts/blocked/foreign-target-evidence.sh --dry-run
```

| Knob | Meaning |
| --- | --- |
| `--dry-run` | run everything that does not need the missing environment |
| `--strict` | promote a repository gap to a failure |
| `--apk` / `--apk-a` / `--apk-b` | the device candidate and the two release candidates |
| `--with-checksums` | also build the `.crate` artifacts of the neutral chain and write the checksum manifest |
| `--skip-cross-check` / `--skip-android-check` | skip the heavy aarch64 cross-check legs |
| `BLOCKED_CARGO_TIMEOUT` | the per-crate timeout of the crates rehearsal (default 1800 s) |
| `BLOCKED_EVIDENCE_ROOT` | where the evidence directory is created (default `target/blocked-evidence`) |

The crates rehearsal and the aarch64 cross-checks call `cargo`, which on a
shared workstation is serialized through the machine's cargo lane. Under
contention from other sessions a single crate can wait for the lane for
minutes; that is a shared-resource condition, not a defect of the procedure.
Raise `BLOCKED_CARGO_TIMEOUT` when the box is busy.

The repository's cargo parallel budget allows one stream to compile at a time,
so the cargo-heavy legs (the crates rehearsal and the two aarch64
cross-checks) are meant to be scheduled when no other stream is building. The
three procedures that do not compile at all — the device, tag and keystore
legs — can run any time.

## Mapping of the 29 board entries

| Board entry | Script | Command | Evidence produced | Environment still needed |
| --- | --- | --- | --- | --- |
| `BANDROID-041.3.2R.15` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` then `bash scripts/blocked/arm64-device-evidence.sh` on the device | `candidate-apk.txt`, `elf-boundary.txt`, `device-plan.md`, `logcat.txt`, `report.md` | ARM64 physical device (adb, `ro.kernel.qemu` must not be `1`) |
| `CORE-007.2` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` then `bash scripts/blocked/arm64-device-evidence.sh` on the device | `session-readback.txt` (process id, service state, version readback after a Binder death/rebind) | ARM64 physical device |
| `BANDROID-041.3.3` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` then `bash scripts/blocked/arm64-device-evidence.sh` on the device | `elf-boundary.txt` (every packaged `lib/arm64-v8a/*.so` is an AArch64 object) plus the on-device load record in `logcat.txt` | ARM64 physical device for the load record; the ELF half is already closed locally |
| `BANDROID-002.4` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` then `bash scripts/blocked/arm64-device-evidence.sh` on the device | `power.txt` (WakeLock held only inside the Start/Cleanup transaction) | ARM64 physical device |
| `BANDROID-005.1` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` then `bash scripts/blocked/arm64-device-evidence.sh --lifecycle-cycles 100` on the device | `lifecycle-cycles.txt` (the bounded lock/background/rotate cycle record) and `meminfo-<package>.txt` | ARM64 physical device |
| `BANDROID-040.5` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` then `bash scripts/blocked/arm64-device-evidence.sh` on the device | `session-readback.txt` (Core and VPN state/generation read back before a command is admitted) | ARM64 physical device |
| `BANDROID-040.7` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` then `bash scripts/blocked/arm64-device-evidence.sh` on the device | `session-readback.txt` (the same owner snapshot revision on Compose, Bevy and Slint) | ARM64 physical device |
| `BANDROID-040.8` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` then `bash scripts/blocked/arm64-device-evidence.sh` on the device | `logcat.txt` (the Age credential read/set/clear round trip with no secret in log, snapshot or profile state) | ARM64 physical device |
| `BANDROID-042.5c` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` then `bash scripts/blocked/arm64-device-evidence.sh` on the device | `logcat.txt` (the same-ABI Mihomo `-t` validation under a Stopped Core, zero write) | ARM64 physical device |
| `BANDROID-040.4` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` then `bash scripts/blocked/arm64-device-evidence.sh` on the device | `session-readback.txt` (the JNI to Activity to UniFFI to host to Messenger to owner path answers a request) | ARM64 physical device |
| `BANDROID-043.4` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` then `bash scripts/blocked/arm64-device-evidence.sh` on the device | `logcat.txt` (asynchronous request-id wait; a late reply is dropped instead of forging success) | ARM64 physical device |
| `BANDROID-018.2` | `scripts/blocked/tag-ci-evidence.sh` | `bash scripts/blocked/tag-ci-evidence.sh --dry-run` then the printed tag push and `gh run watch --workflow release.yml` | `workflow-inventory.txt`, `existing-runs.txt`, `request.md` and the run URL of the tag run | the controlled `v*` tag push and its runner |
| `BANDROID-019.2` | `scripts/blocked/tag-ci-evidence.sh` | `bash scripts/blocked/tag-ci-evidence.sh --dry-run` then the tag run | the manifest contract guard output (local leg) and the aapt identity output of the tag run, bound to the run id | the tag run |
| `BANDROID-038.2` | `scripts/blocked/tag-ci-evidence.sh` | `bash scripts/blocked/tag-ci-evidence.sh --dry-run` then the tag run | the per-flavor APK SHA-256 of the tag run | the tag run |
| `BANDROID-020.1` | `scripts/blocked/tag-ci-evidence.sh` | `bash scripts/blocked/tag-ci-evidence.sh --dry-run` then the tag run | the toolchain check output of the local leg and of the tag run | the tag run (the CI side of the same check) |
| `BANDROID-024.1` | `scripts/blocked/tag-ci-evidence.sh` | `bash scripts/blocked/tag-ci-evidence.sh --dry-run` then the tag run | the AGP/Kotlin/Gradle/JDK tuple check of the local leg and the Gradle build output of the tag run | the tag run |
| `BANDROID-039.1` | `scripts/blocked/release-keystore-evidence.sh` | `bash scripts/blocked/release-keystore-evidence.sh --dry-run` | `signing-contract.txt` (the four secret names and the fail-closed requirement) | the user keystore and its four CI secrets, plus the release signing config described below |
| `BANDROID-015.2` | `scripts/blocked/release-keystore-evidence.sh` | `RELEASE_KEYSTORE_BASE64=... RELEASE_KEYSTORE_PASSWORD=... RELEASE_KEY_ALIAS=... RELEASE_KEY_PASSWORD=... bash scripts/blocked/release-keystore-evidence.sh --apk-a dist/app-compose-universal-release.apk` | `release-fingerprint.txt`, `signer-digest-A.txt` and the identity/aapt guard output | the keystore and a signed release universal APK |
| `BANDROID-039.2` | `scripts/blocked/release-keystore-evidence.sh` | `RELEASE_KEYSTORE_BASE64=... RELEASE_KEYSTORE_PASSWORD=... RELEASE_KEY_ALIAS=... RELEASE_KEY_PASSWORD=... bash scripts/blocked/release-keystore-evidence.sh --apk-a dist/previous.apk --apk-b dist/current.apk` | `signer-digest-A.txt`, `signer-digest-B.txt` and the fingerprint equality verdict | two signed candidates; the `adb install -r` upgrade leg additionally needs an ARM64 device |
| `PUB-001.3` | `scripts/blocked/crates-publish-order.sh` | `bash scripts/blocked/crates-publish-order.sh --dry-run` | `dry-run.json` with the per-crate status of the ordered rehearsal | the contract crate on crates.io (the rehearsal reports the order gate until then) |
| `PUB-003.2` | `scripts/blocked/crates-publish-order.sh` | `bash scripts/blocked/crates-publish-order.sh --dry-run` | `dry-run.json` with the `ok` status of `lurker-engine-runtime` in the enforced order | the chain published on crates.io |
| `PUB-003.3` | `scripts/blocked/crates-publish-order.sh` | `bash scripts/blocked/crates-publish-order.sh --dry-run` | `dry-run.json` with the `ok` status of `lurker-engine-runtime` | the chain published on crates.io |
| `PUB-006.12` | `scripts/blocked/crates-publish-order.sh` | `bash scripts/blocked/crates-publish-order.sh --dry-run --with-checksums` | `checksums.json` (the SHA-256 manifest of the packaged `.crate` artifacts) and the frozen-baseline count | crates.io credentials, the GPG signing material and the one-shot `1.0.0-rc.1` version freeze |
| `PUB-006.5` | `scripts/blocked/crates-publish-order.sh` | `bash scripts/blocked/crates-publish-order.sh --dry-run --with-checksums` then `bash scripts/neutral-crate-release.sh publish` with the token set | `dry-run.json`, `checksums.json` and the seven crates.io version receipts | the crates.io API token, the `v1.0.0` tag and the docs.rs build links |
| `APPID-001.4` | `scripts/blocked/foreign-target-evidence.sh` | `bash scripts/blocked/foreign-target-evidence.sh --dry-run` then the Windows commands of `foreign-legs.md` | `foreign-legs.md` (the validated NSIS template plus the exact Windows build commands) | a Windows target machine for the installer build and the install/uninstall smoke |
| `GAP-2.1` | `scripts/blocked/foreign-target-evidence.sh` | `bash scripts/blocked/foreign-target-evidence.sh --dry-run` then the Windows commands of `foreign-legs.md` | `foreign-legs.md` (the install, tray, TUN and uninstall smoke commands) | a Windows target machine |
| `GAP-2.2` | `scripts/blocked/foreign-target-evidence.sh` | `bash scripts/blocked/foreign-target-evidence.sh --dry-run` then the macOS commands of `foreign-legs.md` | `foreign-legs.md` (the bundle, DMG, codesign and notarisation commands) plus the deferral guard result | a macOS target machine and the Apple CI deferral decision |
| `GAP-3.1` | `scripts/blocked/foreign-target-evidence.sh` | `bash scripts/blocked/foreign-target-evidence.sh --dry-run` then the tag run of the Android leg | `foreign-legs.md` (the Gradle APK build command) and the identity receipt of the tag run | the tag run, or a local full SDK/Gradle environment |
| `GAP-3.2` | `scripts/blocked/foreign-target-evidence.sh` | `bash scripts/blocked/foreign-target-evidence.sh --dry-run` then the iOS commands of `foreign-legs.md` | `foreign-legs.md` (the `aarch64-apple-ios` build and the package structure check) | the Apple target unblocking decision and the macOS toolchain |

## The five blocker classes in detail

### ARM64 physical device (`arm64-device-evidence.sh`)

Local legs, no device required:

1. the blocked-board coverage preflight;
2. `scripts/android-toolchain.py check` — the SDK/NDK/Gradle/JDK baseline;
3. the product identity guard and the Android manifest contract guard;
4. the candidate APK discovery, its SHA-256 and its build age against HEAD
   (a candidate that predates HEAD is reported as a gap, not used silently);
5. the ELF boundary: every `lib/arm64-v8a/*.so` inside the candidate must be an
   AArch64 object;
6. the device evidence plan: for each of the 11 board entries the exact
   on-device observation, the adb command and the evidence file.

Device legs, only with a physical ARM64 device attached over adb. The script
refuses an emulator (`ro.kernel.qemu` must not be `1`) and refuses a non-arm64
ABI. It installs the candidate, records the device model, API level, release
and build fingerprint, reads back the process id, the service state and the
package version, and collects a logcat excerpt plus the power and memory
dumps.

### Controlled tag CI (`tag-ci-evidence.sh`)

Local legs: the CI trigger guard that pins the `v*` tag rule, the release
build target guard, the product identity guard against the exact tag, the
manifest contract guard, the Android toolchain check, the workflow inventory
and the local release rehearsal of the neutral chain.

The script then prints the exact request (in `request.md`): the tag to push,
the `gh workflow run` invocations for the Android, Bevy, parity, neutral
crates and device workflows, and the evidence to archive per board entry from
each job. It reads the existing runs so a second request is never made for a
tag that already ran. It never pushes a tag and never starts a workflow.

### User-held release keystore (`release-keystore-evidence.sh`)

Without a secret the script validates the wiring contract and records it in
`signing-contract.txt`: the four secret names, the Gradle release signing
config and the fail-closed publish job. The private key never enters the
repository, the logs or the evidence directory — a keystore path inside the
repository is rejected and the materialised keystore lives in a private
temporary directory that is removed on exit.

With the secret the script reads the release certificate fingerprint with
`keytool`, reads the signer certificate digest of one or two candidate APKs
with `apksigner`, compares both with the keystore fingerprint and runs the
aapt identity guard on each candidate.

### crates.io publish order (`crates-publish-order.sh`)

The rehearsal runs `cargo publish --dry-run` for the seven neutral crates in
the enforced order. The classifier is the repository's own release guard, so
the two outcomes are never confused:

| Status | Meaning | Effect |
| --- | --- | --- |
| `ok` | the crate packages and verifies | — |
| `order-gate` | a neutral dependency is not on crates.io yet | expected before the chain is published; not a failure |
| `already-published` | crates.io already holds this exact version | expected when the tag is re-run |
| `packaging-failure` | a genuine packaging or verification defect | the run stops at the first one and fails |

The checksum manifest (`checksums.json`) is produced from the `cargo package`
artifacts and reports how many of the seven crates are frozen on the release
candidate baseline. Today only the first crate of the chain can be packaged;
the others are behind the order gate, which is the board's state, not a
defect.

### Foreign target machines (`foreign-target-evidence.sh`)

Local legs: the Apple CI deferral guard, the packaging template validation
(NSIS, DMG, AppImage, deb), the release build target guard, the product
identity guard, the aarch64 desktop cross-check and the aarch64 Android
cross-check.

The foreign legs are written to `foreign-legs.md` with the exact command per
target: the NSIS installer builds and the install/uninstall smoke on Windows,
the bundle, DMG, codesign and notarisation on macOS, the `aarch64-apple-ios`
build and the package structure check for iOS. They are never claimed by this
machine.

## What the local legs already close

No entry of the board can be closed with local work alone: every one of the 29
entries still has an environment leg that only a device, a tag runner, the
user secrets, the crates.io registry or a foreign machine can provide. What
the local work closes are the *sub-legs* of those entries, so that only the
environment leg is left when the environment arrives. The scripts record them,
and the table names them:

| Board entry | Local leg that is already closed | Remaining leg |
| --- | --- | --- |
| `BANDROID-019.2` | the Android manifest contract guard passes for every registered production flavor | the same guard inside the tag run |
| `BANDROID-020.1` | `scripts/android-toolchain.py check` passes with the registered pins (JDK 17, Gradle 9.7.0, AGP 9.3.1, Kotlin Gradle Plugin 2.4.21, platform 37.0, Build Tools 36.0.0, command-line tools 12.0, NDK 29.0.14206865, Rust 1.99.0) | the same check inside the tag run |
| `BANDROID-024.1` | the toolchain checker rejects any drift of the AGP/Kotlin/Gradle/JDK tuple | the Gradle build inside the tag run |
| `BANDROID-018.2` | the CI trigger guard proves the expensive workflows only start for a `v*` tag and that the tag job conditions are in place | the tag run itself |
| `BANDROID-038.2` | the identity registry and the product identity guard are green for every registered flavor | the per-APK SHA receipt of the tag run |
| `PUB-001.3` | the publish order is derived from the dependency graph, the chain is acyclic and every crate is publishable and licensed | the registry state: the contract crate is not on crates.io yet |
| `PUB-003.2` | the topology rehearsal is green; only the registry state gates the chain | the registry state |
| `PUB-003.3` | the topology rehearsal is green | the registry state |
| `PUB-006.12` | the checksum manifest of the packageable crate is produced and the frozen-baseline count is reported | the `1.0.0-rc.1` freeze, the GPG material and the credentials |
| `BANDROID-041.3.3` | the ELF boundary of the packaged driver is proven locally | the on-device load record |
| `APPID-001.4` | the NSIS template is validated and the exact build commands are recorded | the Windows build and the install smoke |
| `GAP-2.1` | the packaging templates of every desktop product are validated | the Windows install, tray and TUN smoke |
| `GAP-2.2` | the DMG script, the Info.plist and the entitlements are validated | the macOS bundle, signing and notarisation |
| `GAP-3.2` | the Apple CI deferral is proven, so the deferral decision is a recorded fact and not an accident | the unblocking decision and the macOS toolchain |

## Findings the procedures surfaced

These are facts the procedures discovered while validating the board, not
opinions. Each one is reproducible with the command that is given.

| Finding | How to see it | Consequence |
| --- | --- | --- |
| the candidate release tag `v0.50.0` already exists locally and on the remote, and it points at commit `58158da6`, not at HEAD | `bash scripts/blocked/tag-ci-evidence.sh --dry-run` | a receipt bound to the current code needs a tag whose commit is HEAD; the product identity registry pins the release version to `0.50.0`, so that means a version bump in the same commit as the tag, or a deliberate decision to move the existing tag |
| the `release` workflow already ran successfully for that tag | `gh run list --workflow release.yml --branch v0.50.0` | a `C`-level receipt of the tag pipeline exists, but it is bound to the older commit `58158da6`; it does not cover the current code |
| `release-neutral-crates.yml` is not published on the default branch | `gh api repos/:owner/:repo/contents/.github/workflows` | a tag push today would not start the neutral crates workflow at all; it has to be published before the crates.io legs can produce a `C` receipt |
| the release build type is signed with the debug key and the release workflow carries no keystore secret | `bash scripts/blocked/release-keystore-evidence.sh --dry-run` | the board entry of §三 claims that the fail-closed path is registered; in the current tree it is not. The three gaps below have to be landed before the environment can help |
| the local candidate APK predates HEAD | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` | the APK in the build output is not a valid device candidate for the current code; it must be rebuilt before the device run |
| the local release APK is signed with the debug key and carries version `0.41.0` while the product identity registry says `0.50.0` | `bash scripts/blocked/release-keystore-evidence.sh --dry-run --apk-a android/app/build/outputs/apk/compose/release/app-compose-universal-release.apk` | the aapt identity check rejects it; a release candidate has to be rebuilt before it can be signed and verified |
| the aarch64 Android clippy leg fails on `clippy::missing-const-for-thread-local` in `crates/infiltrator-slint/src/surface/harness.rs` | `bash scripts/blocked/foreign-target-evidence.sh --dry-run` | the Android gate that the ARM64 board entries depend on does not compile clean at the moment; the defect is in a file another workstream is editing, so the procedure reports it as a `FAIL` and exits `1` instead of hiding it |

## Repository gaps another owner must land

These are **not** environment evidence and **not** script defects: they are
local repository changes that must exist before the environment arrives. They
are reported as `GAP` by the scripts and are not edited here, because the
release workflow and the Gradle build are owned by another stream.

| Gap | Where | What has to change |
| --- | --- | --- |
| the release buildType is signed with the debug key | `android/app/build.gradle.kts` | a dedicated release signing config must read the keystore from the environment and must fail closed when the secret is absent, instead of `signingConfig = signingConfigs.getByName("debug")` |
| the release workflow carries no keystore secret | `.github/workflows/release.yml` | the publish job must read the four secrets, must fail loudly when they are absent and must never fall back to a debug key |
| the release signing config does not exist | `android/app/build.gradle.kts` | a `signingConfigs.create("release")` block that binds `RELEASE_KEYSTORE_BASE64`, `RELEASE_KEYSTORE_PASSWORD`, `RELEASE_KEY_ALIAS` and `RELEASE_KEY_PASSWORD` |

The keystore procedure reports the third gap from two angles (the file does
not read the secret, and it has no dedicated release signing config); they are
one change, not two.

Until those three gaps are closed, the board entries of §三 cannot produce a
signed release artifact even if the secret arrives. The scripts print them as
`GAP` on every run, so the gap cannot be forgotten.

## The coverage checker

`scripts/blocked/check-blocked-coverage.py` is the tripwire:

* every open entry of the blocked board must appear in the mapping table
  above;
* every mapped entry must name an existing script and a non-empty command;
* every procedure script in this directory must be mapped by at least one
  entry, so a new script cannot be added without a board entry.

Every procedure script runs the checker as its first step, and
`scripts/blocked/run-all.sh` runs the checker plus every procedure in dry-run
mode. A new blocked entry without a procedure therefore fails the next run of
any procedure.

## Evidence honesty

* A dry run is **not** a device run, **not** a CI run, **not** a signature
  check and **not** an upload. The dry runs in this directory close only the
  local legs listed in the table above.
* The device procedure refuses an emulator and refuses a non-arm64 device; it
  exits `2` instead of producing an empty report.
* The tag procedure never pushes a tag and never starts a workflow; it prints
  the request and reads the existing runs.
* The keystore procedure never copies a private key into the repository, into
  the logs or into the evidence directory.
* The crates procedure never uploads; the upload stays behind the crates.io
  API token in the publish mode of the repository release guard.
