# Raport Faza 2 — Produsul v1

Punct de pornire: `cc00f94` (raportul Fazei 1). Toate commit-urile sunt pe `main`: `git log --oneline cc00f94..HEAD` (aproximativ 50).

Deciziile tale din Faza 1, aplicate:
- copierea pe un volum fără Coș e refuzată, cu mesaj în UI;
- excepția la destinații se păstrează;
- `{ext}` la un fișier fără extensie produce skip, cu motivul afișat;
- rădăcina de disc e refuzată în `save_config` și în UI, iar profilul utilizatorului e permis;
- watcher-ul rescanează la 10 minute.

## Decizie PM: Rules, Settings și Stats rescrise de la zero

Le-am rescris, nu le-am reparat:
- **Rules** folosea un model (`pattern`/`destination`) care nu exista în backend; nu putea funcționa nici reparat.
- **Settings** avea 822 de linii și ~20 de câmpuri, din care codul citea 3, iar Export/Import afișau „succes” fără să facă nimic.
- **Stats** dubla Istoricul și citea o tabelă pe care jurnalul a înlocuit-o.
- Niciunul nu folosea i18n sau clientul tipizat.

Ar fi trebuit schimbate aproape toate liniile, deci rescrierea era mai mică decât reparația.

Ce a rezultat:
- Stats a dispărut din UI, conform PLAN 2.5: un singur ecran principal, cu Previzualizare, Reguli și Istoric, iar Setările secundare. Comenzile backend `get_statistics` și `clear_operation_history` au rămas, nefolosite (decizia 3).
- PreferencesView, care nu era folosit, a fost șters.

## Ce s-a construit

**Reguli (valet-core)**
- Condiții noi:
  - `nameContains`, `extIn`/`extNotIn`;
  - `typeGroup` (image, document, archive, installer, video, audio, code);
  - `modifiedOlderThanDays`, `createdOlderThanDays`;
  - `emptyFolder`;
  - `duplicate` (grupare după mărime, apoi blake3 doar în grupurile de aceeași mărime; placeholder-ele, fișierele ascunse și cele excluse nu se hash-uiesc niciodată).
- Acțiuni noi: `trash` și `report`.
- `scope` per regulă și opțiunea `topLevelOnly`.
- Token-uri de folder: `{home}`, `{desktop}`, `{documents}`, `{downloads}`, `{pictures}`, `{videos}`, `{music}`, plus `{day}`.
- Validare cu coduri `rule.*`; `delete_rule`; versiunea crește la fiecare modificare.
- Regulile utilizatorului se evaluează înaintea presetărilor.

**Executor**
- `trash` trece prin aceleași verificări ca mutările, plus `can_trash`. Un folder e verificat din nou că e gol chiar înainte de trimiterea la Coș.
- Undo prin restaurare din Coș:
  - tip de operație nou `restore`, cu migrația 0004;
  - locul inițial ocupat dă `RestoreOccupied`, iar elementul rămâne în Coș;
  - conținutul diferit se raportează în `restored_modified`.
- Pe Windows, restaurarea găsește elementul după fișierul `$I` din Coș. Crate-ul `trash` folosea numele afișat de Explorer, care ascunde extensia, deci restaurarea `.txt` eșua.

**Cele 8 presetări** (`crates/valet-core/presets/*.json`)
- Fiecare are test de integrare: plan → execuție → undo, cu arbore identic byte cu byte la final. În plus, 6 teste de interacțiune:
  - o factură ajunge în Facturi, nu în Documente;
  - un .exe vechi ajunge în Coș, unul nou în Instalatoare;
  - „Foldere goale” nu trimite la Coș destinațiile altor reguli;
  - o captură de ecran de pe Desktop ajunge în Capturi;
  - „Snippet.png” și „bonus.pdf” nu se potrivesc.
- Ordinea de afișare urmează propunerea researcher-ului. Bifate implicit: Descărcări după tip, Capturi, Facturi, Fișiere mari. Nebifate: Duplicate, Desktop curat, Instalatoare vechi, Foldere goale. Nimic din ce trimite la Coș nu e bifat implicit.
- Ordinea de evaluare pune presetările specifice înaintea celor generice.

**Aplicarea automată** (`apps/desktop/src-tauri/src/auto.rs`)
- Rulează doar dacă sunt îndeplinite toate condițiile:
  - `auto_apply_enabled`;
  - prima previzualizare a fost aprobată;
  - nu e pe pauză din tray;
  - nu e deschisă o previzualizare (timeout de 15 minute).
