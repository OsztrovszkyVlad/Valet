# Privacy

Versiunea în română: [PRIVACY.ro.md](PRIVACY.ro.md)

This page says what Valet sends over the network, what it stores on your computer, what it does not have, and what happens to the data when you uninstall. It describes the code at commit `1d910c9`. Where something was read from the code or from a dependency and not observed on a running Windows install, it says so. The license check (section 1, "The license check") is the exception to "describes the code": it is written from the contract in `docs/LICENSING.md`, which the Phase 4 code is being built to, and it was not checked against code or a running build.

Short version: Valet makes two kinds of network request. One is a check for a new version of itself. The other is a license check, and it happens only if you have imported a Pro license in a build that has a license server configured. No file name, path, rule or file content is part of either. Everything else stays on your computer.

## 1. What goes over the network

At commit `1d910c9`, Valet's own code opens a connection in one place: the updater (`apps/desktop/src-tauri/src/updater.rs`). The only network library in the dependency tree, `reqwest`, came in through `tauri-plugin-updater` and nothing else (`cargo tree -p app -i reqwest` printed that one path, checked for the Linux host and for `--target x86_64-pc-windows-msvc`). The license check described below adds a second user of `reqwest`, in the app crate; once that code is in, the same command prints more than one path.

### The addresses

| Channel | Address |
|---|---|
| Stable | `https://github.com/OsztrovszkyVlad/Valet/releases/latest/download/latest.json` |
| Beta | `https://github.com/OsztrovszkyVlad/Valet/releases/download/updater-channel/latest-beta.json` |

Valet asks only the address of your channel. The channel is set in Settings, Updates. Until you choose one, it is Beta for a pre-release version (a version number with a `-`, such as `1.0.0-beta.1`) and Stable otherwise.

If a newer version exists, the file `latest.json` names an installer on GitHub Releases. Valet downloads that installer only after you press "Install and restart" and confirm. GitHub may answer with a redirect to its own download servers; this was not captured on a running install.

### What a request contains

The check is an HTTPS GET of a static JSON file. From the code and from the source of `tauri-plugin-updater` 2.14.1:

- The address has no placeholders, so the installed version, the Windows version and the processor type are not in the address.
- The headers the plugin sets are `Accept: application/json` and `User-Agent: tauri-plugin-updater/2.14.1`. The user agent names the plugin version, not the Valet version. The download of the installer uses `Accept: application/octet-stream` and the same user agent.
- There is no request body and no cookie.
- GitHub sees what any web server sees: your public IP address, the time, the user agent and the TLS connection data. GitHub's own logging and retention are GitHub's, not Valet's.
- The program compares versions on your computer. The installed version is not sent.
- The request goes through the Windows proxy settings (the plugin is built with its `system-proxy` feature).

No file name, folder path, rule or file content is part of any request. The web view that shows the interface has a content security policy whose `connect-src` is `'self' ipc: http://ipc.localhost` (`tauri.conf.json`), so the interface itself cannot call other addresses. There is no test for that policy.

### When

- 30 seconds after Valet starts, then once every 24 hours while it keeps running. The code looks at the clock every hour and checks when 24 hours have passed, so time spent asleep counts. So each start of Valet, in a build that has updates, makes one automatic check.
- When you press "Check now" in Settings, Updates.
- When you press "Install and restart", Valet downloads the installer.

An automatic check never installs anything. It only shows a notification (once per version for each run of Valet) and a mark on the Settings button. Installing needs your click and a confirmation. The installation waits while a batch of file operations is running.

### Can it be turned off

Not in Settings: there is no option to switch the automatic check off. What you can change is the channel. Blocking `Valet.exe` in a firewall stops the checks (the failure is written to the log as a warning, and Valet goes on); this was not tested.

A build made without the updater keys has no update code running at all: no automatic check, and "Check now" answers that updates are not configured. Builds made with `pnpm tauri dev` are of this kind. The release workflow builds with the keys only when the repository owner has set them (`docs/RELEASE.md`, section 0.2); the dry run at commit `1d910c9` built without them (its log says "updater artifacts off").

### The license check

This subsection follows `docs/LICENSING.md`, "Online check". It was not checked against code or against a running build.

**When it runs.** Only when a Pro license file has been imported. The trial (14 days) and the Free plan make no license request: the trial start is a date stored on your computer, and nothing about it goes over the network. With a Pro license imported, the check runs:

