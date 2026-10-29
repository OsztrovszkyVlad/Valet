# Microsoft Store listing (English)

Versiunea în română: [listing.ro.md](listing.ro.md)

**DRAFT — Vlad decides.** The Store listing is not live. Nothing here has been entered in Partner Center. Limits quoted below (characters, number of items) are the ones given in the task for this phase; check them against Partner Center when you enter the text, because they were not checked there. Facts come from `README.md`, `docs/LICENSING.md`, `docs/PRIVACY.md`, `docs/SAFETY.md` and `docs/PLAN.md` sections 3.1 and 3.2.

Open points that only Vlad can settle are marked `[VLAD: ...]`. They are collected at the end of this file.

---

## Product name

Valet

[VLAD: reserve the name in Partner Center. `docs/PLAN.md` section 8, question 1, asks for a trademark check (EUIPO) before the Store, because other products use the name.]

## Short description

Valet organizes your Downloads and Desktop by rules you write. It shows a preview before anything moves, sends removed files only to the Recycle Bin, and can undo a batch.

(about 170 characters)

## Description

Valet keeps folders such as Downloads and Desktop in order, by rules you can read and undo.

You choose the folders Valet watches. A rule has conditions (file extension, file type, part of the name, size, age, empty folder, duplicate) and an action: move, copy, send to the Recycle Bin, or only report. Eight ready-made rules cover common cases, such as Downloads by type, Screenshots and Invoices and receipts.

Before anything moves, Valet shows a preview: each file, where it is, where it would go, and which rule decided. Nothing runs until you approve.

Valet never deletes a file permanently: anything it removes goes to the Windows Recycle Bin. It never overwrites a file: a taken name becomes "report (2).pdf". It leaves Windows, Program Files, AppData, .git, node_modules and hidden files alone, and skips files that are still downloading, locked files, and OneDrive or Dropbox files that are not downloaded.

A finished batch can be undone while the moved files are unchanged; if you changed one, Valet asks first. Each operation is written to a journal before it runs, and after a crash Valet checks the unfinished ones at the next start.

No account, no ads, no telemetry. Updates come through the Store; the Store version does not check GitHub. Only with a Pro license, Valet checks the license (license id, a one-way hash of the machine id, the app version). No file names, paths or contents are sent. Starting with Windows is set in the Windows settings (Valet has a button that opens them). Uninstalling the app from the Store deletes its data, including History and undo.

Free: all eight ready-made rules, 3 own rules, 2 watched folders, 7 days of history, undo of the latest batch. Pro: no limits, undo of any batch, export and import of rules; 29 EUR with VAT included, once, with 12 months of updates, bought outside the Store. A 14-day Pro trial needs no card.