- Execută doar regulile aprobate la versiunea lor curentă **și pentru folderul respectiv**. Un folder nou sau readăugat cere o previzualizare.
- Se declanșează la evenimentele watcher-ului (debounce de 2 s) și la rescanarea completă de la 10 minute.
- Fișierele blocate se reîncearcă la următoarele 3 treceri.
- Fișierele care erau încă în descărcare se reverifică după 12 s.
- Planificarea și execuția rulează pe `spawn_blocking`.
- Notificarea de final de lot e în limba UI-ului.

**Platformă**
- Autostart doar per utilizator: valoarea „Valet” sub `HKCU\...\Run`, cu calea exe-ului între ghilimele și `--minimized`, respectând starea din Task Manager. Pluginul nu era sigur că rămâne per utilizator, așa că l-am înlocuit.
- Notificări prin `tauri-plugin-notification`.
- Tray: Arată Valet, Pauză/Reia, Ieșire, în ro/en.
- Închiderea ferestrei o ascunde în tray; fereastra pornește ascunsă, fără să clipească la autostart.
- `show_in_explorer` fără shell.

**Frontend**
- Onboarding în 3 pași, cu alegerea limbii la pasul 1.
- Editor de reguli pe schema reală.
- „Adaugă presetări” pe ecranul Reguli.
- Previzualizarea arată fișier, operație, de unde, unde și regula, plus fișierele sărite cu motivul lor și secțiunea Rapoarte (fișiere mari, duplicate cu copia păstrată marcată).
- Istoric cu rezumate traduse.
- Settings rescris: limbă, notificări, autostart, aplicare automată, foldere, foldere invalide din `config.json`.
- Stări goale pe fiecare ecran.
- Toate textele trec prin i18n: 378 de chei ro = en.

**Teste și CI**
- Joburi blocante:
  - `core (Linux)`;
  - `workspace (Windows)`;
  - `frontend (Linux)`: lint, vitest, check:i18n, tsc, build;
  - `e2e smoke (Linux)`.
- Joburi informative:
  - `e2e smoke (Windows)`;
  - benchmark-urile criterion;
  - `scripts/measure-idle.ps1` (pornire, RAM și CPU pe runner-ul Windows).
- Smoke test-ul UI (`tests/e2e/smoke.mjs`) merge prin tauri-driver și un client WebDriver scris pe `fetch`, fără pachete noi. Parcurge onboarding → previzualizare → Organizează → verificare pe disc → Anulează → arbore identic → Istoric, plus Reguli și Setări, și face cele 8 capturi din `docs/images/`.

**Documentație**
- `docs/USER_GUIDE.ro.md` și `.en.md`, cu capturi reale. Capturile vin din smoke test-ul pe Linux, deci căile arată ca `/tmp/valet-e2e-…`.
- SAFETY.md, README și CONTRIBUTING actualizate.

## Ce s-a testat

