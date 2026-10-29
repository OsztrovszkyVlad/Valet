# Raport Faza 3 — Livrare

Punct de pornire: `d36cc51` (raportul Fazei 2). Commit-urile sunt pe `main`: `git log --oneline d36cc51..HEAD`.

## Deciziile tale din Faza 2, aplicate

- `jsdom` și `@testing-library/dom` sunt adăugate în allowlist-ul din CLAUDE.md, doar ca devDependencies (`bd1b843`).
- PLAN.md e actualizat (`bd1b843`):
  - 2.3: `tag` iese din plan; `rename` cu șabloane e prima funcție după lansare;
  - 3.1: limitele Gratuit/Pro, regulile peste 3 rămân salvate și dezactivate după probă, 29 € cu TVA inclus;
  - 2.6: bugetul de RAM se măsoară pe private bytes, iar working set-ul se raportează separat.
- `get_statistics`, `clear_operation_history` și tabela `operation_history` sunt șterse, cu migrația 0005 (`54b533f`, `55da75c`). Testul de upgrade arată că jurnalul rămâne intact.
- Bugetul de RAM se compară acum cu private bytes în `scripts/measure-idle.ps1`.

## Cerințele tale suplimentare

- **Folder lipsă la salvare** (`27d8fb7`, `2b44657`): un folder care nu există (de exemplu un disc deconectat) se salvează și apare ca „Indisponibil acum”. Watcher-ul și planificarea îl sar, iar când reapare e preluat la rescanarea de 10 minute, fără repornire. Aprobările lui se păstrează. Rădăcina de disc și folderele excluse rămân refuzate.
- **Smoke test-ul UI pe Windows** a fost gata în 34 de minute din cele 2 ore (`e7ab643`) și acum e blocant (`d2e1974`).
  - Cauza: wry setează propriile argumente pentru WebView2 (`AdditionalBrowserArguments`), iar acestea anulează portul de depanare pe care îl trimite msedgedriver.
  - Reparația: doar build-ul de test adaugă `--remote-debugging-port=0`, prin variabila `TAURI_CONFIG`. Release-ul nu e afectat.
- **Capturile de ecran de pe Windows din `docs/screenshots/`: blocat.** Folderul nu există pe niciun branch de pe remote (verificat cu `git ls-remote` pe 29 octombrie). Ghidurile folosesc în continuare capturile de pe Linux. Când împingi folderul, înlocuirea e o sarcină mică (decizia 1).

## Ce s-a construit

**Instalator și release**
- `tauri.conf.json` (`69bbaf5`):
  - instalator NSIS doar per utilizator, fără drepturi de administrator, în română și engleză, cu selector de limbă;
  - WebView2 prin bootstrapper;
  - fereastră de 1100×750 (minim 900×600);
  - Content Security Policy strictă: doar resurse locale și IPC, fără `unsafe-inline`;
  - exe-ul se numește `Valet.exe` (`mainBinaryName`).
  - Textele românești pe care Tauri nu le are sunt în `packaging/nsis/Romanian.nsh`.
- **Iconiță provizorie:** un „V” alb pe fond albastru-gri, în locul logo-ului Tauri. Iconița finală trebuie desenată (decizia 3).
- **Versiunea dintr-o singură sursă:** `scripts/set-version.mjs` și `scripts/check-version.mjs` țin la fel `tauri.conf.json`, `Cargo.toml`, `Cargo.lock` și `package.json`.
- **`.github/workflows/release.yml`**, activat cu aprobarea ta:
  - un tag `v*` construiește instalatorul, creează un release **draft** (prerelease dacă versiunea conține „-”), cu note din CHANGELOG și avertismentul SmartScreen;
  - artefactele updater-ului se semnează doar dacă există cheile tale;
  - la publicare, un job separat actualizează `latest-beta.json` pentru canalul beta;
  - în modul `workflow_dispatch` cu `dry_run` doar construiește instalatorul;
  - același mod, cu `beta_tag`, repune canalul beta pe o versiune anterioară (rollback).
- **Canale de actualizare:**
  - stable: `releases/latest/download/latest.json`;
  - beta: `releases/download/updater-channel/latest-beta.json`.
- **winget:** șabloane pentru `OsztrovszkyVlad.Valet` și `OsztrovszkyVlad.Valet.Beta` (schema 1.10, scope user), plus `scripts/winget-manifest.mjs`. Șabloanele sunt validate cu schema JSON a winget; nu au fost trimise.
- `docs/RELEASE.md` descrie pas cu pas un release, ce setezi tu o singură dată și cum faci rollback.
- `CHANGELOG.md` e completat.

