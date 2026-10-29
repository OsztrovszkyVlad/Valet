# Show HN

**DRAFT — Vlad decides.** Nothing here is posted. Fill in the `[brackets]`. Before posting, read the current Show HN rules (not checked when this draft was written). As far as I know, people must be able to try what is shown without signing up; the free tier needs no account, and the installer is on GitHub. Check that this still holds when you post.

---

**Title (pick one, keep it plain):**

- `Show HN: Valet – rules-based file organizer for Windows, with preview and undo`
- `Show HN: Valet – Rust and Tauri file organizer for Windows (MIT)`

**URL:** the GitHub repository, `https://github.com/OsztrovszkyVlad/Valet`, or the site: [SITE LINK]. Pick one; the repository is the safer one for this audience.

**First comment (post it right after submitting):**

I'm Vlad. Valet watches folders on Windows (Downloads, Desktop) and applies rules: move, copy, send to the Recycle Bin, or only report. You see a preview of every operation before anything runs, and a finished batch can be undone while the moved files are unchanged.

[Vlad: one or two sentences on why you built it. Leave out anything that is not true.]

How it is built, since this is HN:

- Rust core (`valet-core`) with no OS code, so it builds and tests on Linux. A Windows crate holds the Recycle Bin (the `trash` crate), watcher, placeholder detection and long paths. The app is Tauri 2 with a React front end. Journal and rules are in SQLite.
- Every file operation is written to a journal before it runs and marked after. On start, the app decides each unfinished operation from what is on disk.
- Moves never overwrite: the Windows call is `MoveFileExW` without the replace flag, and a taken name gets the next free name.
- Undo compares a blake3 hash of the file at the destination with the hash taken when it was moved. If it differs, it asks.
- Testing: unit and property tests (planner, collision naming), and a test that runs a batch in a child process, kills it at a random time, runs recovery, checks each file exists exactly once and then undoes the batch. That runs on Linux with a fake Recycle Bin. The Windows API calls are only covered by tests in CI on a Windows runner. `docs/SAFETY.md` lists what is covered and what is not, including two cases where a file can end up at both source and destination after a double failure (never zero copies).

Limits you will want to know about:

- Beta. The installer is not code-signed, so SmartScreen warns.
- Not tested: real OneDrive/Dropbox placeholders, USB drives, network shares, power loss.
- No rename or tag action yet. No AI.
- The update check (GitHub, static JSON) runs automatically and has no off switch yet. A license check runs only if you import a Pro license. No file names or paths are sent. `docs/PRIVACY.md` has the details.
- The code is MIT. The Pro limits are enforced by the program itself, and the source is public, so anyone can build a copy without them. I'm fine with that. Pro is 29 EUR, one-time, with 12 months of updates; the free tier is 3 own rules and 2 watched folders. [Vlad: confirm you want this sentence.]

Download: [GITHUB RELEASES LINK]

I'd like to hear about file-safety edge cases I did not think of.

---

Notes for Vlad:

- On HN, answer technical questions directly and say "not tested" where that is the case. Do not argue about the SmartScreen warning.
- If someone asks about Hazel: same idea, separate program, Windows only. Do not claim feature parity; there is no rename or tag action.
