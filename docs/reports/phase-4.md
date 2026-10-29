# Raport Faza 4 — Beta publică, licențe, plăți

Punct de pornire: `827c6be` (raportul Fazei 3). Sunt 39 de commit-uri pe `main` (`git log --oneline 827c6be..HEAD`).

## Deciziile tale, aplicate

- Allowlist-ul din CLAUDE.md conține acum `minisign-verify` (verificarea licenței) și `reqwest` (doar aplicația desktop), pe care le-ai aprobat la începutul fazei (`7f6082f`).
- PLAN.md 4.5 dă calea reală a logurilor: `%LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log` (`7f6082f`).
- Comutatorul pentru verificarea automată de actualizări e făcut și pornit implicit (detalii mai jos).
- Instalarea silențioasă nu mai creează scurtătura de pe Desktop (detalii mai jos).
- Baza de date apărea la 2,4–4,8 s după pornire. Am găsit cauza și am reparat-o (detalii mai jos).
- Capturile de ecran: `docs/screenshots/` încă nu există pe `origin/main` (verificat la sfârșitul fazei), așa că ghidurile au aceleași imagini ca până acum.

## Ce s-a construit

### Contractul de licență: `docs/LICENSING.md`

Un singur document descrie cum lucrează împreună nucleul, aplicația, serverul de licențe și interfața. Cuprinde:
- planurile și limitele;
- proba de 14 zile și ce se întâmplă când treci pe Gratuit;
- formatul fișierului de licență și al tokenului de activare;
- verificarea online;
- comenzile Tauri.

Toți agenții au lucrat după el. Am modificat contractul de 5 ori pe parcurs, când implementarea a arătat goluri; de exemplu, „ultimul lot” la undo nu include loturile de recuperare.

### Nucleul (`crates/valet-core`)

Commit-uri `a43a939` și `723320e`:
- `license.rs`:
  - fișierul de licență e un JSON semnat Ed25519 în format minisign (prehashed `ED`), verificat cu `minisign-verify`;
  - `resolve_plan` e o funcție pură care dă Gratuit, Probă sau Pro, plus codurile de motiv.
  - Regulile: tokenul e valabil 60 de zile; după 30 de zile se cere o verificare online; un token datat cu mai mult de o zi în viitor e invalid; licența acoperă un build doar dacă data lui de build e până la `updates_until`.
- `entitlements.rs`: limitele Gratuit (3 reguli proprii, 2 foldere, 7 zile de istoric, undo doar pe ultimul lot `apply`, fără export/import).
  - `apply_free_limits` și `restore_after_upgrade` sunt idempotente și nu schimbă `version`, deci aprobările rămân valabile.
- `rule_transfer.rs`: export și import de reguli.
  - Regulile importate primesc id-uri noi, deci cer o previzualizare înainte de aplicarea automată.
  - Importul are limite de 1 MiB și 500 de reguli.
- Fișierele `crates/valet-core/license/{pubkey,api,checkout}.txt` sunt goale. Cât rămân goale, licențele nu sunt active și aplicația nu face nicio cerere de licență.

### Platforma (`crates/valet-platform`)

Commit `254ea00`:
- `machine_id()` întoarce primele 32 de caractere hex din `blake3("valet-machine-v1:" + MachineGuid)`. MachineGuid e citit din HKLM, doar citire.
- `is_packaged()` detectează build-ul MSIX prin `GetCurrentPackageFullName`.

### Pornirea mai rapidă

Commit `86237ba`.
- **Cauza:** baza de date se deschidea în callback-ul `setup`, pe care Tauri îl cheamă abia după ce creează fereastra. Pe Windows, crearea ferestrei așteaptă pornirea WebView2, care durează câteva secunde la prima rulare.
- **Reparația:** baza de date și recuperarea pornesc pe runtime-ul async între `build` și `run`, în paralel cu fereastra.
  - `setup` așteaptă să se termine, apoi pornește `AppState`, sincronizarea planului, watcher-ul și auto-aplicarea, în această ordine.
  - Blocarea celei de-a doua instanțe se ia înainte de deschiderea bazei de date.
- **Măsurat în CI Windows:** baza de date apare la 131–142 ms după pornire, față de 2,4–4,8 s înainte.

