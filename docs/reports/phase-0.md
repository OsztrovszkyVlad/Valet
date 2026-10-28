# Raport Faza 0 — Adevăr și curățenie

Commit de pornire: `5a5f1e3`. Commit-uri din fază (toate pe `main`):

| Commit | Ce face |
|---|---|
| `5cc4843` | teste de caracterizare pentru `matcher.rs` și `engine::dry_run_for_paths` |
| `a6561d3` | `main.rs` spart în module; dependențele `tokio` și `valet-platform` devin necondiționate; 5 apeluri `std::fs::remove_*` eliminate |
| `21cd82f` | `Cargo.lock` intră în repo |
| `d61edd6` | README și CONTRIBUTING rescrise; DEVELOPMENT_COMPLETE.md și TESTING_GUIDE.md șterse |
| `a7d5be9` | warnings și clippy rezolvate |
| `cb74cac` | `cargo fmt` pe tot workspace-ul (doar formatare) |
| `f04fca8` | CI nou: Linux (core) + Windows (workspace) |
| `a55a820` | pnpm fixat la 10.28.0 prin `packageManager` în `apps/desktop/package.json`; CI citește versiunea de acolo |
| (acest commit) | raportul |

## Ce s-a construit

- **Inventar** (researcher): stare build, warnings, clippy, cele 36 de comenzi Tauri cu utilizarea din frontend, dependențe, view-uri, CI. Datele brute sunt în `.claude/state/research-phase0-raw.txt` (local, ignorat de git). Concluziile sunt în acest raport.
- **Teste de caracterizare** (qa), scrise înainte de orice modificare în valet-core:
  - `crates/valet-core/tests/characterization_matcher.rs` (31 de teste);
  - `crates/valet-core/tests/characterization_dry_run.rs` (21 de teste);
  - `crates/valet-core/tests/common/mod.rs`.
  - Fixează comportamentul actual, inclusiv defectele. Fiecare defect are un comentariu `// characterization: current behavior, ...`.
  - Dev-dependency nouă: `tempfile` (din allowlist).
- **Refactor `main.rs`** (rust-core): 3681 → 56 de linii.
  - Comenzile au trecut în `apps/desktop/src-tauri/src/commands/{rules,operations,backup,settings,platform,diagnostics}.rs`.
  - Tray-ul și pornirea au trecut în `setup.rs`, `AppState` în `state.rs`.
  - `diagnostics.rs` e marcat în antet pentru decizie.
  - Lista comenzilor înregistrate e identică: 36 înainte, 36 după, `diff` gol.
  - Toate blocurile `#[cfg(target_os = "windows")]` sunt păstrate (8/8 și 8/8 `not`).
- **Build pe Linux reparat**: la `5a5f1e3`, crate-ul `app` nu compila pe Linux (5 erori). `tokio` și `valet-platform` erau folosite necondiționat, dar declarate doar pentru Windows. Le-am mutat în `[dependencies]`, fără versiuni noi. `valet-platform` nu compila singur (lipsea feature-ul tokio `sync`); feature-ul a fost adăugat.
- **Invariantul 1, cinci apeluri de ștergere definitivă eliminate.** Sunt schimbări de comportament făcute intenționat:
  1. și 2. Proba `.valet_test` din `execute_file_operation_internal` (move și copy). Crea un fișier în destinație și îl ștergea cu `remove_file`. Un fișier al utilizatorului cu numele `.valet_test` ar fi fost șters definitiv.
  3. Curățarea după recuperare din `recover_single_file`. `decrypt/decompress` sunt stub-uri care întorc calea backup-ului, deci apelul ștergea chiar backup-ul după recuperare.
  4. și 5. `cleanup_backup_file` (`remove_file` + `remove_dir`). Acum refuză cu eroare și nu atinge discul. Ca efect, `cleanup_old_backups` și `optimize_system_performance` raportează 0 backup-uri curățate.
  - Patru teste de regresie în crate-ul `app` acoperă aceste schimbări.
  - `grep -rnE 'remove_file|remove_dir'` în codul non-test: 0 rezultate.
