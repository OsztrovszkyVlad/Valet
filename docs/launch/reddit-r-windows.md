# Reddit r/windows announcement

**DRAFT — Vlad decides.** Nothing here is posted. Fill in the `[brackets]`. Before posting: read the current rules of the subreddit (not checked when this draft was written). Some subreddits restrict posts about your own software or limit them to certain days or threads; if r/windows does, use the allowed place or skip it.

---

**Title (pick one):**

- `Valet: a folder organizer for Windows. Rules, a preview before anything moves, undo, Recycle Bin only (beta)`
- `Feedback wanted: a rules-based Downloads and Desktop organizer for Windows 10/11 (beta, MIT)`

**Body:**

Hi, I'm Vlad. I'm the developer of Valet, a small Windows program that keeps folders like Downloads and Desktop in order by rules. It's a beta, and I'd like feedback from people on real Windows setups.

What it does, in plain terms:

1. You choose which folders it watches.
2. A rule says "files like this go there": by extension or file type, by part of the name, by size, by age, or "empty folder", "duplicate". Actions: move, copy, send to the Recycle Bin, or only list.
3. Before anything moves, Valet shows a preview of every move. Automatic mode only turns on after you approve a preview.
4. You can undo a batch while the moved files are unchanged.

What it won't do: delete permanently (Recycle Bin only), overwrite a file (a taken name gets `(2)`), or touch Windows, Program Files, AppData, `.git`, `node_modules` and hidden or system files, unless you opt in. It skips files being downloaded or locked, and OneDrive files that are not downloaded.

How it behaves on the machine:

- Per-user installer, no administrator rights. Runs in the tray, closing the window hides it. Start with Windows is optional, off until you turn it on.
- Windows 10 22H2 and Windows 11, 64-bit. It uses WebView2; the installer downloads it if it is missing.
- Network: an update check against GitHub (a static JSON file), and a license check only if you import a Pro license. No file names or paths leave the computer. [PRIVACY LINK]
- No telemetry, no crash reporting, no account, no ads.

Honest state of the beta:

- The installer is unsigned. SmartScreen will say "Windows protected your PC". "More info", "Run anyway". I have no certificate yet.
- I have not tested real OneDrive or Dropbox placeholders, USB drives, network shares or power loss. A drive without a Recycle Bin makes Valet refuse the action instead of deleting.
- The install, update and uninstall flow has not been tested on Windows 10 and 11 virtual machines yet. [UPDATE THIS LINE BEFORE POSTING if it has been.]

Free plan: 8 ready-made rules, 3 own rules, 2 folders, 7 days of history, undo of the last batch. Pro is 29 EUR once (VAT included), with a 14-day trial; details on the site.

Download: [GITHUB RELEASES LINK]
Source (MIT): https://github.com/OsztrovszkyVlad/Valet
Report a problem: [LINK OR EMAIL]

What I most want to hear: any file that ended up somewhere you did not expect, and any screen you did not understand.
