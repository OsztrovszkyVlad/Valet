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
| `cargo test --workspace` (PM, `3c339d6`; din nou la `8e2bf7c`, 565/0) | 0 | 565 trecute, 0 picate, 4 ignorate (cele de performanță, rulate în release mai jos) |
| `VALET_FAULT_ITERS=100 cargo test -p valet-core --features test-support --test fault_injection` | 0 | 100 de iterații: 73 opriri în mijlocul lotului, 61 de decizii de recuperare, 91 de loturi anulate; fiecare fișier o singură dată |
| `node --test` (`services/license`) | 0 | 41/41; după completare 86/86 (webhook, Resend, rate limiting, backup, `lookup --order`) |
| `pnpm lint`, `pnpm test`, `pnpm check:i18n`, `tsc --noEmit`, `pnpm build` | 0 | 239 de teste vitest, 539 de chei ro = en |
| Smoke UI (`tests/e2e/smoke.mjs`, local Xvfb) | 0 | pas nou: badge „Probă Pro: 14 zile”, secțiunea Pro neconfigurată, cumpărarea dezactivată |
| `node scripts/check-site.mjs` | 0 | 360 și 1280 px, temă light/dark: fără overflow orizontal, 0 cereri externe, contrast minim 7,54:1 |
| CI, [run 40909021759](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40909021759) (`fc6acc0`, tot codul fazei) | verde | toate cele 6 joburi |
| CI, [run 40929292606](https://github.com/OsztrovszkyVlad/Valet/actions/runs/40929292606) (`8e2bf7c`, după completare: webhook, rate limiting, backup, avertismentul de probă) | verde | toate cele 6 joburi |
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

## Completare după deciziile tale (29 octombrie)

Ai cerut o sarcină scurtă: webhook → `issue` → email, Fly.io, rate limiting, backup, `SOURCE_DATE_EPOCH`, `rustls`, `docs/STORE.md`. Placeholder-ele din mesaj le-am lămurit cu tine:
- webhook pentru ambii comercianți, ales cu `VALET_MERCHANT`;
- domeniul `valetapp.ro`;
- asistența doar prin GitHub Issues;
- cheile le împingi tu.

| Ce | Unde | Stare |
|---|---|---|
| `rustls` (feature `ring`) în allowlist | CLAUDE.md (`09bb1bb`) | făcut |
| Webhook Paddle și Stripe Managed Payments, emisă licența, email prin Resend cu fișierul atașat, revocare la rambursare | `services/license/` (`3eaa51c`) | făcut, 86/86 teste |
| Rate limiting per IP (activate/validate/deactivate), maximum 4 webhook-uri simultan, IP-urile nu se loghează | `lib/ratelimit.mjs` | făcut |
| Backup zilnic al JSON-ului pe volum (copii datate, ultimele 30), `/healthz` | `lib/backup.mjs` | făcut |
| Fly.io: `Dockerfile`, `fly.toml` (regiunea `fra`, un singur calculator, volum `/data`), pașii de deploy | `services/license/README.md` | făcut; imaginea Docker nu s-a putut construi aici (nu există daemon), comanda de pornire a rulat pe fișierele copiate |
| Asistența după numărul comenzii: `lookup.mjs --order`, `reset.mjs`/`revoke.mjs --order` | `services/license/` | făcut |
| `docs/STORE.md`: citate exacte din politica Microsoft Store 7.20 (10.8.1, 10.8.2, 10.8.3, 10.8.4, 10.8.7, 10.3), textul pentru „Notes for certification” | `32dc6b8` | făcut |
| Avertisment în ultimele 3 zile de probă (10.8.4: utilizatorul trebuie anunțat înainte să piardă accesul la regulile lui) | `PlanBadge` (`25f56ec`) | făcut |
| Rambursare 14 zile, licența se revocă; asistență prin GitHub Issues; canonical, hreflang, og:url pe `valetapp.ro` | site, ghiduri, șabloane, PRIVACY (`8078d4f`, `fa8c070`, `cc20d8a`, `7ea8443`) | făcut |
| `SOURCE_DATE_EPOCH` în `release.yml` | — | **blocat de hook-ul `guard-edit`** (vezi mai jos) |

Identificatorul aplicației, `ro.valetapp.desktop`, se potrivește deja cu domeniul, deci nu l-am schimbat.

**Problemă de securitate găsită pe drum.** Doar codul de licență ajunge ca să ocupi o activare, iar GitHub Issues e public. De aceea asistența nu mai cere niciodată codul, ci numărul comenzii din emailul comerciantului. Textul din Setări avertizează acum: „Nu-l publica…” (`018d81b`).

**Ce face o cumpărare:**
1. Comerciantul trimite `POST https://license.valetapp.ro/v1/webhook/<comerciant>`.
2. Semnătura HMAC e verificată pe corpul brut, cu toleranță de 5 minute și comparație în timp constant.
3. Evenimentul e verificat: produsul corect, plătit.
4. Comanda e legată de o licență nouă (365 de zile, 3 activări), înainte de email.
5. Licența e trimisă prin Resend. Dacă trimiterea eșuează, serverul răspunde 502, comerciantul retrimite evenimentul, iar serverul trimite aceeași licență.
6. La rambursare, licența e revocată, iar aplicația primește 410 la următoarea verificare.

Serverul nu păstrează emailul.

**Lipsă de verificat:** documentația Paddle, Stripe, Fly și Resend nu se poate citi din sesiune. Formatul evenimentelor e scris din cunoștințe, iar fixture-urile copiază formatul documentat. Lista de verificat în sandbox e în `services/license/README.md`. Vânzarea se deschide doar după ce trece toată lista (decizia 10).

**Pentru `SOURCE_DATE_EPOCH`:** hook-ul `.claude/hooks/guard-edit.sh` blochează orice editare a `release.yml`, chiar și cu aprobarea ta. Nu l-am ocolit. Adaugă tu acest rând după pasul `actions/checkout` din jobul `build` (după `persist-credentials: false`):

```yaml
      - run: echo "SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)" >> "$GITHUB_ENV"
```

`build.rs` citește deja variabila. Cu rândul adăugat, data de build devine data commit-ului.

## Completare 2: decizii noi (29 octombrie)

| Decizie | Ce s-a făcut | Commit |
|---|---|---|
| 1. Chargeback-urile și disputele revocă licența și o marchează „disputed”; nu se reactivează prin reset, doar manual | Paddle `action = chargeback`, Stripe `charge.dispute.created` (plus o întrebare de clarificare care devine dispută). `reset.mjs` refuză licențele cu dispută. `undispute.mjs --confirm` e calea manuală. O dispută câștigată doar se notează în istoric. | `31b0636` |
| 2. `resend.mjs` și `reissue.mjs` | `resend` citește din nou emailul de la comerciant și semnează aceeași licență: fișierul e identic octet cu octet dacă emailul nu s-a schimbat. `reissue` revocă licența veche (motiv `reissued`), emite una nouă pe aceeași comandă, cu același `updates_until`, și trimite emailul. Ambele cer numărul comenzii. | `31b0636` |
| 3. Numele comerciantului lângă „Cumpără Pro” | Placeholder-ul `<Paddle/Stripe>` nu era completat, iar tu ai ales „din fișier”. Când `crates/valet-core/license/merchant.txt` conține `Paddle` sau `Stripe`, Setări → Valet Pro arată „Plata se face prin …, care emite factura și se ocupă de TVA.” (ro și en). Fișierul e gol acum. | `8249dd0` |
| 4. Iconița vine separat | — | — |

**Reparat pe drum:**
- O revocare din linia de comandă putea fi suprascrisă de o activare scrisă de server în același moment. Acum scrierile din store trec printr-un lacăt pe fișier, iar un test cu două procese confirmă că nu se pierde nimic.
- Retrimiterea unui webhook după un `reissue` ar fi dat eroare. E reparată.

**Teste:** `node --test` în `services/license` trece 118/118. `cargo test --workspace` trece 565/0. vitest trece 243.

**Ce am găsit pe `origin/main` la sfârșitul sarcinii:**
- `release.yml` nu are rândul `SOURCE_DATE_EPOCH`;
- `pubkey.txt` și `api.txt` au 0 octeți.

Mesajul tău spunea că sunt adăugate. Probabil push-ul nu a ajuns. Până ajung, licențele rămân inactive, iar data de build e momentul compilării.

**Sandbox:** nu a trecut încă (răspunsul tău), deci vânzarea rămâne închisă și `checkout.txt` e gol.

## Probleme cunoscute

1. **Licențele nu sunt active în niciun build.** `pubkey.txt` și `api.txt` sunt goale pe `origin/main`, iar tu ai spus că le împingi. `checkout.txt` rămâne gol până trece testul în sandbox.
2. **Dezinstalarea aplicației din Store șterge datele pachetului** (jurnalul, istoricul de undo). Așa face Windows cu MSIX. Ghidul și textul pentru Store avertizează despre asta.
3. **Neverificate pe un Windows real:**
   - dialogurile de fișier;
   - `open_external`;
   - notificările sub identitatea MSIX;
   - instalarea NSIS și cea din Store la același utilizator.
4. **`SOURCE_DATE_EPOCH` lipsește până adaugi rândul de mai sus.**
5. **Importul unei alte licențe nu dezactivează pe server licența veche.**
6. **Scoaterea licenței fără internet** lasă locul ocupat până la `reset.mjs --order`.
7. **Numerele de comandă vor apărea în GitHub Issues, care e public.** Nu e verificat ce permite un număr de comandă singur la comerciant.
8. **Ce afișează chitanța:** nu e confirmat că chitanța Stripe sau Paddle arată id-ul pe care îl caută `lookup.mjs` (`txn_…`, `cs_…`, `pi_…`). E pe lista de verificat în sandbox.
9. **Evenimentele de dispută sunt scrise din cunoștințe:** Paddle `chargeback`/`chargeback_reverse` și câmpurile disputelor Stripe. Sunt pe lista de verificat în sandbox. Nu e sigur că sandbox-ul Paddle poate simula un chargeback.
10. **Imaginea `node:22-alpine` nu e fixată pe digest.**
11. **Fără ecranul „Ce trimitem” și fără Sentry:** amânate după beta, cum ai decis.
12. **Iconița e tot placeholder-ul.** Lipsește `resources.pri`.
13. **Site-ul nu are `demo.gif` și nu e publicat pe `valetapp.ro`.**
14. **Avertisment de build:** când un worktree și arborele principal folosesc același `target/`, cargo poate rula binare de test vechi.

## Decizii necesare de la Vlad

1. **Comerciantul**, după răspunsul scris despre PFA: setezi `VALET_MERCHANT=paddle` sau `stripe`, apoi treci lista de verificat în sandbox.
2. **Push-ul:** rândul `SOURCE_DATE_EPOCH` din `release.yml`, plus `pubkey.txt` și `api.txt`. La sfârșitul sarcinii nu erau pe `origin/main`.
3. **Cheile și deploy-ul** (pașii exacți sunt în `services/license/README.md`):
   - `keygen.mjs`, apoi comiți `pubkey.txt`;
   - Resend: domeniul `valetapp.ro` cu DKIM/SPF, cheie API;
   - Fly: volumul, secretele, `fly deploy`, certificatul pentru `license.valetapp.ro`;
   - comiți `api.txt`.
4. **Stripe (dacă îl alegi):** o cheie restricționată cu drept de citire pe Checkout Sessions și Charges, pusă în `VALET_STRIPE_API_KEY`, plus evenimentele `charge.dispute.created`, `charge.dispute.updated` și `charge.dispute.closed`.
5. **`merchant.txt`:** îl comiți cu `Paddle` sau `Stripe` înainte de Store.
6. **Partner Center:**
   - rezervi numele „Valet” și setezi variabilele `VALET_MSIX_*`;
   - la fiecare trimitere, bifezi folosirea unui sistem de plată terț (10.8.2);
   - folosești textul pentru certificare din `docs/STORE.md`;
   - verifici marca în EUIPO.
7. **Iconița finală** (PNG de 1024×1024) înainte de Store: ai spus că vine separat.

## Checklist manual pentru Vlad

- [ ] CI verde pe `main` (6 joburi, plus MSIX).
- [ ] Rândul `SOURCE_DATE_EPOCH` adăugat în `release.yml`, iar un dry run Release trece.
- [ ] Pe un Windows 11 curat, din dry run-ul Release:
  - [ ] fără scurtătură pe Desktop după `/S`;
  - [ ] „Database ready N ms” sub ~300 ms în `valet.log`.
- [ ] Header: „Probă Pro: 14 zile”. Cu ceasul mutat la 3 zile înainte de sfârșitul probei, badge-ul spune „Proba se termină curând”, iar tooltip-ul spune ce devine inactiv.
- [ ] Trecerea pe Gratuit (5 reguli, 3 foldere): dialogul „ce rămâne activ”, eticheta pe reguli, Istoricul pe 7 zile, undo doar pe ultimul lot.
- [ ] Setări → Actualizări: cu bifa scoasă, nu apare nicio verificare automată în log.
- [ ] Export de reguli: salvarea peste un fișier existent e refuzată; importul cere o previzualizare.
- [ ] MSIX: fără secțiunea de actualizări; autostart prin setările Windows.
- [ ] Server pe Fly (după decizia 3):
  - [ ] `curl https://license.valetapp.ro/healthz` răspunde `ok`;
  - [ ] lista de verificat în sandbox din `services/license/README.md`, toată;
  - [ ] licența pe 2 calculatoare, al patrulea refuzat;
  - [ ] rambursarea revocă licența;
  - [ ] `lookup.mjs --order` găsește licența după numărul de pe chitanță.
- [ ] Pe server: `lookup.mjs`, `resend.mjs`, `reissue.mjs --confirm` și `undispute.mjs --confirm` pe o comandă din sandbox. După o dispută simulată, `reset.mjs` refuză.
- [ ] Release `v1.0.0-beta.1`, apoi `v1.0.0-beta.2`, upgrade din updater pe VM, apoi 7 zile de folosire zilnică. Abia după aceea anunțurile.

## Propunere pentru faza următoare

Codul Fazei 4 e complet. Criteriile de ieșire rămase sunt toate acțiuni ale tale: deploy-ul pe Fly și testul în sandbox, cumpărarea de test, listarea aprobată în Store, 50 de instalări externe și upgrade-ul pe VM. Planul nu cere nicio schimbare. Faza 5 nu pornește până nu spui.