- **Clippy, fmt și warnings**: 3 warnings rustc și 15 findings clippy rezolvate mecanic. `cargo fmt` a trecut codul de la 2 spații la stilul standard (commit separat, doar formatare).
  - Excepție: `let_underscore_future` din handler-ul tray „pause” are `#[allow]` cu comentariu. Notificarea nu se trimite nici înainte, nici acum; nu am schimbat comportamentul.
- **Documentație** (docs-writer):
  - `README.md` rescris: ce este, starea reală, lipsurile, structura repo-ului, build pe Windows, comenzi de dezvoltare, formatul regulilor, ce invarianți nu sunt încă respectați, licență.
  - `CONTRIBUTING.md` rescris (54 de linii); `apps/desktop/README.md` rescris.
  - `DEVELOPMENT_COMPLETE.md` și `TESTING_GUIDE.md` șterse. TESTING_GUIDE descria comenzile de diagnostic, care întorc date hardcodate, ca pe un cadru de testare.
- **CI** (`.github/workflows/ci.yml`):
  - job `core (Linux)`: fmt check, clippy `-D warnings` și teste pentru valet-core și valet-platform;
  - job `workspace (Windows)`: `pnpm install --frozen-lockfile`, `pnpm build`, apoi `cargo build`, `cargo clippy -D warnings` și `cargo test` pe tot workspace-ul.
  - Jobul macOS și `pnpm tauri build` au fost scoase. Bundling-ul aparține Fazei 3.

## Ce s-a testat

Toate comenzile au rulat în containerul Linux pe `f04fca8`, în afară de ultimul rând (GitHub Actions).

| Comandă | Exit | Observații |
|---|---|---|
| `cargo build --workspace --all-targets` | 0 | 0 warnings |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `cargo test --workspace` | 0 | 57 trecute, 0 picate: valet-core 1 unit + 21 + 31 caracterizare, app 4 |
| `cargo check -p valet-platform` / `-p valet-core` (separat) | 0 / 0 | |
| `pnpm install --frozen-lockfile && pnpm build` (apps/desktop) | 0 | 42 de module, JS 226.8 kB |
| `pnpm lint` (apps/desktop) | 1 | 5 erori, 1 warning, toate preexistente (`SettingsView.tsx`, `StatsView.tsx`); lint nu e în CI |
| `wc -l apps/desktop/src-tauri/src/main.rs` | — | 56 (criteriu: < 300) |
| `diff handlers-before.txt handlers-after.txt` | 0 | 36 de comenzi, identice |
| `claude plugin validate .claude/agents` | 0 | „Validation passed”, 7 agenți cu `name` și `description` |
| GitHub Actions CI, run #6 pe `f04fca8` ([link](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40679182527)) | verde | `core (Linux)`: fmt, clippy, 53 de teste OK (1 min 7 s). `workspace (Windows)`: `pnpm install` + `pnpm build`, `cargo build` (3 min 50 s), `cargo clippy -D warnings` (2 min 19 s), `cargo test` OK, inclusiv codul `cfg(windows)`, cu Rust 1.99.1 msvc |

Hook-uri: `cargo --version` a trecut. `git push --force` a fost refuzat de lista `deny` din `.claude/settings.json`. `echo "hook probe: git rebase"` **nu** a fost blocat, deci hook-urile PowerShell nu rulează în acest container (vezi Probleme cunoscute, punctul 1).

## Măsurători vs. bugete (PLAN.md 2.6)

| Metrică | Buget | Măsurat |
|---|---|---|
| CPU la relanti | < 1% | nemăsurat |
| RAM la relanti | < 150 MB | nemăsurat |
| Indexare 100.000 de fișiere | < 60 s | nemăsurat |
| Previzualizare pe 10.000 de fișiere | < 3 s | nemăsurat |
| Pornire | < 2 s | nemăsurat |

