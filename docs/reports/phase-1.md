# Raport Faza 1 — Nucleul de siguranță

Punct de pornire: `fb5728e` (raportul Fazei 0). Înainte de fază, cu aprobarea ta din 28 octombrie, am aplicat:
- hook-urile rescrise în bash (`6bfa547`), testate live: `echo git rebase`, scriere în `.claude/hooks/` și `remove_file` în cod de produs, toate blocate;
- LICENSE complet, cu titularul Osztrovszky Vlad, 2026 (`c355759`).

Toate commit-urile sunt pe `main`. Lista completă: `git log --oneline c355759..HEAD`.

## Ce s-a construit

**Curățenie (deciziile 1, 2 și 4 din Faza 0)**
- Comenzi Tauri: 36 → 13 aprobate + 3 noi pentru istoric și undo (`list_batches`, `get_batch`, `undo_batch`). `diagnostics.rs` și `backup.rs` au fost șterse (`de37182`, `1dc77db`).
- Dependențe:
  - eliminate `dirs`, `thiserror` (nefolosit), `num_cpus`, `winapi`, `winreg` și, în frontend, `react-router-dom`;
  - adăugate din allowlist `trash`, `windows-sys` (doar în valet-platform), `tauri-plugin-single-instance`, `proptest` (dev), `i18next` și `react-i18next`.
- Un singur folder de configurare, cu identificatorul `ro.valetapp.desktop` (`b9d385c`):
  - DB-ul, `config.json` și `app_settings.json` stau în `%APPDATA%\ro.valetapp.desktop`;
  - un test verifică că `tauri.conf.json` și `APP_IDENTIFIER` din core au aceeași valoare;
  - datele vechi nu se migrează, cum ai cerut;
  - `sample_rules.json` e inclus în binar.

**valet-core (fără cod de OS; verificat prin grep, 0 rezultate)**
- `platform.rs`: trait-ul `Platform`, cu `trash`, `can_trash`, `is_cloud_placeholder`, `is_locked`, `is_hidden_or_system`, `is_reparse_point`, `same_volume`, `rename_no_replace` și `os_path`. `FakePlatform` există pentru teste, în spatele feature-ului `test-support`.
- `planner.rs`, separat de matcher:
  - **excluderi implicite**: Windows, Program Files, Program Files (x86), ProgramData, AppData, $Recycle.Bin, System Volume Information, .git, node_modules, target, .venv, venv, __pycache__, plus orice folder cu `.valetignore`. În aceste foldere nu se intră, iar raportul are o singură intrare per folder;
  - fișierele ascunse (cu punct sau cu atribut) se sar, în afară de cazul `includeHidden`;
  - reparse point-urile se sar, dar doar cele de tip symlink, junction sau mount point; tag-urile OneDrive nu contează;
  - se sar fișierele în scriere (`.crdownload .part .tmp .download .partial`, `~$*`, modificate în ultimele 10 s) și placeholder-ele cloud, detectate după atribute, fără deschiderea fișierului;
  - **semantica `moveTo`/`copyTo`** (decizia 7): folder destinație cu `~`, `%VAR%`, `{year}`, `{month}` (după mtime, ora locală) și `{ext}`. Un folder relativ se raportează la rădăcina scanată. Fișierul își păstrează numele;
  - **o singură operație per fișier**: prima regulă cu move/copy câștigă, următoarele apar ca `Conflict`;
  - **coliziuni**: `nume (2).ext`, `nume (3).ext`, deterministe și fără diferențe de majuscule; se ia în calcul și ce e deja rezervat în același plan;
  - destinațiile din foldere excluse sau aflate sub un `.valetignore` sunt refuzate (`InvalidDestination`);
  - motive de skip, cu nume serde camelCase: Excluded, Hidden, ReparsePoint, BeingWritten, CloudPlaceholder, Conflict, AlreadyInPlace, NoExtension, InvalidDestination, UnsupportedAction, Unreadable.
- `collision.rs`: `next_free_name`.
- `journal.rs`: SQLite în modul WAL, tabelele `batches` și `operations` (migrația 0003):
  - stări planned → in_progress → done | failed, plus done → rolled_back; tranzițiile invalide sunt respinse;
  - o tranzacție per schimbare de stare.
- `executor.rs`:
  - verificare înainte de fiecare operație, în ordinea: placeholder (primul, înainte de orice deschidere), dispărut, modificat de la previzualizare, blocat;
  - hash blake3;
  - **pe același volum**: `rename_no_replace`, care nu înlocuiește niciodată un fișier existent;
  - **între volume**: cere `can_trash` pe sursă și pe destinație, copiază într-un fișier temporar `create_new`, face fsync, verifică hash-ul, mută fără înlocuire, apoi trimite sursa la Coș. Dacă Coșul refuză sursa, copia ajunge la Coș și fișierul rămâne doar la sursă;
  - **copiere**: la fel, fără a trimite sursa la Coș.