| Comandă | Exit | Observații |
|---|---|---|
| `cargo build --workspace --all-targets` | 0 | 0 warnings |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | |
| `cargo clippy -p valet-core --all-targets --features test-support -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `cargo test --workspace` | 0 | 429 trecute, 0 picate, 4 ignorate (performanță, rulate separat) |
| `VALET_FAULT_ITERS=100 cargo test -p valet-core --features test-support --test fault_injection` | 0 | 100 de rulări, inclusiv variante cu trimitere la Coș; 0 eșecuri; 42 s |
| `cargo test -p valet-core --release --features test-support --test perf_plan_10k --test perf_index_100k -- --include-ignored` | 0 | vezi măsurătorile |
| `pnpm install --frozen-lockfile`, `pnpm lint`, `pnpm build` | 0 | |
| `pnpm test` | 0 | 15 fișiere, 157 de teste |
| `pnpm check:i18n` | 0 | 378 de chei ro = en |
| `bash scripts/e2e-linux.sh target/e2e/debug/app docs/images` | 0 | smoke test UI end-to-end, ~5 s |
| CI, [run 40773348229](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40773348229) pe `c5acb7d` | verde | toate joburile blocante, inclusiv Windows (presetări, Coș și restaurare, e2e platformă) și e2e Linux |
| CI, [run 40779952551](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40779952551) pe `d512246` (codul final) | verde | toate joburile blocante: core Linux, workspace Windows, frontend, e2e Linux; `e2e smoke (Windows)` informativ, roșu |

## Măsurători vs. bugete (PLAN.md 2.6)

| Metrică | Buget | Măsurat |
|---|---|---|
| Previzualizare pe 10.000 de fișiere (release) | < 3 s | 0,09–0,13 s (inclusiv o regulă `duplicate` peste toate cele 10.000); criterion: 115 ms pe CI |
| Indexare 100.000 de fișiere (release) | < 60 s | ~9 s (11,2 s cu fixture-ul) |
| Pornire până la prima fereastră (Windows CI, `measure-idle.ps1`) | < 2 s | 93 ms până la fereastra vizibilă; primul proces WebView2 la 959 ms |
| CPU la relanti, 60 s (Windows CI) | < 1% | 0,40% medie (max 10% într-o secundă), app + 6 procese WebView2 |
| RAM la relanti (Windows CI) | < 150 MB | **private bytes 101,5 MB** (sub buget); **working set 314 MB** (peste buget) |

RAM-ul depinde de cum îl măsurăm. Working set-ul include memoria partajată a celor 6 procese WebView2 (Edge), numărate fiecare în parte. Private bytes, adică memoria proprie a arborelui de procese, e 101 MB. Bugetul din PLAN nu spune care dintre ele contează (decizia 2). Măsurătorile sunt de pe un runner GitHub, nu de pe un Windows real.

## Probleme cunoscute

1. **Smoke test-ul UI pe Windows pică la crearea sesiunii WebDriver** („DevToolsActivePort file doesn't exist”), deși msedgedriver și WebView2 au aceeași versiune (155.0.4283.62). Jobul e informativ. Fluxul complet e acoperit de smoke test-ul Linux și de testele Windows e2e la nivel de executor; pe UI-ul real de Windows îl verifici tu în checklist.
2. **Comportamentul real pe Windows e verificat doar pe runner-ul CI**: Coș, restaurare, atribute, autostart, notificări. Nu a fost verificat pe un calculator obișnuit, cu OneDrive, USB și Explorer cu extensiile ascunse. Checklist-ul acoperă asta.
3. **Folderele invalide din `config.json` se elimină toate deodată** din Settings, pentru că `save_config` refuză orice listă care încă are o intrare invalidă. Cine vrea să păstreze un folder lipsă temporar (un disc deconectat) nu poate modifica alte foldere până nu îl elimină.
4. **Undo lasă folderele create** (de exemplu `Downloads/Imagini`, gol după anulare). Fișierele revin byte cu byte. E un comportament documentat, nu o pierdere.
5. **Capturi de ecran**: capturile pentru ghid sunt de pe Linux, din fereastra WebKitGTK. Conținutul e identic, dar căile arată `/tmp/...`, nu `C:\Users\...`.
6. **Rezumatul tehnic al loturilor de recuperare e în engleză**, afișat sub „Detalii tehnice”. Loturile de organizare și anulare au rezumate traduse.
7. **Acțiunile `rename` și `tag`** din lista v1 (PLAN 2.3) nu sunt implementate. Tabelul Fazei 2 nu le cerea, iar `upsert_rule` le refuză (decizia 4).
8. **Două pachete de test sunt în afara allowlist-ului**: `@testing-library/dom`, peer obligatoriu pentru `@testing-library/react`, și `jsdom`, mediul DOM pentru vitest (decizia 1).
9. **Bug găsit de CI și reparat** (`d512246`): dacă utilizatorul schimba tabul imediat după apariția previzualizării, `close_preview` putea să nu fie trimis, iar aplicarea automată rămânea pe pauză până la timeout-ul de 15 minute. Planul se transmite acum într-un `useLayoutEffect`, iar testul verifică sincron.
10. **Overflow-ul watcher-ului pe Windows** (notify 6.1) tot nu e detectat. Rescanarea la 10 minute îl acoperă, cu o întârziere de cel mult 10 minute.

## Decizii necesare de la Vlad

1. **`jsdom` și `@testing-library/dom`** (doar devDependencies, pentru `pnpm test`): le aprobi în allowlist? Fără ele, testele de componente React nu rulează.
2. **Bugetul de RAM**: îl măsurăm ca private bytes (101 MB, în buget) sau ca working set (314 MB, din cauza WebView2)? Propunere: private bytes, cu working set raportat separat.
3. **`get_statistics`, `clear_operation_history` și tabela `operation_history`**: UI-ul nu le mai folosește. Le ștergem în Faza 3? Propunere: da.
4. **Acțiunile `rename` și `tag`** din PLAN 2.3: în Faza 3, după v1, sau scoase din v1? Propunere: după v1. `tag` nu are un echivalent nativ pe Windows.
5. **Prețul (din cercetarea R1)**: 29 € e cu ~19% sub Hazel (42 $) și peste aplicațiile mici de 15–25 $. Mouzi (gratuit, MIT) are deja previzualizare și undo, deci Pro trebuie vândut pe limite. Trei întrebări de clarificat până în Faza 4:
   - Presetările cu vechime sunt gratuite, dar „reguli pe vechime” apar la Pro: limita se aplică doar regulilor proprii?
   - Ce se întâmplă cu regulile proprii peste 3 după ce expiră proba?
   - Prețul include TVA?

   Datele R1 sunt din rezumate de căutare, pentru că paginile vendorilor nu au fost accesibile din container.
6. **Poziționarea (R1)**:
   - propunere pentru site: „Valet organizează Descărcările și Desktopul după reguli pe care le vezi, îți arată ce mută înainte să mute și poate anula orice lot.”;
   - sub-linie: „Fiecare mutare se vede în istoric și se poate anula, inclusiv după o cădere.”;
   - „Hazel pentru Windows” rămâne doar pentru SEO, din cauza riscului de marcă.

## Checklist manual pentru Vlad

- [ ] CI pe `main`: https://github.com/OsztrovszkyVlad/Valet/actions. Joburile blocante trebuie să fie verzi; `e2e smoke (Windows)` e informativ.
- [ ] `git pull`, apoi `cd apps/desktop`, `pnpm install`, `pnpm tauri:dev`. Folderul de date `%APPDATA%\ro.valetapp.desktop` e nou: șterge-l înainte, ca onboarding-ul să pornească de la zero.
- [ ] Onboarding:
  - [ ] pasul 1: limbă, foldere (Descărcări și Desktop bifate);
  - [ ] pasul 2: presetări cu badge-urile „trimite la Coș” / „doar raport”;
  - [ ] pasul 3: previzualizare.
  - [ ] Nu se mută nimic până la „Organizează”.
- [ ] Pe Downloads-ul real (sau pe o copie), verifică previzualizarea: fișier, operație, de unde, unde, regula. Uită-te și la „Fișiere sărite” și la Rapoarte.
- [ ] „Organizează” → notificare Windows → „Anulează lotul” → totul revine.
- [ ] Reguli:
  - [ ] creează o regulă (de exemplu PDF cu „contract” în nume → `{documents}/Contracte`);
  - [ ] previzualizează și execută;
  - [ ] modific-o: trebuie să ceară o previzualizare nouă înainte de aplicarea automată.
- [ ] Settings → aplicare automată pornită → descarcă un fișier în Downloads. În ~2 s după terminarea descărcării, fișierul e organizat și apare notificarea; în Istoric, „organizat automat” are butonul Anulează.
- [ ] Tray:
  - [ ] Pauză: un fișier nou nu mai e organizat;
  - [ ] Reia;
  - [ ] Ieșire închide procesul;
  - [ ] închiderea ferestrei cu X doar o ascunde.
- [ ] Autostart pornit:
  - [ ] Task Manager → Startup apps arată „Valet”;
  - [ ] în regedit, `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Valet` are calea exe-ului între ghilimele și `--minimized`;
  - [ ] la relogare, Valet pornește în tray fără fereastră.
- [ ] Coș: activează „Instalatoare vechi” pe un .exe mai vechi de 30 de zile → ajunge în Coș → Anulează → fișierul revine cu numele complet, inclusiv cu extensiile ascunse în Explorer.
- [ ] OneDrive: un fișier „online-only” e sărit cu motivul afișat și nu se descarcă.
- [ ] Stick USB ca folder urmărit sau ca destinație: mutarea și copierea sunt refuzate cu „Volumul nu are Coș de reciclare”.
- [ ] Settings: `C:\` ca folder urmărit e refuzat cu mesajul despre disc întreg.
- [ ] Citește `docs/USER_GUIDE.ro.md`: corespunde aplicației?
- [ ] Răspunde la deciziile 1–6.

## Propunere pentru faza următoare

Faza 3 (Livrare) poate porni conform planului. Propun să adăugăm la începutul ei trei lucruri: ștergerea comenzilor de statistici (decizia 3), mutarea capturilor de ecran pe Windows (din checklist-ul tău sau dintr-un smoke test Windows reparat) și investigarea smoke test-ului pe Windows. Restul planului rămâne neschimbat.