Faza 0 nu cere măsurători. Nu există încă benchmark-uri `criterion` și nici fixture de 10k fișiere. Bugetele se cer din Faza 2, testele de volum din Faza 1.

## Probleme cunoscute

1. **Hook-urile nu rulează în sesiunile Claude Code din cloud.** Containerul e Linux și nu are `powershell`/`pwsh`; toate cele 4 hook-uri `.ps1` cad fără efect. Aici a funcționat doar lista `deny` din `settings.json`. Pe Windows ar trebui să meargă; verificarea e în checklist.
2. **Previzualizarea arată calea brută a regulii, iar „Execute” cu regula de exemplu e periculos.** `moveTo` folosește `params.path` ca și cale completă a fișierului destinație. Nu expandează `~` și nu adaugă numele fișierului. Pentru regula de exemplu (`~/Documents/Finance`), din citirea codului (neexecutat): primul PDF potrivit ar fi redenumit într-un fișier numit `Finance`, într-un folder `~` creat relativ la directorul curent al procesului. Următoarele ar eșua cu „Destination file already exists”.
3. **Suprascriere posibilă (invariantul 2).** Verificarea `dest.exists()` și `rename`/`copy` sunt pași separați. Pe Windows, `std::fs::rename` înlocuiește un fișier existent. `recover_files` cu `overwrite_existing: true` suprascrie cu `fs::copy`.
4. **Invarianți neîndepliniți (Faza 1):** nu există jurnal și nici undo; nu există excluderi implicite (testul `char_dry_run_does_not_exclude_hidden_or_default_excluded_dirs`); nu se sar fișierele în scriere, blocate sau placeholder-ele; `index_paths` hash-uiește orice fișier (invariantul 6); nu există Coș de reciclare.
5. **Defecte în matcher și dry run, fixate de testele de caracterizare** (24 marcate). Cele mai importante:
   - un fișier potrivit de două reguli primește două `MoveTo` contradictorii;
   - `ext` cu `.pdf` nu se potrivește niciodată;
   - `pathContains` nu face case-folding pentru diacritice (`Ă` ≠ `ă`);
   - valori numerice greșite (șir, float, negativ) devin 0 sau `u64::MAX` fără eroare;
   - o regulă fără condiții se potrivește cu orice fișier;
   - un root inexistent dă un plan gol fără eroare;
   - rădăcinile suprapuse dublează acțiunile.
6. **Două foldere de configurare.** `valet.sqlite3` stă în `ProjectDirs("com","example","Valet")`, iar `config.json`, `app_settings.json` și `backups.json` în `app_config_dir()` al Tauri (`com.example.valet`). În plus, `sample_rules.json` se citește prin `env!("CARGO_MANIFEST_DIR")`, o cale absolută de la compilare, care nu există pe alt calculator.
7. **Frontend:**
   - formularul din Rules nu corespunde schemei backend (`pattern`/`destination` față de `conditions`/`actions`), deci crearea de reguli din UI eșuează;
   - Export/Import din Settings afișează „succes” fără să facă nimic;
   - `PreferencesView` nu e folosit;
   - textele sunt în engleză, hardcodate, fără i18n;
   - nu există `pnpm test`;
   - `pnpm lint` pică (5 erori);
   - `react-router-dom` e declarat, dar nefolosit.
8. **Backend:**
   - watcher-ul doar loghează evenimente și nu organizează nimic;
   - notificarea din tray „pause” nu se trimite niciodată;
   - fiecare comandă deschide o conexiune nouă la DB și rulează migrațiile;
   - `save_backup_config` eșuează la prima rulare;
   - health check-ul caută `valet.db`, care nu există, deci raportează mereu Critical;
   - `export_rules` primește id-uri `u32`, dar regulile au UUID;
   - șabloanele și `validate_rule` folosesc tipul de condiție `"extension"`, pe care matcher-ul și `upsert_rule` nu îl acceptă.