**Updater în aplicație** (`1d910c9`, `2b44657`, `6d39637`)
- `tauri-plugin-updater` 2.14.1. Pluginul se inițializează doar dacă build-ul are configurație de updater, care e injectată de release la build din variabila ta `VALET_UPDATER_PUBKEY`. `tauri.conf.json` nu are secțiune de updater, iar un test verifică asta.
- Se cere doar endpoint-ul canalului ales. Canalul implicit e beta pentru versiunile pre-release și stable pentru celelalte.
- Verificarea automată rulează la 30 s după pornire și apoi la 24 de ore. Doar te anunță: notificare, o bulină pe butonul Setări și o notă pe ecranul principal.
- Instalarea pornește doar când apeși butonul, după o confirmare. E refuzată cât rulează un lot și reîncercată automat după el; pe durata instalării nu poate porni niciun lot.
- Semnătura minisign e verificată de plugin.
- Settings are acum secțiunile Actualizări (versiune, canal, verificare, instalare cu progres), Asistență și Despre.

**Loguri și erori** (`1d910c9`)
- Logurile sunt în `%LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log`, cu rotație de 5 fișiere a câte 2 MB.
- Nivelul e INFO, fără căi. DEBUG se pornește din Setări și se aplică fără repornire.
- Crate-urile zgomotoase (sqlx, hyper) sunt limitate la INFO.
- Hook-ul de panică scrie în log, cu căile înlocuite prin `<path>`. Operațiile rămase `in_progress` sunt închise de recuperare la următoarea pornire.

**Calitate și CI**
- Test de upgrade (`c2efbdc`): baze de date construite exact ca după Fazele 1 și 2 (migrațiile istorice, verificate byte cu byte), deschise cu codul curent. Verifică:
  - toate rândurile din jurnal rămân identice;
  - `operation_history` dispare;
  - un lot din Faza 1 se poate anula și astăzi, cu arborele restaurat identic.
- **Job nou, blocant, `installer (Windows)`** (`76663a0`, `d2e1974`):
  - construiește instalatorul ca release-ul și îl instalează silențios;
  - verifică instalarea în `%LOCALAPPDATA%\Valet`, cheia de dezinstalare doar în HKCU, scurtătura din Start;
  - pornește `Valet.exe --minimized` și verifică baza de date și logul;
  - dezinstalează și verifică că exe-ul, scurtăturile, cheia și intrarea `Run` dispar, iar datele utilizatorului rămân;
  - reinstalează și verifică că e folosită aceeași bază de date.
  - Pe un runner nou a lipsit cheia `HKCU\...\Run`, iar scriptul a picat la scrierea ei. Acum scriptul creează cheia, cum face și aplicația (`21cd92b`). Nu a fost un bug al aplicației.
- Măsurătorile de relanti rulează acum pe `Valet.exe` instalat.
- **Bug găsit de CI** (watcher, Windows): un lot întârziat putea ajunge la callback după `cancel()` (`1ac103c`).
  - Cauza: notify-debouncer-mini 0.4.1 nu își așteaptă propriul thread la oprire, iar verificarea unui flag atomic lăsa o fereastră între verificare și apel.
  - Reparația: un lacăt ținut pe toată durata callback-ului. `cancel()` îl închide și așteaptă callback-ul care rulează, iar loturile rămase în coadă sunt aruncate.
  - Teste noi: `pending_batch_after_cancel_is_dropped`, care e determinist, și `close_waits_for_running_callback`, care a picat la o reintroducere temporară a bug-ului.
  - Testele watcher-ului au rulat de 20 de ori local fără eșec.

**Documentație** (`9f858a3`, `4287940`)
- `docs/PRIVACY.md` (en) și `docs/PRIVACY.ro.md`: singura rețea e verificarea de actualizări către două adrese GitHub; ce conține cererea; ce se salvează local; ce face dezinstalarea.
- README: secțiune de instalare cu avertismentul SmartScreen și pașii „More info → Run anyway”.
- SAFETY.md și ghidurile ro/en sunt actualizate.

## Ce s-a testat