- at activation, when you import the license file (`POST /v1/activate`);
- then at most once every 30 days: the app looks at startup and every 24 hours, and sends a request (`POST /v1/validate`) only when the last successful check is 30 days old or older;
- when you remove the license in Settings (`POST /v1/deactivate`, see below).

**What is sent.** The body of the activate and validate requests has three fields:

- the license id, a random identifier (a UUID version 4) written in your license file;
- a machine identifier: the first 32 hexadecimal characters of the blake3 hash of the text `valet-machine-v1:` followed by the Windows `MachineGuid` (read from `HKLM\SOFTWARE\Microsoft\Cryptography`). It is a one-way hash: the server can compare two identifiers but cannot get the GUID from one;
- the app version.

The deactivate request has the license id and the machine identifier. The requests go to an HTTPS address, with a timeout of 10 seconds. The server answers with a signed token that is stored next to the license file, or with a refusal (unknown license, too many activations, revoked). Like any web server, it also sees your public IP address and the time of the request. For each license id the server has to remember which machine identifiers activated it, because that is how it counts the activations (3 at most per license).

**What is never sent.** No file name, folder path, rule, rule content or number of rules or files. Not the raw `MachineGuid`, not the computer name, not the Windows version, and not the email address in the license file, which stays on your computer and is shown in Settings. (The email is known to whoever sold you the license, from the purchase. Valet does not send it.)