9. **Codul Windows nu se poate compila din containerul Linux.** `cargo check --target x86_64-pc-windows-msvc` pică în build script-ul `ring` (lipsesc headerele MSVC). Verificarea pentru codul `cfg(windows)` e jobul CI `windows-latest`, verde pe `f04fca8`, inclusiv clippy. Nu înseamnă că aplicația a rulat pe Windows; asta e checklist-ul manual.
10. **CI-ul vechi pica la `pnpm install`** cu `ERR_PNPM_IGNORED_BUILDS` (pnpm nefixat, prin corepack). Acum pnpm e fixat la 10.28.0 prin `packageManager`. Local, `corepack enable` va folosi aceeași versiune.
11. **Comportament schimbat intenționat:** cele 5 eliminări de la invariantul 1 (secțiunea „Ce s-a construit”).
12. **Restanțe locale:** testele app din primul refactor au lăsat 20 de foldere `valet-app-test-*` în `/tmp` al containerului; nu e nimic de făcut. `apps/desktop/src-tauri/src/lib.rs` conține un `run()` din șablon, nefolosit.

## Decizii necesare de la Vlad

1. **Lista de comenzi Tauri: păstrare sau ștergere.** Propunere: 13 păstrate, 23 șterse la începutul Fazei 1 (înainte de journal/executor), ca să nu fie mutate și întreținute degeaba.

   | Comandă | Folosită din UI | Propunere | Motiv |
   |---|---|---|---|
   | get_rules, upsert_rule | da | păstrează | nucleul v1 |
   | dry_run, execute_operations | da | păstrează | se rescriu pe planner/journal în Faza 1 |
   | get_config, save_config | da | păstrează | |
   | get_app_settings, save_app_settings | da | păstrează | majoritatea câmpurilor nu sunt citite de cod; se curăță în Faza 2 |
   | get_statistics | da | păstrează | |
   | clear_operation_history | da | păstrează deocamdată | intră în conflict cu jurnalul și undo; se revizuiește în Faza 1 |
   | configure_auto_start | da | păstrează deocamdată | se înlocuiește cu `tauri-plugin-autostart` în Faza 2 |
   | show_in_explorer | nu | păstrează | e în PLAN Faza 2 |
   | send_notification | nu (intern) | păstrează deocamdată | se înlocuiește cu `tauri-plugin-notification` în Faza 2 |
   | run_system_health_check, run_performance_analysis, run_system_tests, run_security_audit, optimize_system_performance, generate_system_report | nu | șterge | date hardcodate sau false („Passed”, scor 85, `gdpr_compliant: true`); `optimize_system_performance` apela ștergerea de backup-uri |
   | create_backup, get_backups, recover_files, cleanup_old_backups, get_backup_stats, get_backup_config, save_backup_config, verify_backup_integrity | nu | șterge | jurnalul și Coșul din Faza 1 îi iau locul; codul ștergea definitiv și suprascria; compresia și criptarea sunt stub-uri |
   | get_rule_analytics | nu | șterge | date false |
   | execute_operations_bulk | nu | șterge | duplică executorul, fără jurnal |
   | validate_rule | nu | șterge | matcher duplicat, cu tip de condiție greșit și `confidence: 0.85` fals |
   | get_rule_templates, create_rule_from_template | nu | șterge | emit condiții invalide; presetările JSON din Faza 2 le înlocuiesc |
   | export_rules, import_rules | nu | șterge | export-ul nu funcționează (id-uri `u32`); export/import e funcție Pro, se rescrie în Faza 4 |
   | install/uninstall_context_menu_integration | da (Settings) | șterge | scrie în `HKEY_CLASSES_ROOT` (cere administrator, contra instalatorului per utilizator), iar argumentele `--organize-*` nu sunt citite niciodată |