| Comandă | Exit | Observații |
|---|---|---|
| `cargo build --workspace --all-targets` | 0 | 0 warnings |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | |
| `cargo clippy -p valet-core --all-targets --features test-support -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `cargo test --workspace` | 0 | 454 trecute, 0 picate, 4 ignorate (cele 4 de performanță, rulate cu `--ignored` în CI) |
| `VALET_FAULT_ITERS=100 cargo test -p valet-core --features test-support --test fault_injection` | 0 | 100 de iterații, fiecare fișier o singură dată, la sursă sau la destinație |
| `pnpm lint`, `pnpm test`, `pnpm check:i18n`, `pnpm build` | 0 | 181 de teste vitest, 416 chei ro = en |
| Release dry run, [run 40857121076](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40857121076) | verde | `Valet_0.1.0_x64-setup.exe`, 4,15 MB |
| Release dry run, [run 40860957894](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40860957894) (`1d910c9`, `Valet.exe`, updater în cod) | verde | 4,94 MB, sha256 `82706c3e…8a44` |
| CI, [run 40866474361](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40866474361) (`d2e1974`) | verde | toate cele 6 joburi, blocante: core Linux, frontend, e2e Linux, **e2e Windows**, workspace Windows, **installer Windows** |
| CI, [run 40870393351](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40870393351) (`1ac103c`, fix-ul watcher-ului) | roșu | 5 din 6 verzi, inclusiv testele watcher-ului pe Windows; `installer (Windows)` a picat pe cheia `Run` lipsă din runner (vezi mai sus) |
| CI, [run 40872865750](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40872865750) (`21cd92b`, ultimul commit de cod) | verde | toate cele 6 joburi |

## Măsurători vs. bugete (PLAN.md 2.6)

Măsurate pe `Valet.exe` instalat de NSIS, pe runner-ul Windows din CI (run 40872865750, `21cd92b`). Run-ul 40866474361 a dat valori apropiate (98,3 MB, 0,12%, 54 ms).

| Metrică | Buget | Măsurat |
|---|---|---|
| RAM la relanti, private bytes (app + procesele WebView2) | < 150 MB | **95,0 MB** medie (max 100,1 MB) |
| RAM la relanti, working set (raportat, fără buget) | — | 309,5 MB medie |
| CPU la relanti, 60 s | < 1% | 0,09% medie (max 1,93%) |
| Pornire până la prima fereastră | < 2 s | 64 ms |
| Previzualizare pe 10.000 de fișiere | < 3 s | 0,09–0,13 s (Faza 2; planificatorul nu s-a schimbat) |
| Indexare 100.000 de fișiere | < 60 s | ~9 s (Faza 2) |
| Instalare / dezinstalare silențioasă | — | 2,1 s / 0,8 s |

## Probleme cunoscute

1. **Updater-ul nu a fost testat cap-coadă**: nu există cheie și nici un release publicat. Verificat: endpoint-ul ales după canal (un build local cu configurație injectată cere doar URL-ul canalului), refuzul cât rulează un lot, lipsa cererilor de rețea fără configurație. Neverificate: descărcarea, verificarea semnăturii și instalarea unui update semnat (criteriul „beta.1 → beta.2 pe VM” rămâne la tine).
2. **După un update pornit cu `--minimized`** (autostart), installer-ul NSIS repornește aplicația tot ascunsă în tray. Pluginul nu permite altfel.
3. **Verificarea automată de actualizări nu se poate opri din Setări.** Textul din Setări și PRIVACY.md spun exact când rulează.
4. **Sentry nu e integrat** (decizia 2), deci nu există raportare de erori și nici ecranul „Ce trimitem”. Panica ajunge doar în logul local.
5. **Instalatorul e nesemnat**, deci SmartScreen avertizează. Semnarea așteaptă SignPath/Certum (PLAN 3.3).
6. **Dezinstalarea silențioasă păstrează datele.** Bifa „Delete the application data” din dezinstalarea interactivă șterge baza de date, istoricul și logurile.
7. **Logurile nu sunt în `%LOCALAPPDATA%\Valet\logs`** (PLAN 4.5), pentru că acolo e folderul de instalare NSIS. Sunt în `%LOCALAPPDATA%\ro.valetapp.desktop\logs`.
8. **Baza de date apare la 2,4–4,8 s după pornire** pe runner (măsurat în trei run-uri), deși fereastra apare în sub 100 ms. Nu e un eșec, dar merită investigat în Faza 4.
9. **Capturile din ghid sunt de pe Linux**, iar `settings.png` e dinaintea secțiunilor noi din Settings.
10. **Instalarea silențioasă (`/S`) creează și o scurtătură pe Desktop.** Dezinstalarea o șterge. Instalarea interactivă are bifa obișnuită.
11. **Cât nu există niciun release publicat**, verificarea de actualizări primește 404, iar pluginul scrie o linie ERROR în log. În interfață apare doar un mesaj de eroare la verificare. Linia dispare după primul release.

## Decizii necesare de la Vlad

1. **Capturile Windows**: împinge `docs/screenshots/` (lipsește pe remote), ca să le pun în ghiduri.
2. **Sentry / raportare de erori**: crate-ul `sentry` nu e în allowlist și cere un cont plus DSN. Opțiuni:
   - (a) aprobi `sentry` și creezi contul; implementez opt-in cu ecranul „Ce trimitem” și curățarea căilor;
   - (b) amânăm după beta.

   Propunere: (b). Logul local acoperă beta-ul, iar PLAN 3 cere oricum doar opt-in.
3. **Iconița**: cea de acum e provizorie. Îmi dai o iconiță desenată (PNG 1024×1024) sau păstrăm placeholder-ul pentru beta?
4. **Cheile și primul release**, pe care le faci tu, după `docs/RELEASE.md`:
   - (1) `pnpm tauri signer generate`;
   - (2) secretele `TAURI_SIGNING_PRIVATE_KEY` și `_PASSWORD`, plus variabila `VALET_UPDATER_PUBKEY`;
   - (3) `node scripts/set-version.mjs 1.0.0-beta.1`, mutarea secțiunii din CHANGELOG, commit și tag `v1.0.0-beta.1`;
   - (4) verificarea draft-ului și publicarea;
   - (5) la fel pentru beta.2, apoi testul de upgrade pe VM.
5. **Oprirea verificării automate de actualizări** din Setări: o adăugăm? Propunere: da, în Faza 4, cu implicit pornit.
6. **SignPath Foundation / Certum și contul Microsoft Partner Center** (PLAN Faza 3, „Decizie Vlad înainte de fază”): sunt încă deschise.

## Checklist manual pentru Vlad

- [ ] CI verde pe `main`: https://github.com/OsztrovszkyVlad/Valet/actions (6 joburi).
- [ ] Release dry run: Actions → Release → Run workflow (`dry_run`), apoi descarcă artefactul `valet-installer-*`.
- [ ] Pe un Windows 11 curat (VM):
  - [ ] instalează (SmartScreen: More info → Run anyway);
  - [ ] instalarea e per utilizator, fără UAC, în `%LOCALAPPDATA%\Valet\Valet.exe`, cu scurtătură în Start;
  - [ ] la fel pe Windows 10 22H2, unde WebView2 se instalează prin bootstrapper.
- [ ] Onboarding → Organizează → Anulează, ca în Faza 2.
- [ ] Settings → Foldere: adaugă un folder de pe un stick USB → scoate stick-ul → apare „Indisponibil acum” → introdu-l la loc → în cel mult 10 minute e urmărit din nou.
- [ ] Settings → Actualizări: versiunea 0.1.0, canalul Beta. „Verifică acum” arată „Actualizările nu sunt configurate” până la primul release semnat.
- [ ] Settings → Asistență: pornește jurnalul detaliat, organizează câteva fișiere și verifică liniile DEBUG din `%LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log`; oprește-l.
- [ ] Dezinstalează din Settings → Apps:
  - [ ] fără bifă, datele rămân și, la reinstalare, istoricul e acolo;
  - [ ] cu bifă, datele dispar, iar fișierele organizate rămân unde sunt.
- [ ] După decizia 4: release `v1.0.0-beta.1`, instalare, release `v1.0.0-beta.2` → notificare „Versiunea … este disponibilă” → instalare din Setări → repornire pe beta.2, cu regulile și istoricul păstrate.
- [ ] winget (după primul release): `node scripts/winget-manifest.mjs …`, `winget validate`, `wingetcreate submit`.
- [ ] Răspunde la deciziile 1–6.

## Propunere pentru faza următoare

Criteriile de ieșire ale Fazei 3 sunt trei: release publicat din tag, upgrade beta.1 → beta.2 pe VM și manifest winget acceptat. Toate trei depind de cheile și conturile tale (decizia 4). Pipeline-ul e gata și testat prin dry run, iar instalarea și dezinstalarea sunt testate automat pe Windows.

Propun să considerăm Faza 3 închisă din partea mea și să bifezi aceste trei criterii când faci primul release. Faza 4 poate porni în paralel cu modulul de licență, care nu depinde de release. Restul planului rămâne neschimbat.