**Where the license is stored.** In the app data folder, as `license.valet-license` (the license file, a UTF-8 JSON file with a signature) and `activation.valet-token` (the server's last answer). The contract says "the app data folder"; this is read here as `%APPDATA%\ro.valetapp.desktop`, the folder in section 2, and was not checked on an install.

**What happens without a connection.** A network error keeps the stored token. A token whose last successful check is 60 days old or less still gives Pro; older than that, Valet works as Free until the next successful check, and nothing is lost. If the server answers that the license is unknown or revoked (or, on a re-check, that there are too many activations), Valet removes the token and works as Trial or Free. The license file stays, so Settings can show why.

**How removal deactivates.** Removing the license in Settings sends the deactivate request, which frees that computer's activation on the server, so the license can be used on another computer. Uninstalling does not do this: the uninstaller makes no network call, so remove the license in Settings first. The contract does not say what happens to the request when there is no connection at that moment, nor in so many words that the two local files are deleted; neither is verified.

**When it is not active.** The server address and the public key come from two files in the repository, `crates/valet-core/license/api.txt` and `pubkey.txt`. When either is empty or missing, the app makes no license request at all, a Pro license cannot be activated, and the trial and Free plan work as described. At the time of writing both files are empty (0 bytes), so a build made from this checkout has no license check. A release build has one only if the repository owner has filled the files in before the build.

### Other connections

- The installer downloads the Microsoft WebView2 bootstrapper, and WebView2 itself, only if WebView2 is not installed on the computer (`downloadBootstrapper` in `tauri.conf.json`; per `docs/RELEASE.md`, section 6). This is the installer talking to Microsoft, not Valet.
- WebView2 is a Microsoft component with its own update service. What it contacts is outside Valet's code and is not covered here.
- No network capture was made while Valet ran, so what Tauri and its plugins do beyond the above is read from code, not observed.

## 2. What Valet stores on your computer

### In `%APPDATA%\ro.valetapp.desktop`

| File | Content |
|---|---|
| `valet.sqlite3` (with `-wal` and `-shm` files while open) | The rules and the operation journal. The journal holds the full source and destination path of every operation, sizes and content hashes (blake3) of the files Valet moved, copied or sent to the Recycle Bin, the state of each operation and error texts. It is never pruned: nothing deletes rows, so it grows with each batch. The History screen reads it. |
| `config.json` | The watched folders. |
| `app_settings.json` | Language, notification on or off, start with Windows, automatic organizing on or off, whether you finished the first-run guide, whether you approved a preview, the update channel and the detailed-log switch. |
| `approvals.json` | For each rule, the version that was approved and the watched folders of the preview that approved it. |
| `license.valet-license`, `activation.valet-token` | Only after you import a Pro license: the signed license file (it holds the license id, the email it was sold to, the number of allowed activations and the date until which updates are included) and the server's last signed answer. See "The license check". |

Before this version, the database also had a table of operation statistics (`operation_history`). Migration `0005_drop_operation_history.sql` drops it. The journal is the only history. The test `storage::tests::migration_0005_drops_operation_history_and_keeps_the_journal` checks that the table goes and the journal rows stay.

Start with Windows, if you turn it on, writes the value `Valet` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (current user only).

Files Valet sends to the Recycle Bin are in the Windows Recycle Bin, which Windows manages.

### In `%LOCALAPPDATA%\ro.valetapp.desktop\logs`

The log file is `valet.log`. Its place comes from Tauri's `app_log_dir` and the log plugin's `LogDir` target (read from their source; not checked on a Windows install).

- Rotation: a file is closed at 2 MB and renamed `valet_<date and time>.log`. At most 5 files are kept, the active one and 4 rotated, so the logs take at most about 10 MB. (`logging::tests::log_settings_values`.)
- Default level: INFO. The INFO lines in Valet's code carry counts, batch ids, version numbers and fixed text, for example "Dry-run completed: 12 operation(s) proposed, 3 skipped, across 2 path(s)". They do not carry file paths. This was read from the log calls in `apps/desktop/src-tauri/src`; no test checks log lines, and the error values that some WARN and ERROR lines print (operating system and database errors) were not checked for paths.
- Detailed log: Settings, Support, "Detailed log (for troubleshooting)". It switches the level to DEBUG at once, without a restart. DEBUG lines can contain file paths: the watcher error text and notification text are examples in Valet's code, and the libraries Valet uses write DEBUG lines too. The SQL layer and the HTTP stack, which write many lines, are held at INFO. The setting is off by default and stays on this computer.
- A panic (a program error that stops a thread) is written to the log as `Panic at <source file>:<line>: <message>`. Absolute paths in the message are replaced by `<path>` (`logging::tests::panic_hook_logs_message_and_location`, `scrub_replaces_absolute_paths`).
- Valet never sends the logs anywhere. If you send a log to someone for support, read it first, especially one written with the detailed log on.

## 3. What Valet does not have

- **No telemetry.** Nothing counts what you do, and there is no analytics library in `Cargo.toml` or `package.json`.
- **No crash reports.** Sentry is not built in. A crash or panic leaves a line in the local log and nothing else. If crash reports are added later, they will be opt-in (off until you turn them on), a screen called "Ce trimitem" (what we send) will show exactly what is sent, and paths will be removed before sending (PLAN 4.5). None of that exists yet.
- **No license check unless you import a Pro license.** Free and the trial never contact a license server. With a license imported, the check is the one described in section 1, and only in a build that has a license server configured.
- **No account, no advertising, no cloud copy** of your rules, journal or files.

Nothing prevents a future version from adding a network call, and no test fails when one is added. The two things in the code that pin the update behavior are `updater::tests::endpoints_match_the_release_config_script` (the two addresses above are the ones the release workflow writes into the build) and `updater::tests::not_configured_without_updater_section` (`tauri.conf.json` has no updater section; release builds add it).

## 4. Uninstalling

The installer is per user and uses the Tauri NSIS template. On uninstall:

- Your data is kept by default. The page of the uninstaller has a checkbox "Delete the application data" (in the Romanian installer, "Șterge datele aplicației"), unticked at the start.
- With the box ticked, the uninstaller removes `%APPDATA%\ro.valetapp.desktop` (settings, rules, approvals and the journal, so History and undo are gone) and also `%LOCALAPPDATA%\ro.valetapp.desktop` (the logs). This was read from the template of `tauri-bundler` 2.6.1 and 2.10.4 (`RmDir /r` on both folders); no uninstall was run on Windows.
- The start-with-Windows entry is removed on uninstall, whether or not the box is ticked (same template).
- With the box unticked, a later install finds the old journal and settings and carries on. The upgrade tests below cover that for databases of earlier versions.
- The license files are in the same folder, so they are kept by default and removed with the box ticked. The uninstaller does not contact the license server and does not free the activation; remove the license in Settings before uninstalling if you want the activation back.
- Files Valet moved or copied stay where they are. Files in the Recycle Bin stay there. Uninstalling does not undo anything.

To remove everything by hand, delete the two folders named above.

## 5. How to check this yourself

From the repository root:

```
grep -rn "releases/latest/download\|updater-channel" apps/desktop/src-tauri/src/updater.rs
cargo tree -p app -i reqwest
cargo test -p app updater::
cargo test -p app logging::
cargo test -p valet-core --test upgrade_journal
wc -c crates/valet-core/license/api.txt crates/valet-core/license/pubkey.txt
```

The first shows the two addresses. The second shows which crate uses the HTTP library. The tests are the ones named above. The last prints the size of the two license configuration files; 0 for both means that the build has no license check (it printed 0 and 0 for the checkout this section was written from).