### Aplicația (`apps/desktop/src-tauri`)

Commit-uri `015ff33`, `39f3c9f`, `344ce2c`, `3c8ec7c`:
- **Comenzi noi:** `get_plan_status`, `import_license_file`, `import_license_text`, `check_license_now`, `remove_license`, `apply_plan_choice`, `export_rules_file`, `import_rules_file`, `open_external`.
  - Dialogurile de fișier se deschid din Rust (`tauri-plugin-dialog`), fără pachet npm.
  - `open_external` poate deschide doar URL-ul de cumpărare și `ms-settings:startupapps`.
- **Limitele sunt verificate în backend:**
  - `upsert_rule` și `save_config` refuză depășirea cu `plan.limit`;
  - `get_config` întoarce `inactive_paths`;
  - `list_batches` întoarce `{batches, hidden_older, latest_apply_batch_id}`;
  - `undo_batch` întoarce `plan.undoLatestOnly`;
  - watcher-ul și planificarea sar peste folderele inactive.
- **Trecerile între planuri** rulează la pornire, din oră în oră și la comenzile care schimbă planul. Regulile se scriu cu `db.upsert_rule`, care păstrează versiunea. Când ceva se schimbă, aplicația emite evenimentul `plan-changed`.
- **Clientul de licență** (`license_client.rs`, `reqwest`):
  - folosește doar HTTPS, fără redirecturi, cu timeout de 10 s;
  - trimite doar `license_id`, `machine` și `app_version`;
  - dacă licențele nu sunt configurate, nu construiește niciun client HTTP (verificat de un test);
  - la răspunsurile 404, 409 și 410, tokenul e mutat în `license-removed/`, nu șters.
- **Actualizări:**
  - setarea `auto_update_check` e citită din nou la fiecare ciclu, deci nu cere repornire;
  - `check_for_update` întoarce acum `pub_date` și `covered_by_license`.
- **În build-ul din Store:** `update.packaged` și `autostart.packaged`, iar bucla automată de actualizare nu pornește.
- **Data de build:** `build.rs` setează `VALET_BUILD_DATE`.

### Interfața (`apps/desktop/src`)

Commit-uri `19a8384`, `9f56e54`, `8c1b841`, `fc6acc0`, `07199a1`, `3c339d6`:
- **Secțiunea Valet Pro din Setări:**
  - planul curent, tabelul Gratuit/Pro și prețul „29 € (TVA inclus)”;
  - butonul „Cumpără Pro”, dezactivat cât nu există link de cumpărare;
  - import de licență din fișier sau ca text, verificare acum, scoatere, codul licenței (pentru asistență);
  - motivul, în cuvinte, pentru care nu ai Pro.
- **Header:** „Probă Pro: N zile” pe durata probei, „Gratuit” pe Gratuit, nimic pe Pro. Nu apare nicio fereastră pop-up în timpul probei.
- **Dialogul „ce rămâne activ”:** maximum 3 reguli și 2 foldere. Escape înseamnă „decid mai târziu”.
- **Limitele, acolo unde apar:**
  - mesajul `plan.limit` la reguli și foldere;
  - eticheta „dezactivată de planul Gratuit”;
  - nota de 7 zile din Istoric și undo dezactivat pe celelalte loturi;
  - butoanele de export/import marcate „Pro”.
- **Actualizări:**
  - bifa „Verifică automat actualizările”;
  - un avertisment înainte de instalarea unei versiuni pe care licența nu o mai acoperă;
  - în build-ul din Store, secțiunea de actualizări și comutatorul de autostart sunt înlocuite cu note.
- 539 de chei i18n, identice în ro și en.

### Serverul de licențe (`services/license/`)

Commit-uri `320dc3b` și `3a66ba7`:
- Node ≥ 20, fără dependențe.
- Semnează în formatul minisign `ED`.
- Rutele `POST /v1/activate`, `/v1/validate`, `/v1/deactivate`: maximum 3 calculatoare, al patrulea primește 409, o licență revocată primește 410.
- Activările sunt stocate într-un fișier JSON, cu scriere atomică. Nimic nu se șterge.
- Unelte: `keygen.mjs`, `issue.mjs`, `reset.mjs` (pentru un calculator vechi care a dispărut), `revoke.mjs`.
- Nu sunt încă făcute: webhook-ul comerciantului, adaptorul de hosting și rate limiting.
- Testele (41) rulează în jobul frontend din CI.