- `executor/recover.rs`: rulează la pornire, fără intervenție, și scrie un lot `recovery` care explică fiecare decizie.
- `executor/undo.rs`:
  - operații inverse, în ordine inversă, verificate prin hash;
  - fișierele modificate sau locurile ocupate se raportează ca `needs_confirmation`;
  - cu `force`, restaurarea se face lângă original, ca `nume (2).ext`; nu se suprascrie nimic.
- `index.rs`: placeholder-ele nu se hash-uiesc; scrierea în DB se face în tranzacții de câte 1.000.
- Matcher-ul compilează regex-urile o singură dată (cache), cu același comportament: cele 31 de teste de caracterizare trec nemodificate.

**valet-platform (Windows)**
- `trash_path`, care refuză dacă `can_trash` e fals. `can_trash` cere un disc fix, fără politica `NoRecycleFiles` și fără `NukeOnDelete` pe volum. Altfel, pe USB sau rețea, shell-ul ar șterge definitiv.
- `is_cloud_placeholder`, `is_hidden_or_system` și `is_reparse_point` citesc atributele cu `GetFileAttributesW` / `FindFirstFileW` și nu deschid fișierul.
- `is_locked`, `same_volume`, `to_long_path` (`\\?\`) și `rename_no_replace` (`MoveFileExW` fără flag de înlocuire).
- Watcher rescris: debounce de 2 s, oprire reală la `cancel`, rescanarea întregii rădăcini la overflow (pe Linux).
- `SystemPlatform` implementează `Platform` și e conectat în app.

**App (src-tauri)**
- `dry_run` → planner, cu `plan_id` și lista `skipped`.
- `execute_operations(plan_id)` rulează **exact planul previzualizat**, o singură dată; altfel întoarce `preview_required` (`51467bc`).
- Meniul din tray „execute-now” a fost scos: nu mai există execuție fără previzualizare.
- Recuperarea rulează la pornire, înainte de watcher.
- Single-instance, înregistrat primul: a doua instanță iese înainte de `setup`, deci recuperarea nu poate rula în paralel cu un lot (`fcc44ea`).
- Log Info în release; nicio cale sau conținut de configurare la INFO (`b2b1969`).

**Frontend**
- i18n (ro implicit, en) pentru textele noi; `pnpm check:i18n` verifică cheile (85 în fiecare limbă).
- Clientul tipizat e în `src/api/commands.ts`.
- În Previzualizare: lista fișierelor sărite, cu motivul tradus, și butonul permanent „Anulează lotul” după execuție.
- Tabul nou **Istoric**: loturile cu stările lor, detaliul pe fiecare operație, rezumatul recuperării după oprire și fluxul de anulare (previzualizare, confirmare, „Anulează oricum”, rezultat).

**Teste și CI**
- Unitare în planner, collision, journal, executor, recover, undo și index.
- 52 de teste de caracterizare păstrate. 11 au fost schimbate intenționat și sunt marcate `// Phase 1: changed intentionally`.
- Proptest: `prop_planner.rs` (o operație per fișier, destinații unice fără diferențe de majuscule, nicio suprascriere, determinism) și `prop_collision.rs` (injectivitate, stabilitate).
- `fault_injection.rs`: procesul copil e omorât la 10–500 ms, urmează recuperarea și verificarea „exact o dată” cu hash, apoi undo byte cu byte. 100 de rulări în CI.
- `integration_10k.rs`: 10.000 de fișiere, plan → execuție → undo, cu arbore identic la final.
- Windows: `crates/valet-platform/tests/windows.rs` (atribute, blocare, căi lungi, junction, Coș) și `windows_e2e.rs` (executor real cu `SystemPlatform`: mutare, placeholder sărit și nehash-uit, fișier blocat, cale de 300 de caractere, undo).
- CI:
  - Linux: fmt, clippy (inclusiv `test-support`), teste, injecție ×100, performanță în release;
  - Windows: build, clippy, teste workspace, injecție ×100, clippy și teste platformă cu e2e;
  - eșecurile de test apar ca annotări (`scripts/ci-test.sh`).
- Toolchain fixat la Rust 1.99.1 (`rust-toolchain.toml`), ca local și CI să fie identice.

**Documentație**
- `docs/SAFETY.md`: invarianții, cum îi respectă codul, cu testele care o dovedesc, cazurile-limită, jurnalul și ce nu e încă acoperit.
- README actualizat.

## Ce s-a testat

| Comandă | Exit | Observații |
|---|---|---|
| `cargo build --workspace --all-targets` | 0 | 0 warnings (Rust 1.99.1) |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | |
| `cargo clippy -p valet-core --all-targets --features test-support -- -D warnings` | 0 | |
| `cargo clippy -p valet-platform --all-targets --target x86_64-pc-windows-msvc -- -D warnings` | 0 | codul `cfg(windows)` din platform, compilat din Linux |
| `cargo fmt --all -- --check` | 0 | |
| `cargo test --workspace` | 0 | 223 trecute, 0 picate, 3 ignorate (testele de performanță, rulate separat în release) |
| `VALET_FAULT_ITERS=100 cargo test -p valet-core --features test-support` | 0 | injecție: 100 de rulări, 0 eșecuri (75–76 procese omorâte în timpul lotului) |
| `cargo test -p valet-core --release --features test-support --test perf_plan_10k --test integration_10k --test perf_index_100k -- --include-ignored` | 0 | vezi măsurătorile |
| `pnpm install --frozen-lockfile && pnpm build` | 0 | |
| `npx tsc -p tsconfig.app.json --noEmit` | 0 | |
| `pnpm check:i18n` | 0 | 85 de chei ro = en |
| `pnpm lint` | 1 | aceleași 5 erori preexistente (SettingsView, StatsView), niciuna în codul nou |
| `grep -rnE 'remove_file\|remove_dir' crates/*/src apps/desktop/src-tauri/src` | — | 0 rezultate |
| CI pe GitHub, run [40737483308](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40737483308) pe `fe6200e` | verde | `core (Linux)` și `workspace (Windows)`: toate testele, inclusiv injecția ×100 pe ambele OS, `windows.rs` și `windows_e2e.rs` |

## Măsurători vs. bugete (PLAN.md 2.6)

| Metrică | Buget | Măsurat |
|---|---|---|
| Previzualizare pe 10.000 de fișiere (release) | < 3 s | 0,12–0,16 s (înainte de cache-ul de regex: 2,2 s cu o regulă `nameMatches`, 5,9 s cu trei) |
| Indexare 100.000 de fișiere (release) | < 60 s | 9,9–10,3 s (înainte de grupare: 70,8–74,9 s) |
| Execuție pe 10.000 de fișiere, 6.711 operații (release, FakePlatform) | — | 14,8 s (454 fișiere/s); undo 18,3 s |
| CPU / RAM la relanti | < 1% / < 150 MB | nemăsurat (necesită Windows real; Faza 2) |
| Pornire | < 2 s | nemăsurat (Faza 2) |

Măsurătorile sunt făcute pe VM-ul Linux din cloud, împărțit cu build-urile altor agenți.

## Probleme cunoscute

1. **Overflow-ul watcher-ului pe Windows nu e detectat.** `notify` 6.1.1 ignoră overflow-ul `ReadDirectoryChangesW`; abia notify 9 (rc) îl raportează. Watcher-ul doar loghează evenimentele; aplicarea automată vine în Faza 2, iar atunci e nevoie de o rescanare periodică.
2. **Două cazuri de dublă eroare pot lăsa un fișier în două locuri, niciodată în zero:**
   - Coșul refuză sursa, apoi refuză și copia;
   - recuperarea găsește un placeholder la destinație în timp ce sursa există.
   Ambele sunt descrise în jurnal și în SAFETY.md.
3. **Copierea cere Coș de reciclare pe volumul destinație** (pentru undo și pentru curățarea fișierelor temporare). Copierile pe USB sau în rețea sunt refuzate cu `NoRecycleBin`.
4. **Rădăcini care diferă doar prin majuscule** (`C:\Docs` și `c:\docs`) nu sunt deduplicate, deci fișierele ar fi planificate de două ori. A doua operație eșuează pentru că sursa a dispărut; nu se pierd date.
5. **Excluderile se aplică doar sub o rădăcină aleasă.** Se poate alege `C:\` ca rădăcină, iar Windows, Program Files, AppData etc. tot sunt sărite de sub ea.
6. **Excepția la destinații:** componentele de cale comune cu rădăcina aleasă nu blochează destinația. De exemplu, o rădăcină aleasă în AppData poate sorta în interiorul ei.
7. **Neimplementate încă:**
   - verificarea „mărimea se schimbă între două citiri”: executorul compară în schimb mărimea și mtime față de previzualizare;
   - reîncercarea fișierelor blocate la următoarele 3 treceri ale watcher-ului (Faza 2, odată cu aplicarea automată);
   - hook-ul de panică (PLAN 4.5, Faza 3);
   - rotația logurilor 5 × 2 MB (Faza 3).
8. **Execuția rulează pe un worker tokio, cu I/O sincron.** UI-ul rămâne utilizabil; în Faza 2 trebuie mutată pe `spawn_blocking`.
9. **Rezumatele loturilor sunt text în engleză**, generat de backend; frontend-ul le afișează ca atare. Traducerea cere expunerea deciziilor ca date (Faza 2).
10. **Frontend:** view-urile vechi (Rules, Settings, Stats) sunt încă în engleză hardcodată. Formularul Rules nu corespunde schemei backend, deci regulile nu se pot crea din UI (Faza 2). Nu există `pnpm test`.
11. **CI-ul a fost roșu între `1f51239` și `d3dd139`**: GitHub folosea Rust 1.99.1, iar containerul 1.97, căruia îi lipsea un lint clippy nou. Am fixat toolchain-ul la 1.99.1. Apoi au picat 3 teste doar pe Windows, cu așteptări valabile numai pe Linux (NTFS nu ține cont de majuscule, separatorul e `\`); codul de produs era corect. Le-am reparat în `fe6200e`.
12. **Logurile GitHub Actions nu se pot descărca din containerul cloud** (politica de rețea blochează stocarea Azure). Diagnosticul se face prin annotări.

## Decizii necesare de la Vlad

1. **Copierea fără Coș pe destinație**: o refuzăm, cum e acum, sau o permitem fără undo pentru copii? Propunere: o refuzăm în v1.
2. **Excepția la destinații** (problema 6): o păstrăm? Propunere: da, e aceeași regulă ca pentru rădăcinile alese.
3. **`{ext}` pentru fișierele fără extensie**: acum se sar cu motivul `NoExtension`. Alternativa ar fi un folder fix, de exemplu „Altele”. Propunere: rămâne skip.
4. **Rădăcina `C:\` sau o rădăcină de disc**: o permitem (excluderile se aplică sub ea) sau o refuzăm în `save_config`? Propunere: o refuzăm în Faza 2, cu mesaj în UI.
5. **Watcher pe Windows**: rescanare periodică (de exemplu la 10 minute) când pornește aplicarea automată, sau trecere la notify 9 când iese stabil? Propunere: rescanare periodică în Faza 2.

## Checklist manual pentru Vlad

- [ ] CI verde pe `main`: https://github.com/OsztrovszkyVlad/Valet/actions (`core (Linux)` și `workspace (Windows)`).
- [ ] `git pull`, apoi `cd apps/desktop`, `pnpm install`, `pnpm tauri:dev`. Rust 1.99.1 se instalează automat prin `rust-toolchain.toml`.
- [ ] Folderul de configurare e `%APPDATA%\ro.valetapp.desktop`. Regulile și setările de dinainte nu se mai văd; e normal, fără migrare.
- [ ] În Settings → Folders setează un folder de test cu 500 de fișiere. Fă o copie a lui ca snapshot (sau `dir /s /b > before.txt` plus hash-urile).
- [ ] Previzualizare: verifică tabelul și deschide „Fișiere sărite”, cu motivele traduse.
- [ ] Apasă Execute și, în timpul lotului, închide Valet din Task Manager.
- [ ] Redeschide Valet:
  - nimic pierdut, nimic dublat (compară cu snapshot-ul);
  - în Istoric, primul rând e „Recuperare după oprire”, cu explicația;
  - lotul întrerupt are stările pe fiecare fișier.
- [ ] Anulează lotul (Previzualizare → „Anulează lotul” sau Istoric → lot → „Anulează lotul” → „Confirmă anularea”). Folderul trebuie să fie identic cu snapshot-ul.
- [ ] Pornește Valet a doua oară cât rulează deja: nu apare o a doua fereastră, iar cea existentă vine în față.
- [ ] Opțional (raportul T3): un fișier OneDrive „online-only” e sărit și nu se descarcă; un `.docx` deschis în Word e raportat „blocat”; pe un stick USB, copierea și mutarea sunt refuzate cu „fără Coș de reciclare”.
- [ ] Răspunde la deciziile 1–5.

## Propunere pentru faza următoare

Faza 2 ar trebui să înceapă cu formularul de reguli pe schema reală (`conditions`/`actions`) și cu presetările JSON. Fără ele, aplicația nu se poate folosi de la zero. Tot în Faza 2, odată cu aplicarea automată din watcher, trebuie făcute rescanarea periodică (problema 1), reîncercarea fișierelor blocate și mutarea execuției pe `spawn_blocking`. Restul planului rămâne neschimbat.
