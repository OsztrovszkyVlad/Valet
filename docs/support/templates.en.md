# Support reply templates (English)

Version în română: [templates.ro.md](templates.ro.md)

Short replies for the cases that come up most. Replace the `{placeholders}`. Edit freely; these are starting points, not scripts.

Rules for every reply:

- Never ask for the contents of a file. Names and paths can be sensitive too: ask for them only when needed, and let the person look at what they send first.
- The log can hold file paths when the detailed log was on. Say so every time you ask for it.
- Do not promise a fix date.
- The support channel is GitHub Issues (https://github.com/OsztrovszkyVlad/Valet/issues). There is no support email address. The pages are public: do not ask for, and do not leave in them, email addresses, license files, logs that were not read, or the person's files. If someone pastes them, ask them to edit the message.

Where things are (as in `docs/PRIVACY.md`):

- Log file: `%LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log` (older ones are `valet_<date and time>.log` in the same folder, at most 5 files).
- Detailed log: Settings, Support, "Detailed log (for troubleshooting)". It takes effect at once, without a restart. It is off by default.
- Data folder: `%APPDATA%\ro.valetapp.desktop` (database with the journal, settings, license files; removed licenses are in the subfolder `license-removed`). In the Microsoft Store version the data is in a folder of the package, under `%LOCALAPPDATA%\Packages\`, and the logs are in `%LOCALAPPDATA%\ro.valetapp.desktop\logs` only in the usual variant (not checked in the Store version).
- Plan and license: Settings, Valet Pro. It also shows "License code (for support)". With this code, plus an order lookup in the merchant account (the order number is in the merchant's confirmation email), support finds a license. No link between email and license is kept.
- Version: Settings, About.

---

## 1. Bug report received

```
Hi {name},

Thanks for the report. To find the cause I need a few things:

1. The Valet version (Settings, About).
2. Your Windows version (Start, type "winver", Enter) and whether it is Windows 10 or 11.
3. The steps that lead to the problem, as exact as you can: which screen, which button, what you expected and what happened.
4. The log file: %LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log
   (paste this into the Explorer address bar). Please attach it, or the last 50 lines.

If the log does not show enough, you can switch on more detail:
Settings, Support, "Detailed log (for troubleshooting)". Repeat the steps, then send the log again.
Note: the detailed log can contain file paths. Open the file in Notepad and read it before you send it;
remove any line you do not want to share. Valet never sends logs by itself.

Please do not send any of your files.

Vlad
```

---

## 2. A file went missing (urgent)

Answer the same day. A lost file is the one report that can end the product (`docs/PLAN.md`, section 8, risk 1). First help the person find the file, then collect what is needed to find the cause.

```
Hi {name},

I am sorry about this, and I want to get it fixed. Valet never deletes a file permanently,
so the file should still exist somewhere. Please check these, in this order:

1. Recycle Bin. If a rule sent the file there, it is in the Bin. Search the Bin for the file name.
   If you have not emptied the Bin, you can restore it from there.
2. Valet, History. Find the batch from the time the file disappeared. Each row shows where the file
   was and where it went. Open the destination folder. If the name was already taken there,
   the file has a number in it, for example "report (2).pdf".
3. Undo. In History, the "Undo batch" button on the batch puts the files back, as long as they
   have not been changed since. (On the Free plan only the most recent batch can be undone;
   if the batch is older, History still shows the destination, and you can move the file back by hand.)
4. Windows search for the file name, in case it was moved by hand or by another program.

If you cannot find it, please send me:

- the Valet version (Settings, About) and your Windows version;
- the name of the rule that was active, and what the History row says (a screenshot of that row is enough);
- whether the file was on OneDrive, Dropbox, a USB drive or a network folder;
- the log file: %LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log
  It normally has no file paths. If you switched on the detailed log it can have some, so read it first.

Please do not close Valet or empty the Recycle Bin until we know more, and do not send the file itself.

Vlad
```

Internal notes for this case:

- The journal (`valet.sqlite3` in the data folder) has the full path of every operation. Do not ask for the database by default: it holds paths and hashes of many files. If it is needed, ask the person to agree first, and ask only for the rows of the batch in question.
- A file that went to the Recycle Bin cannot be restored by Valet after the Bin was emptied (`docs/SAFETY.md`, invariant 4). Do not promise any other recovery.
- Keep a count of these reports with the Valet version and the outcome. The plan's target for the first three months is zero lost-file reports (`docs/PLAN.md`, section 0).

---

## 3. Feature request

```
Hi {name},

Thanks for the idea. To understand it properly: what would you like Valet to do, for which kind of files,
and what do you do today instead? An example (file type, folder, where it should end up) helps more than a description.

I keep a list of requests and read all of it. I cannot say if or when this will be built.

Vlad
```

Notes: renaming files by a template is the first feature planned after launch. Tags are not planned. Disk-space actions are planned for a later phase. Say "planned", never a date.

---

## 4. Refund request

Policy (decided by Vlad, 2026-10-29): refund within 14 days of purchase, through the merchant that processed the payment; the license is revoked on refund. Buying is not open yet, so this template is not sent until it opens.

```
Hi {name},

Yes, you can get your money back within 14 days of purchase. The refund goes through the merchant that processed
the payment (the order number is in the confirmation email you got from it), not directly through Valet.

What happens after the refund:
- The license is revoked. Valet learns about it at the next license check (at most once every 30 days);
  if you want it at once, open Settings, Valet Pro and press "Check license now".
- Valet goes back to the 14-day trial, if it has not ended, or to the Free plan.
  Nothing is lost: your rules, folders, history and settings stay. Whatever goes over the Free plan limits
  is switched off, not deleted, and comes back if you move to Pro again.
- The license file stays on the computer, so Settings can show why it is no longer active.

Please do not put your email address, the license file or the order number on this page: GitHub Issues is public.

Vlad
```

Internal notes:

- The license service withdraws a license with `services/license/revoke.mjs` and, when the merchant webhook is ready, on the merchant's refund event (`docs/LICENSING.md`, "License service"). After a revoke, Valet refuses it with "The license was revoked."
- How to find the license: the person gives you the first 8 characters of "License code" (Settings, Valet Pro); the server keeps the merchant order ids, not the email.
- [VLAD: once the merchant is chosen, add the exact steps for the buyer (a link or a button at the merchant), or say whether you trigger the refund from the merchant account. Not decided, because it depends on the merchant.]

---

## 5. License does not activate

```
Hi {name},

Let us find out why. What does Valet say when you import the license (the exact sentence, or a screenshot)?
Common causes:

- "The license is already active on 3 computers". A license works on up to 3 computers. If you no longer use one of them,
  open Valet on that computer, go to Settings, Valet Pro, press "Remove the license from this computer",
  and import the license again here. Removing it frees that computer's place. Uninstalling Valet does not free it.
  If the old computer is gone (broken, formatted), or the license was removed without internet,
  reply here with the first 8 characters of "License code (for support)" (Settings, Valet Pro). Do not put
  your email address or the license file on the page: GitHub Issues is public. I can clear the license's list
  of active computers on the server. After that you import the license on the new computer (Settings, Valet Pro, "I have a license").
  The reset clears only the list of active computers; the license stays as it is.
- No internet at the moment of import. Importing needs one connection to the license server.
  After that, Valet checks again at most once every 30 days. If the connection fails later,
  Pro keeps working for up to 60 days after the last successful check, then the app works as Free
  (nothing is lost) until the next successful check.
- "The license does not cover this version". Your license includes updates for 12 months from purchase.
  The message names the date. Versions released before that date stay Pro; later ones need
  the update renewal (15 EUR per year). Older versions keep working.
- The file was changed or damaged (for example opened and saved in an editor). Use the original
  .valet-license file from the email.

The check sends only the license id, a machine identifier (a one-way hash) and the app version.
Details: the privacy page.

Please also tell me the Valet version (Settings, About) and your Windows version.

Vlad
```

Internal notes for this case:

- Vlad does the reset, on the server: `node services/license/reset.mjs --license <id> --store <file>` (or with the variable `VALET_LICENSE_STORE`). The command clears the active machines of the license; the machine history and the license record stay (`services/license/reset.mjs`, `docs/LICENSING.md`, "License service"). The user cannot do it.
- The command needs the license id (a UUID), which the user can read in Settings → Valet Pro ("License code (for support)"). No link between email and license is kept (Vlad's decision, 2026-10-29). The license is found by the code in Settings or by the order number in the merchant account: the server keeps the order ids and the license id, not the email. The full code should not be put on a public page: anyone who has it can take an activation slot (the activate request needs only the license id and the machine id). Ask for the first 8 characters and look up the full id in the server's store. [VLAD: confirm that 8 characters are enough to find the license in the store; not verified.]
- Do not ask for the `.valet-license` file if the email is enough. The file has the id, the email and the license data, not the person's files.
- Tell the person not to use the license on the old computer if they repair it later: it would take a place again.

---

## 6. SmartScreen question

```
Hi {name},

The warning is expected. The Valet installer is not code-signed yet, and Windows SmartScreen shows
"Windows protected your PC" for programs that are unsigned and not yet widely downloaded.
It is not the result of an analysis of Valet. A free signing certificate for open-source projects
has not been obtained yet, so I cannot say when the warning will go away.

To continue: select "More info", then "Run anyway".

If you want to check the file first:
- Valet is open source (MIT license): https://github.com/OsztrovszkyVlad/Valet
- In PowerShell: Get-FileHash .\Valet_<version>_x64-setup.exe -Algorithm SHA256
  and compare the result with the SHA-256 listed by the "Release" run in GitHub Actions.

If "Run anyway" does not appear, your computer may be set by an administrator (or by Windows S mode)
to block programs that are not signed. I cannot change that from here.

Vlad
```