### Instalatorul

Commit-uri `5b7cdbe`, `6c38e81`, `d063308`:
- `packaging/nsis/hooks.nsh`:
  - instalarea silențioasă (`/S`, winget) nu mai creează scurtătura de pe Desktop;
  - actualizările păstrează alegerea utilizatorului;
  - instalarea interactivă e neschimbată.
- **Bug găsit de testul nou:** după o instalare silențioasă, actualizarea pasivă a updater-ului (`/P /UPDATE`) se oprea la dialogul de limbă și aștepta un clic.
  - Cauza: MUI salvează limba doar pe o pagină care nu rulează în modul silențios.
  - Reparația: hook-ul salvează limba la instalarea silențioasă.
  - Testul din CI e acum o verificare blocantă: actualizarea pasivă se termină în 2,3 s, cu exit 0.

### MSIX pentru Microsoft Store

Commit `4938ef4`:
- `packaging/msix/AppxManifest.xml`:
  - aplicație full-trust, în ro-RO și en-US;
  - un `StartupTask` dezactivat, controlat din Setările Windows;
  - identitatea din variabilele Actions `VALET_MSIX_*`; fără ele se folosește o identitate falsă, cu avertisment.
- `scripts/msix-pack.ps1`, `scripts/msix-smoke.ps1`, `scripts/msix-manifest.mjs`.
- Workflow-ul `.github/workflows/msix.yml`:
  - construiește fără configurația updater-ului (verificat de două ori);
  - instalează cu un certificat de unică folosință creat în job (niciodată exportat sau salvat);
  - pornește aplicația prin `shell:AppsFolder` și verifică baza de date, apoi dezinstalează;
  - încarcă `.msix` nesemnat, ca artefact.
- Doar versiunile stabile merg în Store (`x.y.z` devine `x.y.z.0`); versiunile beta rămân pe GitHub.

### Documente

Commit-uri `cad69f2`, `ca4f1ef`, `5c57d3f`, `650e83a`, `f4b4aa3`, `3a2b0b3`, `4e67866`, `af2ab0c`, `03995ec`, `6fa7b61`:
- **Site:** o pagină statică în `site/` (ro și en, plus confidențialitate), fără JavaScript, fără resurse externe, fără tracking. Link-ul de descărcare duce la `releases`, care include și pre-release-urile.
- **Ghidurile ro/en:** secțiunea 18, „Valet Pro și planul Gratuit”, plus diferențele build-ului din Store.
- **PRIVACY și SAFETY:** verificate cu trimiteri `fișier:linie` la codul clientului de licență.
- **Alte texte:** CHANGELOG, README, șabloanele de răspuns pentru asistență și textele pentru listarea în Store (ro/en).
- **Anunțuri:** ciorne pentru r/software, r/windows și Show HN, marcate „DRAFT — Vlad decides”.

### Cercetare: Stripe și ceilalți comercianți

Răspuns la întrebarea ta „Stripe?”. Sursa e un sumar al rezultatelor de căutare din 29 octombrie; paginile Stripe, Paddle și Polar nu au putut fi citite direct din sesiune.

| | Cine e vânzătorul | TVA | Comision la 29 € | Chei de licență |
|---|---|---|---|---|
| Stripe direct | tu (PFA) | tu: înregistrare OSS la ANAF, declarații trimestriale | procesare + 0,5% (Stripe Tax doar calculează) | nu |
| Stripe Managed Payments (GA din aprilie 2026) | Stripe | Stripe | 3,5% + procesare | nu |
| Polar | Polar | Polar | ~5% + 0,50 $ + taxe de plată | da |
| Paddle | Paddle | Paddle | ~5% + 0,50 $ | nu |

Dacă alegi Stripe, varianta potrivită e Managed Payments. Înainte de decizie trebuie verificate trei lucruri:
1. România e pe pagina de eligibilitate (un singur sumar spune că da).
2. Stripe acceptă un PFA pentru Managed Payments.
3. Webhook-ul `checkout.session.completed` conține emailul cumpărătorului.

