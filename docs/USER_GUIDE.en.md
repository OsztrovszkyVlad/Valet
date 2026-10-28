# Valet user guide

Versiunea în română: [USER_GUIDE.ro.md](USER_GUIDE.ro.md)

Valet puts your files in order, using rules you can see and undo. This guide describes what the app does in the version in this repository.

Valet is still in testing. It has no public installer and no release. You start it from the source code (see [README.md](../README.md)). Until you trust it, work on folders where you have copies of your files.

The screenshots show test folders, which is why the paths start with `/tmp/valet-e2e-…`. They were taken from the real app, on Linux, by the automatic UI smoke test. The app was in Romanian, so the screenshots show Romanian text. On Windows, paths look like `C:\Users\name\Downloads`. The names below are the English interface names.

## Contents

1. [What Valet does and does not do](#1-what-valet-does-and-does-not-do)
2. [First run](#2-first-run)
3. [The Preview](#3-the-preview)
4. [Organize](#4-organize)
5. [Undoing a batch](#5-undoing-a-batch)
6. [History](#6-history)
7. [The 8 presets](#7-the-8-presets)
8. [Writing your own rules](#8-writing-your-own-rules)
9. [Automatic organizing](#9-automatic-organizing)
10. [Tray and starting with Windows](#10-tray-and-starting-with-windows)
11. [Settings](#11-settings)
12. [What Valet never touches](#12-what-valet-never-touches)
13. [Drives without a Recycle Bin](#13-drives-without-a-recycle-bin)
14. [Privacy](#14-privacy)
15. [Where the data is](#15-where-the-data-is)
16. [Known limits](#16-known-limits)

## 1. What Valet does and does not do

**What it does**

- It watches the folders you choose, for example Downloads and Desktop.
- It applies the active rules to the files in those folders. A rule can move a file, copy it, send it to the Recycle Bin, or only report it.
- It shows a list of what it would do (the preview) before it touches a file.
- It writes every operation to a journal before it does it. You see the journal on the History screen.
- It can undo a whole batch.
- If the app or the computer stops in the middle of a batch, it checks the unfinished operations at the next start.
- It can organize new files by itself, but only after you have approved a preview (see [section 9](#9-automatic-organizing)).

**What it does not do**

- It never deletes anything permanently. Every removal goes to the Windows Recycle Bin.
- It never overwrites a file. If the name at the destination is taken, the file gets `name (2).ext`, then `name (3).ext`, and so on.
- It does not rename files and it does not tag them. These actions do not exist in the current version.
- It does not read file contents to classify files. Rules look at the name, extension, type, size, dates and path. The only reading of contents is the hash check of files it moves, copies or sends to the Recycle Bin, and the comparison of same-size files for duplicate rules.
- It does not use the internet (see [section 14](#14-privacy)).
- It does not touch system folders, hidden files, or OneDrive and Dropbox files that are not downloaded (see [section 12](#12-what-valet-never-touches)).

## 2. First run

The first time you start Valet, a 3-step guide appears. Nothing moves during it. The screens are in Romanian. You change the language in Settings, after you finish the guide. The presets installed on first run use the folder names of the language that is active then, so Romanian ones (Imagini, Documente and so on).

**Step 1 of 3: folders**

![Step 1: choosing folders](images/onboarding-1-folders.png)

- Downloads and Desktop are checked. Documents and Pictures are not.
- You can add another folder by typing its path in "Add another folder", for example `C:\Users\name\Projects`. There is no folder picker.
- You must check at least one folder.
- Valet refuses a folder if it is a whole drive (`C:\`, `D:\`, `\\server\share`), if its path contains a default-excluded name (for example `AppData` or `Program Files`), or if it does not exist. Valet checks the folders when you press "Continue" on step 2. If one is refused, you get a message and go back to step 1.
- You can change the list at any time in Settings.

**Step 2 of 3: rules**

![Step 2: choosing presets](images/onboarding-2-presets.png)

- You see the 8 presets, each with a description and its number of rules. The ones checked by default are Downloads by type, Screenshots, Invoices and receipts, and Large files.
- The "sends to Recycle Bin" badge marks presets that send files to the Recycle Bin. The "report only" badge marks presets that move nothing.
- "Skip" continues without any preset. "Continue" saves the checked folders and presets.
- What each preset does is in [section 7](#7-the-8-presets).

**Step 3 of 3: the preview**

After "Continue", Valet saves your choices and runs a preview. You see it on the Preview screen, with a blue "Step 3 of 3" note. Nothing moves until you press "Organize files".

![The preview at step 3](images/preview.png)

## 3. The Preview

The Preview screen has the button "Run preview". The preview reads the watched folders and lists what the rules would do. It changes no file.

After it runs you see two numbers: "Planned operations" and "Skipped files". If there is no planned operation, you see "Nothing to organize". Reports, if any, still appear.

### The "What will happen" table

| Column | Meaning |
|---|---|
| File | The name of the file or folder. |
| Operation | `move`, `copy` or `to Recycle Bin`. |
| From | The current path. |
| To | The destination path. For "to Recycle Bin" it says "To Recycle Bin". |
| Rule | The name of the rule that decided. |

- Long paths are shortened in the middle. Hold the mouse over one to see the full path.
- If the name at the destination is already taken, the "To" column already shows the new name, for example `report (2).pdf`.
- Under the numbers you may see "Rules ignored for files already planned: N". It means one file matches several rules. The first rule in the Rules list that moves, copies or sends to the Recycle Bin is used. The others are ignored for that file.
- A file gets at most one operation.

### Skipped files

"Skipped files: N" has a "Show" button. The table has the columns File, Reason and Rule. Files that no rule wants do not appear there.

| Reason shown | Meaning |
|---|---|
| Excluded (system folder, AppData, .git, node_modules, etc.) | The folder is excluded by default or contains a `.valetignore` file. There is one line for the whole folder. |
| Hidden or system file | The name starts with a dot, or it has the Windows hidden or system attribute. A rule can ask for "Include hidden files". |
| Link (symlink or junction) | Valet does not follow or move links. |
| Still being written or locked | The file has the extension `.crdownload`, `.part`, `.tmp`, `.download` or `.partial`, its name starts with `~$`, or it was modified in the last 10 seconds. |
| OneDrive/Dropbox on-demand file, not downloaded | The file exists only in the cloud. Valet does not open it, so it does not download it. |
| Another rule applies before this one | The file matches several rules. It shows as a row only if the file got no operation. |
| Already at the destination | The file is already in the folder it would be moved to. |
| The file has no extension; the rule uses {ext} | The rule's destination contains `{ext}` and the file has no extension. |
| The rule's destination is not valid | The destination folder cannot be built: unset environment variable, unknown token, missing folder, or a destination inside an excluded folder or under a `.valetignore`. |
| The rule's action is not supported yet | The rule has an action Valet does not run (for example `rename` or `tag`). New rules made in the app cannot have one. |
| Cannot be read | Valet could not read the folder or the file, or could not hash it for a duplicate rule. |
| Empty folder kept: it is the destination of a rule | An empty folder that a rule would send to the Recycle Bin, but another rule moves files into it or below it. Example: a `Music` folder that is still empty. |

A file locked by another program does not appear here. The lock only shows at "Organize", as an error (see [section 4](#4-organize)).

### Reports

Rules with the action "Report only" do no operation. Their files appear under "Reports", with the text "Information only: nothing happens to these files."

- **Large files:** a table with the path and size of each file found. The Large files preset considers files over 1 GB, that is over 1,073,741,824 bytes.
- **Duplicates:** groups of files with identical content: "Group 1, 12 MB each", followed by the file paths. If a rule sends duplicates to the Recycle Bin, the copy that stays has the badge "kept". It is the oldest by modification date. On a tie, the shorter path wins, then the first in alphabetical order.
- A file can appear both in a report and in the operations table. For example, a large archive is moved by "Downloads by type" and listed by "Large files".
- Empty files are not duplicates. Hidden files are not part of the duplicate report.
- For duplicates, Valet reads contents only for files that have the same size as another candidate, and never for OneDrive or Dropbox files that are not downloaded.

### How long a preview stays valid

- The preview is kept only in memory. It is used once.
- It is lost if you run another one, clear it ("Clear result"), or restart the app. If you press "Organize" on a lost one, you see "The preview has expired. Run the preview again."
- While a preview is open, automatic organizing waits (see [section 9](#9-automatic-organizing)).

## 4. Organize

The button "Organize files (N)" appears when the preview has at least one operation. N is the number of operations.

- Valet runs exactly the list you see. It does not make a new list, so files that appeared after the preview are not touched.
- Before each file, Valet checks again: the file exists, it has the same size and modification date as in the preview, it is not an on-demand file, and it is not locked.
- A failed operation does not stop the rest of the batch.
- Move on the same drive: the file is moved directly. Valet replaces no existing file.
- Move between two drives: Valet copies the file, compares the copy with the original by hash, then sends the original to the Recycle Bin. If anything does not match, the copy goes to the Recycle Bin and the original stays where it was.
- For "to Recycle Bin" on a folder, Valet checks again, right before the operation, that the folder is still empty. If something appeared in it meanwhile, the operation fails and the folder stays.
- At the end you see "Operations done: N." or, if there were errors, "Done: X. Failed: Y. Details below." with a table of the failed operations.
- Pressing "Organize" also approves the rules in that preview for automatic organizing (see [section 9](#9-automatic-organizing)).
- After the batch a short notification appears, for example "5 files organized". You can turn it off in Settings.

![After "Organize": the "Last batch" box with the "Undo batch" button](images/after-organize.png)

### Error messages

| Message | Meaning |
|---|---|
| The file no longer exists | The file disappeared between the preview and the operation. |
| The file changed since the preview | Size or date changed, it is no longer a regular file, or a folder is no longer empty. Nothing was touched. |
| On-demand file, not downloaded | A OneDrive or Dropbox file that is only in the cloud. |
| The file is locked by another program | Another program holds the file open. Nothing was touched. |
| This volume has no Recycle Bin; Valet does not move or copy files here. | See [section 13](#13-drives-without-a-recycle-bin). |
| Verifying the copy failed | The copy does not match the original. The original stays, the copy goes to the Recycle Bin. |
| Could not send to the Recycle Bin | Windows refused. The file stays where it was. |
| The destination is taken | A code kept in the journal. In undo, when a name gets taken at that very moment, Valet asks for confirmation instead of showing this error. |
| Interrupted by the app stopping | The operation was left unfinished after a stop. See [section 6](#6-history). |
| Invalid path | The path is not valid Unicode text, or it has no parent folder. |
| Read or write error | Another disk error, or missing permissions (for example a protected folder). Valet does not retry. |

After some messages, a technical note in English may follow in parentheses.

## 5. Undoing a batch

After a batch, the "Last batch" box appears at the top of every screen, with the button "Undo batch". It stays until you undo the batch or a new batch replaces it. If you restart the app, Valet shows it again for the newest organize batch, if it still has done operations.

![Confirming the undo](images/undo-confirm.png)

Steps:

1. Press "Undo batch". You see the list "Files to move back: N".
2. Press "Confirm undo" (or "Cancel").
3. You see "Undo result": "Operations undone", plus, when it applies, "Left as is (not confirmed)", "Skipped (not done in the batch)" and "Failed".

Undo works on a whole batch. There is no undo for a single file. If you want one file back, History shows where it went and you can move it yourself.

Any finished organize batch can also be undone from History, not only the last one.

### What happens in each case

| Situation | What Valet does |
|---|---|
| The file at the destination is unchanged and the original place is free | It moves the file back. |
| The file was changed after the batch (its content differs) | Nothing is moved. The file appears under "Files that need confirmation" with "The file changed after the batch". With "Undo anyway" it is moved back as it is now. |
| The original place is taken (a file with the same name exists there again) | Nothing is moved. You see "The original location is taken; the file will get a new name". With "Undo anyway", the file is put next to the existing one, as `name (2).ext`. The result says "X restored as Y". The existing file is not touched. |
| The file was sent to the Recycle Bin (a "To Recycle Bin" rule) | Valet restores it from the Recycle Bin to its place. The list says "restore from Recycle Bin". Empty folders sent to the Recycle Bin are restored too. |
| The file sent to the Recycle Bin is no longer in it (you emptied it) | Undo fails for that file with "The file no longer exists". |
| File sent to the Recycle Bin, and the original place is taken | With "Undo anyway", Valet does not restore it next to the existing file. The item stays in the Recycle Bin and you see "The original location is taken; the item stays in the Recycle Bin". You can restore it yourself from the Recycle Bin. Note: the confirmation list also says "the file will get a new name" for it, but that is not true for files in the Recycle Bin. |
| A file restored from the Recycle Bin has different content than when it was sent | It stays restored and is listed: "restored, but the content differs from when it was sent to the Recycle Bin". |
| The file was a copy (a "Copy to" rule) | The copy is sent to the Recycle Bin. If the original no longer exists, the copy is the last one, so Valet asks for confirmation: "The original is gone; undo sends this copy to the Recycle Bin". |
| The file at the destination is gone, locked, or on-demand | It fails for that file, with the reason. |
| Operations that failed in the original batch | They are not touched. They show under "Skipped". |
| Folders created by the batch | They stay, empty. Undo does not send them to the Recycle Bin. |
| Undo between two drives | It needs a Recycle Bin on both. Otherwise it fails with "This volume has no Recycle Bin" and the file stays where it is. |

Restoring from the Windows Recycle Bin does not yet have a passing test result on Windows (see [SAFETY.md](SAFETY.md)). If undo fails for a file that was sent to the Recycle Bin, open the Recycle Bin and restore it by hand.

Undo is a new batch, of type "Undo". A batch of type "Undo" or "Recovery after a stop" cannot be undone. A second undo of the same batch finds nothing to do.

### Restoring from the Recycle Bin by hand

Files Valet sent to the Recycle Bin are in the normal Windows Recycle Bin. You can restore them from there like any file, with right-click and the restore option. For files sent to the Recycle Bin by the recovery step (see [section 6](#6-history)), Valet has no undo. There, restoring from the Recycle Bin is done by hand.

## 6. History

The History screen lists the batches, newest first. At most 50 are shown. "Refresh" reloads the list.

![History](images/history.png)

The list has the columns Type, Date and Operations.

- **Type:** "Organize", "Undo" or "Recovery after a stop". A batch left open shows "(not finished)".
- **Operations:** how many operations are in each state: done, failed, undone, in progress, planned.

Press a batch to see its details: the batch id, the text "What happened", and the table of operations.

- The table columns are No., Operation, From, To, State and Error.
- The operation can be `move`, `copy`, "Sent to Recycle Bin" or "Restored from Recycle Bin".
- For finished organize batches that have done operations, the button "Undo batch" appears. It works as in [section 5](#5-undoing-a-batch).
- "Undone later: N" means those operations were undone by an undo batch.

### Recovery after a stop

If Valet or the computer stops in the middle of a batch, some operations stay unfinished. At the next start, before Valet begins watching folders, it checks each unfinished operation against what it finds on disk. It does not ask you.

- An operation that never started is marked failed. Nothing was touched.
- An operation that was half done is finished or abandoned, depending on the state of the files. A leftover temporary copy (`.valet-tmp-…`) goes to the Recycle Bin.
- The result is a "Recovery after a stop" batch, with the explanation of each decision under "Technical details" (in English).
- The interrupted batch is closed. Its details say "The batch was interrupted and closed at the next start."
- The goal is that each file stays in one place, at the source or at the destination.

What was checked: automatic tests, including one with 100 iterations in which the process is killed at a random time in the middle of a batch, and recovery runs afterwards. That test runs on Linux, with a simulated Recycle Bin. A power loss was not tested. Two rare cases, after a double failure, can leave one file both at the source and at the destination. The file is not lost. The operation shows as failed, with both paths in the Error column. Details in [SAFETY.md](SAFETY.md), invariant 3.

## 7. The 8 presets

A preset is a set of ready-made rules. You choose them on first run. After that they appear on the Rules screen, with the badge "preset", and you can turn them off, edit them or delete them like any rule.

A preset's rules apply in a folder only if that folder is in the watched folders list. For example, if Desktop is not watched, "Clean desktop" does nothing. If you watch `C:\Users\name` instead of Downloads, the Downloads presets do not match.

The names below are the English ones. The folder names are the ones used when the language is English at install time.

| Preset | What it does | Folders | Recycle Bin | Checked |
|---|---|---|---|---|
| Downloads by type | Moves files into subfolders of Downloads, by type: Images, Documents, Archives, Installers, Videos, Music. Only files directly in Downloads, not in subfolders. | Downloads | no | yes |
| Screenshots | Moves images named Screenshot, Captură, Snip or Clipboard to Pictures\Screenshots\YYYY. | Downloads, Desktop, Pictures | no | yes |
| Invoices and receipts | Moves PDFs named invoice, receipt, factura, chitanta or bon to Documents\Invoices\YYYY. | Downloads | no | yes |
| Large files | Lists files over 1 GB. Moves nothing. | Downloads, Documents | no | yes |
| Duplicates | Lists files with identical content. Moves nothing. | Downloads | no | no |
| Clean desktop | Moves Desktop files modified more than 7 days ago to Documents\From Desktop\YYYY-MM. Shortcuts (`.lnk`, `.url`) and `.ini` stay. | Desktop | no | no |
| Old installers | Sends installers (`.exe`, `.msi`, `.msix`, `.msixbundle`, `.appx`, `.appxbundle`) modified more than 30 days ago to the Recycle Bin. | Downloads | **yes** | no |
| Empty folders | Sends empty folders modified more than 7 days ago to the Recycle Bin. | Downloads | **yes** | no |

Notes:

- No preset that sends files to the Recycle Bin is checked by default.
- "Downloads by type" has 6 rules, one per type. Types are decided by extension:
  - Images: jpg, jpeg, png, gif, bmp, webp, tif, tiff, heic, heif, avif, svg, ico, psd, raw, cr2, nef, arw, dng.
  - Documents: pdf, doc, docx, xls, xlsx, ppt, pptx, odt, ods, odp, rtf, txt, md, csv, epub.
  - Archives: zip, rar, 7z, tar, gz, tgz, bz2, xz, zst, cab, iso.
  - Installers: exe, msi, msix, msixbundle, appx, appxbundle.
  - Videos: mp4, mkv, avi, mov, wmv, webm, m4v, mpg, mpeg, flv, 3gp.
  - Music: mp3, wav, flac, aac, ogg, m4a, wma, opus, aiff, mid, midi.
  - Files of any other type, and files without an extension, stay where they are.
- YYYY is the year and MM is the month of the file's modification date, not today's date.
- "Screenshots" and "Invoices and receipts" look for the whole word in the file name, ignoring upper and lower case. `Screenshot 2024.png`, `Snip_1.jpg`, `clipboard.png`, `Factura_123.pdf`, `bon fiscal.pdf` and `Invoice2024.pdf` match. `Snippet.png`, `screenshots-tutorial.png`, `bonus.pdf` and `refacturare.pdf` do not.
- "Invoices and receipts", "Large files", "Duplicates", "Old installers" and "Empty folders" also look in the subfolders of their folders. The other presets look only at what is directly in the folder.
- "Empty folders" never touches the watched folder itself. It keeps empty folders that are destinations of other rules, for example a `Music` folder that is still empty.
- Evaluation order of the presets: Invoices and receipts, Screenshots, Old installers, Downloads by type, Clean desktop, Large files, Duplicates, Empty folders. With "Old installers" and "Downloads by type" both on, an installer older than 30 days goes to the Recycle Bin and a newer one goes to Installers.
- "Report only" rules do not stop other rules from moving the same file.
- Changing the language later does not rename the rules already installed.
- The Rules screen has no button to reinstall a deleted preset. You can rebuild it with your own rule.

## 8. Writing your own rules

The Rules screen lists the rules. For each one you see:

- the "On" checkbox;
- the name, with the badge "preset" on the ones from presets;
- a summary (conditions → action);
- the folders it applies to ("In all watched folders" or "In: …");
- the buttons "Edit" and "Delete". Delete asks first, and no file is touched.

![The Rules screen](images/rules.png)

"Add rule" opens the editor. Saving a rule touches no file.

### Conditions

A rule applies to a file only if it meets all the conditions. You can also check "Applies to all files (no conditions)".

| Condition | What it checks |
|---|---|
| Extension (one) | The file extension, for example `pdf`. Upper and lower case do not matter. |
| Extension, one of | The extension is in the list. |
| Extension, none of | The extension is not in the list. Files without an extension pass. |
| File type | Images, Documents, Archives, Installers, Video, Audio or Code. The lists are in [section 7](#7-the-8-presets); "Code" has rs, py, js, ts, html, css, json, xml, yaml, sql and more. |
| Name contains | One or more texts. One of them is enough. Case does not matter. Only the file name is checked. |
| Name matches expression | A regular expression on the file name. It is case-sensitive unless it starts with `(?i)`. A wrong expression gets "The regular expression is not valid." |
| Path contains | A text anywhere in the full path. |
| Larger than, Smaller than | The size, in KB, MB or GB (1 KB = 1024 bytes). |
| Modified more than (days) ago | More than N whole days have passed since the last modification. N is from 0 to 36500. |
| Created more than (days) ago | The same, by creation date. If the system gives no creation date, the condition is not met. |
| Empty folder | The rule sees only folders with nothing in them. It cannot be combined with file conditions (extension, name, type, size, duplicate). The action can only be "To Recycle Bin" or "Report only". |
| Duplicate (same content) | The file has an identical copy among the files the other conditions accept. The action can only be "To Recycle Bin" or "Report only". With "To Recycle Bin", every copy except the oldest goes to the Recycle Bin. |

### Actions

Each rule has one action.

| Action | What it does |
|---|---|
| Move to | Moves the file to the destination folder. The file name stays. |
| Copy to | Puts a copy in the destination folder. The original stays. |
| To Recycle Bin | Sends the file or empty folder to the Recycle Bin, where it can be recovered. It does not delete permanently. |
| Report only | The file appears in the preview under "Reports". Nothing happens to it. |

### The destination folder and tokens

The destination is a folder, not a file. You can write:

- a full path, for example `D:\Archive`;
- a path without a drive letter, for example `Images`. It is taken relative to the watched folder where the file is;
- `~` at the start (the home folder) or a Windows variable between `%` signs, for example `%USERPROFILE%`;
- a folder token as the first part of the path.

Folder tokens, at the start only: `{downloads}` (Downloads), `{documents}` (Documents), `{pictures}` (Pictures), `{desktop}` (Desktop), `{home}` (home folder). `{videos}` and `{music}` also work if you type them by hand. Valet takes the folders as Windows knows them for your account.

Date and extension tokens, anywhere in the path:

| Token | Becomes |
|---|---|
| `{year}` | The year of the file's modification date, for example `2026`. |
| `{month}` | The month, `01`–`12`. |
| `{day}` | The day, `01`–`31`. |
| `{ext}` | The file extension, in lower case. A file without an extension is skipped. |

Examples: `{documents}/Invoices/{year}` becomes `Documents\Invoices\2026`. `{pictures}/{year}-{month}` becomes `Pictures\2026-05`.

Any other text between braces is refused when you save, with the message "The destination is not valid. Use only the tokens in the list, for example {year}." Valet also refuses a destination inside an excluded folder (see [section 12](#12-what-valet-never-touches)).

### Folders and options

- **Folders:** you choose which watched folders the rule applies to. If you check nothing, it applies to all of them. A rule applies in a folder only if that folder is exactly the watched one, not one above it.
- **Include hidden files:** the rule may also take hidden files. Hidden folders stay skipped anyway.
- **Only files directly in the folder, no subfolders:** the rule does not go down into subfolders.

### Rule order

Valet goes through the rules in the order of the Rules list. A file gets the operation of the first rule it matches that moves, copies or sends to the Recycle Bin. The later rules are ignored for it. "Report only" rules are added anyway.

- A new rule is added at the end of the list, so after the presets.
- An edited rule keeps its place.
- There is no button to move a rule up.
- If a preset already takes the files you want, turn it off with its "On" checkbox, or edit it. The preview shows in the "Rule" column which rule decided.

### Examples

The examples use a folder you add in Settings, so they do not collide with the presets.

1. **Scanned PDFs, by year.** Watched folder: `C:\Users\name\Scans`. Rule "Scanned PDFs": condition "Extension (one)" = `pdf`, action "Move to" `{documents}/Scans/{year}`, folder: only `Scans`.
2. **Old archives to the Recycle Bin.** Conditions: "File type" = Archives and "Modified more than (days) ago" 90. Action: "To Recycle Bin". Folder: `Scans`. Run a preview first and read the list.
3. **Large videos, report only.** Conditions: "File type" = Video and "Larger than" 500 MB. Action: "Report only". The presets do not get in its way, because report rules do not stop other rules.

### After saving

A new or changed rule gets a new version. Any change counts, including the "On" checkbox. Until a preview is approved, automatic organizing does not run its operations (see [section 9](#9-automatic-organizing)). Deleting a rule also deletes its approval.

## 9. Automatic organizing

Valet can organize new files in the watched folders by itself. It is off by default.

### How to turn it on

1. Run a preview and press "Organize files". That is the "approved preview".
2. In Settings, check "Apply rules automatically in watched folders". Until the first approved preview, the checkbox is disabled and says "Available after you approve your first preview."

The "Organize" button appears only when the preview has at least one operation.

### Which rules run by themselves

Only rules approved at their current version. A rule is approved when you press "Organize" on a preview that contained it: the rule had an operation, a report line or a skipped file in that preview. A new rule, a changed rule (including one turned off and on again), or a rule that had nothing to do in the preview waits for a new preview. Its operations show in the next manual preview.

### How it works

- Valet plans the folders like a preview does and runs only the operations of approved rules.
- Each operation goes through the same checks as "Organize". It is written to the journal first, the file is checked again first, and nothing is overwritten.
- You get a short notification. The "Last batch" box says "organized automatically" and has the button "Undo batch". The batch also appears in History.

### When it runs

- On changes in the watched folders: after 2 seconds of quiet, plus up to 2 more seconds to gather nearby changes.
- Every 10 minutes, a full check of all watched folders. It also catches changes Windows did not report.
- When the app starts and after you change the folder list, a few seconds later.
- About 12 seconds after a pass triggered by changes that skipped a file as "still being written", for example a download that just finished.

### When it waits

- While a preview is open: from the moment you press "Run preview" until you press "Organize files" or "Clear result", or leave the Preview screen. If you leave it open, automatic organizing starts again after 15 minutes.
- While it is paused from the tray (see [section 10](#10-tray-and-starting-with-windows)).
- While automatic apply is off in Settings, or no preview has been approved.
- While a manual batch or an undo is running. The steps never overlap.

### Locked files

If a file is locked by another program in an automatic pass, Valet tries it again on the next 3 passes. If it is still locked, it leaves the file alone until a change shows up in it or in its folder. The counter is kept in memory only and starts over at restart. Automatic passes happen on changes and every 10 minutes, so retries can be 10 minutes apart.

### Warning

Approval belongs to the rule, not to the folder. If you later add a new watched folder, the rules already approved apply there too, without a separate preview for that folder. After you add a folder, run a manual preview before you leave automatic organizing on, or turn it off.

## 10. Tray and starting with Windows

**The tray**

- The Valet icon appears next to the clock. Left-click shows the window. Right-click opens the menu.
- The menu has "Show Valet", "Pause automatic organizing" (it becomes "Resume automatic organizing") and "Quit".
- Closing the window does not stop Valet. The window hides, and watching folders and automatic organizing go on. Valet stops only with "Quit".
- The pause lasts until you resume or restart the app. It is not saved.
- If you start Valet a second time, a second copy does not start. The window of the first one is shown.

**Starting with Windows**

- In Settings, the checkbox "Start Valet with Windows" adds an entry in the current user's registry (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`). It needs no administrator rights and does not touch other accounts.
- Valet starts with the argument `--minimized`, so it stays hidden in the tray.
- If you disable the entry in Task Manager, "Startup" tab, Settings shows the checkbox unchecked. If you check it again, the entry is enabled again.

## 11. Settings

![Settings](images/settings.png)

- **Watched folders:** the list of folders. You type the path and press "Add", or press "Remove". Valet refuses a whole drive, a folder with an excluded name in its path (for example `AppData`), and a folder that does not exist.
- **Language:** Română or English. It changes at once. Rules and presets already installed keep their names and folders.
- **Short notification after a batch:** on by default.
- **Start Valet with Windows:** see [section 10](#10-tray-and-starting-with-windows).
- **Apply rules automatically in watched folders:** see [section 9](#9-automatic-organizing).

Each change is saved at once.

## 12. What Valet never touches

Valet works only with files under the watched folders. On top of that, it does not touch the following. The only exception is the "Include hidden files" option of a rule:

- **Excluded folders.** Any folder with one of these names, at any depth under a watched folder, is skipped with everything in it: `Windows`, `Program Files`, `Program Files (x86)`, `ProgramData`, `AppData`, `$Recycle.Bin`, `System Volume Information`, `.git`, `node_modules`, `target`, `.venv`, `venv`, `__pycache__`. Upper and lower case do not matter. Any folder named `target` or `venv` is skipped too.
- **Folders with `.valetignore`.** A folder that contains a file named `.valetignore` (the content does not matter) is skipped with everything in it. Valet does not move files into it either.
- **Hidden files and folders.** Names that start with a dot, or with the Windows hidden or system attribute. A rule can ask for "Include hidden files", but hidden folders stay skipped anyway.
- **Links.** Symlinks and junctions are not followed and not moved.
- **OneDrive and Dropbox "on-demand" files that are not downloaded.** Valet recognizes them from the file attributes, without opening them. It does not read or move them, so it does not download them. Detection was tested with a simulated attribute, not with a real OneDrive or Dropbox.
- **Files being written.** The extensions `.crdownload`, `.part`, `.tmp`, `.download`, `.partial`, names that start with `~$`, and files modified in the last 10 seconds.
- **Files locked by another program.** They fail with a message, untouched.
- **Whole drives.** `C:\`, `D:\`, `\\server\share` are not accepted as watched folders.
- **Excluded destinations.** Valet does not move files into an excluded folder or under a `.valetignore`.

## 13. Drives without a Recycle Bin

A USB drive or a network share usually has no Recycle Bin. Valet sends files to the Recycle Bin only on fixed drives, where Windows has one. On a volume without a Recycle Bin, a Windows "delete" would be permanent, so Valet refuses the operation. The check needs a fixed drive, the `NoRecycleFiles` policy not set, and the `NukeOnDelete` setting not enabled.

- **Sending to the Recycle Bin** from a volume without one: fails with "This volume has no Recycle Bin; Valet does not move or copy files here." The file stays.
- **Copying** into a folder on a volume without a Recycle Bin: fails the same way, before anything is copied.
- **Moving between two drives**, if one has no Recycle Bin: fails the same way.
- **Moving within the same volume** (for example inside a USB drive): works, because it needs no Recycle Bin.
- **Undoing a move between drives** needs a Recycle Bin on both. Otherwise it fails and the file stays where it is.

The "drive without a Recycle Bin" case was tested with a simulated Recycle Bin. It was not tested with a real USB drive or network share.

## 14. Privacy

- Valet does not use the internet for your files. The app code has no network calls. A search of the code, at the version in this repository, finds no network library or call. The only HTTP addresses in the project are `http://localhost:5173` (the local development server, in `tauri.conf.json`) and `http://127.0.0.1` in the automatic UI test.
- Automatic updates, the license check and error reporting are not in the app yet.
- File names, paths and contents do not leave the computer.
- The journal (History) keeps the full paths of files, on your disk.
- No capture of network traffic was made while the app ran. It cannot be said for sure that Tauri or its plugins make no request at runtime.
- In development, the run logs write full paths at DEBUG level. A release build, at INFO level, is meant not to write paths, but no release build was made to check.

## 15. Where the data is

In `%APPDATA%\ro.valetapp.desktop`:

- `valet.sqlite3`: the rules and the operation journal;
- `config.json`: the watched folders;
- `app_settings.json`: language, notifications, starting with Windows, automatic apply, whether you finished the first-run guide and whether you approved a preview;
- `approvals.json`: the approved rules and the version at which each was approved.

Valet never deletes rows from the journal. The journal grows with every batch.

## 16. Known limits

- Test version. Valet has no installer and has not been released.
- The "rename" and "tag" actions from the plan do not exist.
- Undo works on a whole batch, not on a file. Rules cannot be reordered from the interface. A deleted preset cannot be reinstalled from the interface.
- Approval for automatic organizing belongs to the rule, not the folder (see [section 9](#9-automatic-organizing)).
- Windows can lose change events when there are very many. The full check every 10 minutes catches them. Valet cannot ask for a rescan by itself in that case, so a file can wait up to 10 minutes.
- Tests that use real Windows (Recycle Bin, attributes, restore from the Recycle Bin) run in CI on `windows-latest`. For the current version they have no passing result; details in [SAFETY.md](SAFETY.md). Restore from the Windows Recycle Bin is unconfirmed. A power loss, real OneDrive and Dropbox, USB drives and network shares were not tested.
- Other limits and cases, with the tests that cover them, are in [SAFETY.md](SAFETY.md).