2. **Dependențe preexistente în afara allowlist-ului.** Propunere:
   - Rust: aprobă `tokio`, `chrono`, `log` (cerut de `tauri-plugin-log`) și una dintre `dirs`/`directories`;
   - înlocuiește `notify-rust` cu `tauri-plugin-notification`, `num_cpus` cu `std::thread::available_parallelism`, iar `winapi` și `winreg` cu `windows-sys` în valet-platform, în Fazele 1–2;
   - șterge `thiserror` dacă rămâne nefolosit;
   - Frontend: aprobă ce cere stack-ul (`react-dom`, `@tauri-apps/api`, `@tauri-apps/cli`, `vite`, `typescript`, `eslint` cu pluginurile) și șterge `react-router-dom` (nefolosit).
3. **LICENSE e incomplet**: conține „YOUR NAME” și textul MIT trunchiat. Confirmă numele titularului (de exemplu „Vlad Osztrovszky”) și anul, ca să pun textul MIT complet.
4. **Identificatorul aplicației** e `com.example.valet` în `tauri.conf.json`, iar `config.rs` folosește `com.example`. Ce identificator vrei, de exemplu `ro.valetapp.desktop` după cumpărarea domeniului? Unificarea folderelor de configurare (problema 6) depinde de răspuns.
5. **Hook-uri în sesiunile din cloud.** Fie accepți că acolo funcționează doar lista `deny` și reviewul PM-ului, fie se adaugă `pwsh` în mediu sau variante `.sh` ale hook-urilor. Ambele variante modifică `.claude/` și ți le las ție.
6. **Branch în cloud.** Harness-ul cloud mi-a desemnat branch-ul `claude/wizardly-rubin-pqsi6k`. Am lucrat și am făcut push direct pe `main`, cum ai cerut. Spune-mi dacă vrei altfel în sesiunile viitoare din cloud.

## Checklist manual pentru Vlad

- [ ] CI verde pe `main`: https://github.com/OsztrovszkyVlad/Valet/actions (ambele joburi, `core (Linux)` și `workspace (Windows)`).
- [ ] Clonează curat într-un folder nou și rulează `scripts/windows_dev_setup.ps1` (PowerShell ca administrator) plus pașii manuali pe care îi afișează.
- [ ] `cd apps/desktop`, `pnpm install`, `pnpm tauri:dev`: aplicația pornește și tray-ul apare.
- [ ] Previzualizare înainte și după refactor:
  - [ ] creează un folder de test (de exemplu `C:\ValetTest`) cu `factura_1.pdf`, `invoice-2.pdf`, `poza.jpg`, `notes.txt`;
  - [ ] în Settings → Folders setează doar acest folder;
  - [ ] apasă Preview și notează lista (așteptat, dacă baza de date are doar regula de exemplu: cele două PDF-uri, regula „Invoices → Finance”, câte două rânduri fiecare, moveTo și tag);
  - [ ] rulează `git checkout 5a5f1e3`, apoi `pnpm tauri:dev`, apoi Preview pe același folder. Lista trebuie să fie identică;
  - [ ] revino cu `git checkout main`.
- [ ] **Nu apăsa „Execute”** cu regula de exemplu (problema cunoscută 2).
- [ ] Hook-urile pe Windows: într-o sesiune Claude Code locală, din rădăcina repo-ului, cere-i să ruleze `echo git rebase`. Trebuie să răspundă „Blocked by CLAUDE.md: rebase is forbidden”.
- [ ] Citește README.md: descrie aplicația reală și starea ei.
- [ ] Răspunde la deciziile 1–6 de mai sus.

## Propunere pentru faza următoare

Faza 1 ar trebui să înceapă prin ștergerea comenzilor aprobate la decizia 1 și prin unificarea folderelor de configurare (decizia 4). Abia apoi vin `journal.rs` și `executor.rs`, ca executorul nou să nu moștenească trei căi de execuție (`execute_operations`, `execute_operations_bulk`, `recover_files`). Tot în Faza 1 trebuie decisă semantica `moveTo` (folder destinație + nume de fișier + coliziuni), pentru că testele de caracterizare `char_dry_run_move_to_*` se vor schimba intenționat atunci. Restul planului rămâne neschimbat.