Serverul nostru emite oricum fișierul de licență, deci oricare dintre cei patru se potrivește.

## Ce s-a testat

| Comandă | Exit | Observații |
|---|---|---|
| `cargo build --workspace --all-targets` | 0 | |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | |
| `cargo clippy -p valet-core --all-targets --features test-support -- -D warnings` | 0 | |
| `cargo clippy -p valet-platform --all-targets --target x86_64-pc-windows-msvc -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `cargo test --workspace` (PM, `3c339d6`) | 0 | 565 trecute, 0 picate, 4 ignorate (cele de performanță, rulate în release mai jos) |
| `VALET_FAULT_ITERS=100 cargo test -p valet-core --features test-support --test fault_injection` | 0 | 100 de iterații: 73 opriri în mijlocul lotului, 61 de decizii de recuperare, 91 de loturi anulate; fiecare fișier o singură dată |
| `node --test` (`services/license`) | 0 | 41/41 |
| `pnpm lint`, `pnpm test`, `pnpm check:i18n`, `tsc --noEmit`, `pnpm build` | 0 | 239 de teste vitest, 539 de chei ro = en |
| Smoke UI (`tests/e2e/smoke.mjs`, local Xvfb) | 0 | pas nou: badge „Probă Pro: 14 zile”, secțiunea Pro neconfigurată, cumpărarea dezactivată |
| `node scripts/check-site.mjs` | 0 | 360 și 1280 px, temă light/dark: fără overflow orizontal, 0 cereri externe, contrast minim 7,54:1 |
| CI, [run 40909021759](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40909021759) (`fc6acc0`, tot codul fazei) | verde | toate cele 6 joburi |
| MSIX, [run 40904535628](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40904535628) (`d063308`) | verde | împachetare 6,97 MB; instalare, pornire 10 s, baza de date la 356 ms după lansare, dezinstalare |
| CI, [run 40913739498](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40913739498) (`3c339d6`, ultimul commit de cod) | verde | toate cele 6 joburi, inclusiv smoke-ul UI pe Windows cu pasul nou de Pro și installer-ul cu actualizarea pasivă |

**Matricea de licență din PLAN.** Testele rulează în `apps/desktop/src-tauri/src/licensing/service_tests.rs` și `transition_tests.rs`, contra serverului real pornit local, cu `now` și id-ul calculatorului injectate:
- **Prea multe activări:** 3 calculatoare primesc Pro, al patrulea primește `license.too_many`, iar fișierul de licență rămâne. Scoaterea licenței eliberează locul. După un `reset.mjs`, validarea muta tokenul în `license-removed/`.
- **Expirată:** un build după `updates_until` dă Gratuit cu `license.not_covering_build`. Un build mai vechi rămâne Pro.
- **Falsă:** un payload modificat sau altă cheie dau `license.invalid_signature`. O licență neînregistrată dă `license.unknown`.
- **Offline:** la 31 de zile e tot Pro. La 61 de zile e Gratuit (2 reguli dezactivate, nimic șters). O validare reușită readuce Pro, cu toate cele 5 reguli și versiunile neschimbate.
- **Revocată:** 410 dă `license.revoked`. Tokenul e mutat, nu șters.
- **Trecerile între planuri, pe date reale:** jurnalul rămâne identic, iar aprobările sunt valabile după un ciclu Pro → Gratuit → Pro.
- **Undo pe Gratuit:** cu loturile [recuperare, apply A, apply B], doar A se poate anula. După anularea lui A, B rămâne blocat.
- **Verificare în ambele sensuri:** licențele semnate de server sunt verificate de nucleu și invers. Orice octet modificat e respins. Fixture-urile se regenerează identic.
- **Cumpărarea în sandbox Paddle nu e făcută:** nu e ales comerciantul.

## Măsurători vs. bugete (PLAN.md 2.6)

Măsurate pe `Valet.exe` instalat de NSIS, pe runner-ul Windows din CI (run 40909021759), plus testele de performanță în release pe Linux.

| Metrică | Buget | Măsurat |
|---|---|---|
| RAM la relanti, private bytes | < 150 MB | 97,9 MB medie (max 101,9 MB) |
| CPU la relanti, 60 s | < 1% | 0,10% medie (max 2,74%) |
| Pornire până la prima fereastră | < 2 s | 59 ms |
| Baza de date după pornire (nu are buget; aveai întrebarea) | — | 142 ms la prima pornire, 9 ms după reinstalare (înainte 2,4–4,8 s) |
| Previzualizare pe 10.000 de fișiere | < 3 s | 0,098–0,136 s |
| Indexare 100.000 de fișiere | < 60 s | 6,68 s |
| Actualizare pasivă după instalare silențioasă | — | 2,3 s (înainte se bloca) |

## Probleme cunoscute

1. **Licențele nu sunt active în niciun build**, pentru că `pubkey.txt`, `api.txt` și `checkout.txt` sunt goale (decizia 3). Proba de 14 zile și planul Gratuit funcționează. Cumpărarea și activarea Pro nu se pot face încă.
2. **Dezinstalarea aplicației din Store șterge datele pachetului**, adică jurnalul și istoricul de undo. Așa face Windows cu MSIX. Ghidul și textul pentru Store avertizează despre asta.
3. **Neverificate pe un Windows real (doar compilate și testate în CI):**
   - dialogurile de fișier;
   - `open_external`;
   - notificările sub identitatea MSIX;
   - ce se întâmplă când același utilizator are atât instalarea NSIS, cât și pe cea din Store.
4. **Data de build vine din `SOURCE_DATE_EPOCH`.** Fără ea, `build.rs` folosește momentul ultimei lui rulări. `release.yml` nu o setează, iar fișierul e protejat (decizia 5).
5. **Importul unei alte licențe nu dezactivează pe server licența veche.** Perechea veche e doar mutată în `license-removed/`.
6. **Scoaterea licenței fără internet** lasă locul ocupat pe server până la un `reset.mjs`. Ghidul spune asta.
7. **Build-ul din Store are butonul „Cumpără Pro”.** Politica Store pentru cumpărări externe trebuie verificată (decizia 7).
8. **Nu există ecranul „Ce trimitem” și nici Sentry.** Sunt amânate după beta, cum ai decis.
9. **Iconița e tot placeholder-ul.** Lipsește `resources.pri`, așa că logo-urile din Store nu au variante pe fundal transparent.
10. **Site-ul nu are încă `demo.gif`, domeniu sau `hreflang`/canonical.** Toate depind de domeniu.
11. **Avertisment de build:** când un worktree și arborele principal folosesc același `target/`, cargo poate rula binare de test vechi. QA a dat de asta o dată. Rulările finale s-au făcut în arborele principal.

## Decizii necesare de la Vlad

1. **Comerciantul:** Stripe Managed Payments, Paddle sau Polar. Webhook-ul serverului (cumpărare → `issue` → email) se scrie după alegerea ta. Pentru Stripe, verifică cele 3 puncte din secțiunea de cercetare.
2. **Hosting-ul serverului de licențe și serviciul de email** (Cloudflare Workers, Cloud Run, Supabase etc.). Serverul e portabil; lipsește doar adaptorul de stocare.
3. **Cheile și adresele:**
   - (1) rulezi `node services/license/keygen.mjs`;
   - (2) pui secretul doar în secretele hostului;
   - (3) comiți `pubkey.txt` (linia `RW…`), `api.txt` (URL-ul HTTPS) și, când cumpărarea e deschisă, `checkout.txt`.
4. **`rustls` ca dependență directă** a aplicației, cu feature-ul `ring`. Clientul de licență are nevoie de el ca furnizor criptografic pentru `reqwest`. E deja în build prin updater, deci binarul nu crește. Aprobi să intre în allowlist?
5. **`SOURCE_DATE_EPOCH` în `release.yml`:** e un rând, dar fișierul e protejat. Aprobi modificarea? Fără ea, avertismentul de acoperire a licenței poate greși cu ore sau zile.
6. **Partner Center:**
   - rezervi numele „Valet”;
   - setezi variabilele Actions `VALET_MSIX_IDENTITY_NAME`, `VALET_MSIX_PUBLISHER`, `VALET_MSIX_PUBLISHER_DISPLAY_NAME`;
   - rulezi `msix.yml` cu `dry_run=false` pe un tag stabil;
   - urci pachetul (pașii sunt în `packaging/msix/README.md`).
7. **Butonul „Cumpără Pro” în build-ul din Store:** îl păstrăm (politica Store permite plata externă pentru aplicații non-joc, dar verifică) sau îl ascundem când `packaged` e true?
8. **Iconița finală** înainte de Store: un PNG de 1024×1024 de la tine.
9. **Politica de rambursare.** E un placeholder în șabloane; `revoke.mjs` există dacă vrei ca rambursarea să revoce licența.
10. **Unde ții legătura email → cod licență** până există webhook-ul. Aplicația arată acum codul în Setări, așa că utilizatorul ți-l poate trimite.
11. **Textele anunțurilor** (`docs/launch/`) și un canal de asistență public (email sau GitHub Issues).
12. **Domeniul site-ului** (PLAN 8: `valetapp.ro` sau `getvalet.app`), plus verificarea mărcii în EUIPO înainte de Store.

## Checklist manual pentru Vlad

- [ ] CI verde pe `main`: https://github.com/OsztrovszkyVlad/Valet/actions (6 joburi, plus MSIX).
- [ ] Pe un Windows 11 curat, instalează build-ul din dry run-ul Release:
  - [ ] fără scurtătură pe Desktop după `/S`, cu scurtătură la instalarea interactivă dacă o bifezi;
  - [ ] în `valet.log`, „Database ready N ms” e sub ~300 ms.
- [ ] Header-ul arată „Probă Pro: 14 zile”. Setări → Valet Pro arată „Licențele nu sunt încă active…” și „Cumpără Pro” dezactivat.
- [ ] Testul trecerii pe Gratuit (cere schimbarea ceasului sau așteptarea celor 14 zile):
  - [ ] cu 5 reguli proprii și 3 foldere, apare dialogul „ce rămâne activ”;
  - [ ] alege alte reguli și verifică eticheta „dezactivată de planul Gratuit”;
  - [ ] Istoricul arată doar ultimele 7 zile, iar undo e permis doar pe ultimul lot.
- [ ] Setări → Actualizări: debifează „Verifică automat actualizările” și repornește. În log nu apare nicio verificare la 30 s.
- [ ] Reguli → Export (pe durata probei): salvează un fișier, apoi încearcă să salvezi peste el; trebuie refuzat. Importă-l pe alt profil; regulile cer o previzualizare.
- [ ] MSIX: descarcă artefactul `valet-0.1.0-msix-x64-dryrun`. Pentru sideload ai nevoie de un certificat de test (vezi `packaging/msix/README.md`). Verifică:
  - [ ] fără secțiunea de actualizări;
  - [ ] autostart prin „Deschide setările Windows”.
- [ ] După decizia 3, pe serverul tău:
  - [ ] `issue.mjs` și importul licenței pe două calculatoare dau Pro;
  - [ ] al patrulea calculator e refuzat;
  - [ ] „Scoate licența” eliberează locul.
- [ ] Criteriul mutat din Faza 3: release `v1.0.0-beta.1`, apoi `v1.0.0-beta.2`, apoi upgrade din updater pe VM, cu regulile și istoricul păstrate.
- [ ] Răspunde la deciziile 1–12.

## Propunere pentru faza următoare

Criteriile de ieșire ale Fazei 4 depind toate de tine:
- o cumpărare reală de test, pentru care trebuie ales comerciantul și pus pe hosting serverul;
- licența pe 2 calculatoare, cu al patrulea refuzat, pe un server real (pe un server local e deja testat);
- listarea aprobată în Store, care cere Partner Center;
- 50 de instalări externe;
- upgrade-ul beta.1 → beta.2 pe VM.

Codul pentru toate acestea e gata și testat local și în CI.

Propun să lăsăm Faza 4 deschisă pentru acești pași. Când alegi comerciantul și hosting-ul, scriem webhook-ul și adaptorul de stocare ca o sarcină scurtă, apoi închidem faza. Faza 5 (spațiu pe disc) nu depinde de ei și poate porni după aprobarea ta.