[VLAD: the sentence with the price above ("Pro: ... 29 EUR ...") is true only when buying works. Decision of 2026-10-29: selling opens only after the merchant's webhook has passed a sandbox purchase and a sandbox refund end to end (`docs/LICENSING.md`, "License service"); until then the site says buying is not open. If you submit the listing before selling opens, remove the sentence with the price and put this in its place: "Pro: no limits, undo of any batch, export and import of rules. Buying Pro is not open yet. The 14-day Pro trial needs no card." If selling is open at submission, keep the sentence, and add the 14-day refund if you want to mention it.]

Romanian and English. Windows 10 22H2 and Windows 11, 64-bit. Valet is in beta: do not point it at folders whose contents you cannot lose, and use the preview.

(about 1,800 characters; the limit is 10,000.)

## Feature bullets (20 at most, 8 used)

1. Rules with conditions on extension, file type, name, size, age, empty folders and duplicates.
2. Actions: move, copy, send to the Recycle Bin, or only report.
3. Eight ready-made rules, free and not counted against the limit.
4. A preview of every move before it happens; automatic mode only after you approve one.
5. Undo of a finished batch while the moved files are unchanged.
6. Recycle Bin only: no permanent deletion, no overwriting of existing files.
7. Skips files being downloaded, locked files and OneDrive or Dropbox files not downloaded.
8. Romanian and English interface; no account, no ads, no telemetry.

## Search terms (7 at most)

1. file organizer
2. hazel for windows
3. organize downloads folder
4. auto file sorter
5. downloads cleaner
6. folder rules
7. file mover

[VLAD: "hazel for windows" is a comparison term, as `docs/PLAN.md` section 3.4 plans for SEO. Store policies may reject another product's name as a search term; if the submission is rejected for it, replace it with "desktop cleaner". Check the current policy text; this was not checked.]

## What's new in 1.0

First version in the Microsoft Store.

- Rules with preview and undo for Downloads, Desktop and other folders you choose.
- Eight ready-made rules.
- Free plan and Pro plan (Pro is bought outside the Store).
- 14-day Pro trial, no card.
- Romanian and English interface.

[VLAD: compare with `CHANGELOG.md` for the version you submit. Do not list anything that is not in that build.]

## Privacy policy URL

https://valetapp.ro/en/privacy.html

The Store needs one because the app uses the internet. The page is in the repository (`site/en/privacy.html`); the address answers only after the site is deployed on `valetapp.ro`. [VLAD: put the site online on the domain before submitting, and open the address from Partner Center to check it.]

## Support URL

https://github.com/OsztrovszkyVlad/Valet/issues

For now support is GitHub Issues only (decision of 2026-10-29); there is no support email address. For licenses, the app shows a "License code (for support)" in Settings, Valet Pro.

## Screenshots

The screenshots come from Vlad's `docs/screenshots/`, taken on Windows with test folders (no real file names). The folder does not exist yet. The images in `docs/images/` show Linux test paths and are not for the Store. Check the required sizes in Partner Center (I recall 1366x768 or larger, not verified).

| # | File (to be made) | Caption |
|---|---|---|
| 1 | `docs/screenshots/01-preview.png` | Preview: every move is listed with the rule that decided it. Nothing runs until you approve. |
| 2 | `docs/screenshots/02-presets.png` | Eight ready-made rules, chosen on first run. |
| 3 | `docs/screenshots/03-after-organize.png` | After a batch: the result, and the Undo batch button. |
| 4 | `docs/screenshots/04-history.png` | History: each batch, what moved and where. |
| 5 | `docs/screenshots/05-rules.png` | Rules: conditions and actions you can read. |
| 6 | `docs/screenshots/06-settings.png` | Settings: watched folders, language, start with Windows, updates. |

## Age rating questionnaire (IARC, answers inferred)

I cannot run the questionnaire. These are the answers the app's behavior suggests; Vlad answers it in Partner Center and reads each question as it is worded there. The category would be a utility or productivity app.

| Topic | Answer | Basis |
|---|---|---|
| Violence, blood, fear | No | No such content. |
| Sexual content or nudity | No | No such content. |
| Profanity or crude humor | No | UI text is fixed in `ro.json` and `en.json`. |
| Controlled substances (drugs, alcohol, tobacco) | No | No such content. |
| Gambling or simulated gambling | No | None. |
| User-generated content, chat, user interaction | No | No accounts and no sharing between users. The app shows only the user's own file names. |
| Shares the user's location | No | Not used. |
| Shares personal information with others | [VLAD: check] | Only with a Pro license: license id, hashed machine id, app version, to Valet's own license server. No file names or paths. See `docs/PRIVACY.md`. Answer by the questionnaire's wording. |
| Unrestricted internet access or a web browser | No | The interface loads only the app's own files; it has no browser. The license check (and, outside the Store, the update check) goes to fixed addresses. |
| Ads | No | None. |
| In-app purchases through the Store | No | Pro is bought outside the Store (`docs/PLAN.md` section 3.2). |
| Digital purchases of any kind in the app | [VLAD: check] | If the Pro screen has a "Buy" button that opens the browser, the questionnaire or the Store policy may count it. See below. |

Expected result, not verified: the lowest age rating (suitable for all ages).

## Store policy question for Vlad: external purchases

`docs/PLAN.md` section 3.2 says that for apps (not games) that use their own payment system, the Store commission is 0%, and that Pro is bought through Paddle (or Polar) from the app. That was written from memory in the plan; it was not checked here. Before submitting:

1. Read the current Microsoft Store policies on payments and on apps that unlock features with a purchase made outside the Store. Check whether a "Buy" button inside the app is allowed, whether a link must open the browser, and whether the listing may name the price or the external site.
2. Check whether an app that is free in the Store but has limits that an external license lifts needs a specific disclosure in the description. The description above says "Pro is bought outside the Store" for that reason.
3. If the policy does not allow the "Buy" button in the Store build, the Store build needs a screen that only accepts a license file, and the purchase link stays on the site. That is a change to the app; ask the frontend work to make it only after you decide.

## Notes for the certification testers (draft)

- The Free plan works without a license or an account.
- Pro features need a license file bought outside the Store. [VLAD: put a test license here for the testers, issued by hand, and say how to import it (Settings).]
- The app reads and moves files in the folders the user picks, and sends removed files to the Recycle Bin. A packaged desktop app that does this normally needs the full-trust capability (`runFullTrust`); Partner Center may ask for a reason. [VLAD: not checked. The manifest in `packaging/msix/AppxManifest.xml` asks for `runFullTrust`, and the workflow `.github/workflows/msix.yml` builds the package, but the package was not submitted in Partner Center.]
- Network use: in the Store version, only the license check, and only with a Pro license imported in a build with a license server (the current ones have none). The version check does not run in the Store version. No other connections. `docs/PRIVACY.md`.
- Starting with Windows: in the Store version it is not a checkbox in the app; Settings has a button that opens the Windows settings for startup apps (`packaging/msix/README.md`, section 5).

## Open questions for Vlad

1. Name: trademark check and reservation of "Valet" before submission.
2. Privacy policy URL: `https://valetapp.ro/en/privacy.html` is decided; the site must be online on the domain before submission.
3. The Store policy on external purchases (section above).
4. Whether "hazel for windows" stays as a search term.
5. The age rating answers marked `[VLAD: check]`.
6. A test license for the certification testers.
7. Screenshots from Windows in `docs/screenshots/`.
8. The MSIX package is built in CI (`packaging/msix/README.md`) but was not submitted in Partner Center and was not tried in a Store install; the package identity comes from Partner Center.
9. Buying Pro: it opens after the merchant webhook passes its sandbox test (payment and refund). At submission, pick the variant of the "Description" text that matches the state at that time.
