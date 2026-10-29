# Reddit r/software announcement

**CONDITION (Vlad, 2026-10-29): post nothing until both are true.**

1. The updater has been tested from `1.0.0-beta.1` to `1.0.0-beta.2` on a Windows virtual machine, and it worked.
2. Vlad has used Valet daily for 7 days.

Until then this file is a draft and nothing is announced anywhere.

Fixed facts for the post:

- Domain: `valetapp.ro`. Site: https://valetapp.ro (English: https://valetapp.ro/en/). Privacy page: https://valetapp.ro/en/privacy.html. These addresses work only after the site in `site/` is online on the domain; check them before posting.
- Support: GitHub Issues, https://github.com/OsztrovszkyVlad/Valet/issues. There is no support email address; do not add one.
- Selling: Pro can be bought only once the merchant webhook has passed a sandbox purchase and refund. If selling is not open on the day you post, change every sentence about the Pro price so that it says "buying is not open yet". If it is open, the refund policy is 14 days from purchase, and the license is revoked on refund.

**DRAFT — Vlad decides.** Nothing here is posted. Edit freely. Fill in the `[brackets]`. Before posting: check the current rules of the subreddit on self-promotion and on flair (not checked when this draft was written), and post only after a beta release is published on GitHub and the download link below works.

Facts in this draft come from `README.md`, `docs/LICENSING.md`, `docs/PRIVACY.md`, `docs/SAFETY.md` and `docs/PLAN.md` section 3.1. If any of them changes before posting, change the draft.

---

**Title (pick one):**

- `Valet: a rules-based file organizer for Windows, with preview and undo (beta, free tier, MIT)`
- `I made a file organizer for Windows that shows what it will move first, and can undo it (beta)`

**Body:**

Hi, I'm Vlad. I wrote Valet, a file organizer for Windows. It is in beta, and I'm looking for people to try it on their Downloads and Desktop folders and tell me what breaks.

What it does:

- You pick folders to watch (Downloads and Desktop are pre-checked in the first-run guide). You write rules, or use the 8 ready-made ones: Downloads by type, Screenshots, Invoices and receipts, Large files (report only), Duplicates (report only), Clean desktop, Old installers, Empty folders.
- A rule has conditions (extension, file type, part of the name, a regular expression, size, age, duplicate) and an action: move, copy, send to the Recycle Bin, or only report.
- Before anything moves, Valet shows a preview: file, from, to, which rule. Automatic organizing stays off until you have approved a preview.
- A finished batch can be undone while the moved files are unchanged. If you changed one, it asks first.
- It never deletes permanently (Recycle Bin only) and never overwrites: a taken name becomes `name (2).ext`. Every operation is written to a journal before it runs, and after a crash the app decides each unfinished operation at the next start.
- It skips files that are still being written, locked files, and OneDrive/Dropbox files that are not downloaded.

What it is not: no AI, no cloud, no account, no ads. It cannot rename or tag files yet.

Plans:

- Free: all 8 ready-made rules, 3 own rules, 2 watched folders, 7 days of history, undo of the latest batch.
- Pro, 29 EUR with VAT included, one-time: unlimited rules and folders, full history, undo of any batch, export and import of rules. Includes 12 months of updates; renewing is optional (15 EUR a year), and the version you have keeps working. There is a 14-day trial of Pro, no card.

Things you should know before installing:

- It is a beta. Don't point it at folders whose contents you can't lose. Use the preview.
- The installer is not code-signed yet, so Windows SmartScreen will warn. "More info", then "Run anyway". I have not got a signing certificate yet.
- Network: Valet checks GitHub for a new version (30 seconds after start, then every 24 hours; you can switch the automatic check off in Settings, and it never installs without your click), and, only if you import a Pro license, it checks the license (license id, a one-way hash of the machine id, and the app version). No file names or paths are sent. Details: https://valetapp.ro/en/privacy.html
- There are no crash reports. If it crashes, a line goes to a local log file and nowhere else.
- Tested so far: unit and property tests, a test that kills the process in the middle of a batch and checks each file exists exactly once after recovery (on Linux, with a fake Recycle Bin), and Windows tests in CI. Not tested: real OneDrive/Dropbox placeholders, USB drives and network shares, power loss. The list of what is and isn't covered is in the repository: `docs/SAFETY.md`.
- Windows 10 22H2 and Windows 11, 64-bit. The Microsoft Store listing is in preparation and is not live.

Download: [GITHUB RELEASES LINK]
Source (MIT): https://github.com/OsztrovszkyVlad/Valet
Site: https://valetapp.ro

If something goes wrong, I want to know: https://github.com/OsztrovszkyVlad/Valet/issues. If a file seems to be missing, check the Recycle Bin and the History screen first; the log is at `%LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log`.

Questions I'd like answers to: which rules would you write first, and what in the preview screen was unclear?

---

Notes for Vlad:

- The comparison with Hazel is left out of the title and the first lines on purpose. If someone asks, a short honest answer: same idea, different program, Windows only, no tags or rename yet.
- Reply to comments about the unsigned installer without defending it; say what the plan is (a free certificate for open-source projects, not obtained yet).
