# Confidențialitate

English version: [PRIVACY.md](PRIVACY.md)

Pagina spune ce trimite Valet prin rețea, ce păstrează pe calculatorul tău, ce nu are și ce se întâmplă cu datele la dezinstalare. A fost scrisă întâi după commitul `1d910c9` și revizuită pentru Faza 4 la commitul `fc6acc0`. Verificarea licenței (secțiunea 1, „Verificarea licenței”), setarea „Verifică automat actualizările”, fișierele licenței și build-ul din Microsoft Store au fost verificate cu codul de la `fc6acc0`, cu trimiteri la fișier și linie. Celelalte afirmații au fost verificate la `1d910c9` și nu au fost repetate, în afară de locurile unde o comandă e numită cu rezultatul ei la `fc6acc0`. Unde ceva a fost citit din cod sau dintr-o dependență și nu a fost observat pe o instalare Windows care rulează, pagina spune asta. Nu există încă un server de licențe, deci verificarea licenței nu a rulat niciodată cu un server real; testele folosesc un server de probă local. Subsecțiunea „Cumpărarea unei licențe” a fost adăugată la 2026-10-29. Ea descrie ce hotărăște `docs/LICENSING.md` pentru webhook-ul comerciantului, găzduirea pe Fly.io și emailul prin Resend; nimic din acestea nu rulează încă, deci nimic nu a fost verificat pe un sistem pornit.

Pe scurt: Valet face două feluri de cerere de rețea. Una e verificarea unei versiuni noi a lui însuși; partea automată o poți opri din Setări. Cealaltă e verificarea licenței, și are loc doar dacă ai importat o licență Pro într-un build care are un server de licențe configurat. Nicio denumire de fișier, cale, regulă sau conținut de fișier nu face parte din vreuna. Restul rămâne pe calculatorul tău.

Cumpărarea unei licențe se face în afara aplicației și are o parte proprie de date (plata și emailul tău); e descrisă în secțiunea 1, „Cumpărarea unei licențe”. Cumpărarea nu e deschisă încă.

## 1. Ce merge prin rețea

La commitul `fc6acc0`, codul Valet deschide conexiuni în două locuri: actualizatorul (`apps/desktop/src-tauri/src/updater.rs`) și clientul de licențe (`apps/desktop/src-tauri/src/license_client.rs`). Singura bibliotecă de rețea din arborele de dependențe e `reqwest`. `cargo tree -p app -i reqwest --offline` a afișat la `fc6acc0` doi utilizatori: `app` însuși (clientul de licențe) și `tauri-plugin-updater`; la fel pentru `--target x86_64-pc-windows-msvc`. La `1d910c9`, înainte de clientul de licențe, afișa doar calea actualizatorului.

### Adresele

| Canal | Adresa |
|---|---|
| Stabil | `https://github.com/OsztrovszkyVlad/Valet/releases/latest/download/latest.json` |
| Beta | `https://github.com/OsztrovszkyVlad/Valet/releases/download/updater-channel/latest-beta.json` |

Valet interoghează doar adresa canalului tău. Canalul se alege în Setări, Actualizări. Până alegi unul, e Beta pentru o versiune de pre-lansare (un număr de versiune cu `-`, de exemplu `1.0.0-beta.1`) și Stabil în rest.

Dacă există o versiune mai nouă, fișierul `latest.json` indică un instalator din GitHub Releases. Valet descarcă acel instalator doar după ce apeși „Instalează și repornește” și confirmi. GitHub poate răspunde cu o redirecționare către serverele lui de descărcare; asta nu a fost surprinsă pe o instalare care rulează.

### Ce conține o cerere

Verificarea e un GET HTTPS pentru un fișier JSON static. Din cod și din sursa `tauri-plugin-updater` 2.14.1:

- Adresa nu are marcaje de înlocuire, deci versiunea instalată, versiunea de Windows și tipul de procesor nu sunt în adresă.
- Antetele puse de plugin sunt `Accept: application/json` și `User-Agent: tauri-plugin-updater/2.14.1`. User-Agent-ul numește versiunea pluginului, nu versiunea Valet. Descărcarea instalatorului folosește `Accept: application/octet-stream` și același User-Agent.
- Nu există corp de cerere și nici cookie.
- GitHub vede ce vede orice server web: adresa ta IP publică, ora, User-Agent-ul și datele conexiunii TLS. Jurnalizarea și păstrarea datelor la GitHub țin de GitHub, nu de Valet.
- Versiunile se compară pe calculatorul tău. Versiunea instalată nu se trimite.
- Cererea trece prin setările de proxy ale Windows (pluginul e construit cu funcția `system-proxy`).

Nicio denumire de fișier, cale de folder, regulă sau conținut de fișier nu face parte din vreo cerere. Fereastra web care afișează interfața are o politică de securitate a conținutului cu `connect-src` egal cu `'self' ipc: http://ipc.localhost` (`tauri.conf.json`), deci interfața nu poate apela alte adrese. Politica nu are test.

### Când

- La 30 de secunde după pornirea Valet, apoi o dată la 24 de ore cât timp rămâne pornit, atâta timp cât caseta „Verifică automat actualizările” din Setări, Actualizări e bifată (din oficiu e bifată). Codul se uită la ceas din oră în oră și verifică dacă au trecut 24 de ore, deci timpul petrecut în repaus se numără. Așadar fiecare pornire a Valet, într-un build care are actualizări și cu setarea pornită, face o verificare automată.
- Când apeși „Verifică acum” în Setări, Actualizări.
- Când apeși „Instalează și repornește”, Valet descarcă instalatorul.

O verificare automată nu instalează nimic. Doar arată o notificare (o dată pe versiune la fiecare rulare a Valet) și un semn pe butonul Setări. Instalarea cere un click și o confirmare. Instalarea așteaptă cât timp rulează un lot de operații pe fișiere.

### Se poate opri?

Da, partea automată. Setări, Actualizări are caseta „Verifică automat actualizările”, bifată din oficiu. Debifată, Valet nu mai face nicio verificare automată și nu mai arată notificări de versiune nouă. Butonul „Verifică acum” merge oricum. Setarea e câmpul `auto_update_check` din `app_settings.json` (`commands/settings.rs:373`, valoarea implicită `true` la linia 387; testul `commands::settings::tests::auto_update_check_defaults_to_on_and_is_saved`). Bucla de actualizare o citește din nou în fiecare oră, deci nu e nevoie de repornire (`updater.rs:352`, `auto_check_wanted` la linia 362; testul `updater::tests::automatic_checks_follow_the_setting`). Caseta nu influențează verificarea licenței, care nu o citește (`licensing.rs` și `license_client.rs` nu o pomenesc). Blocarea `Valet.exe` în firewall oprește și ea verificările (eșecul se scrie în jurnal ca avertisment, iar Valet merge mai departe); asta nu a fost testată.

Nici build-ul din Microsoft Store (MSIX) nu rulează cod de actualizare: când Valet detectează că rulează ca aplicație împachetată, nu pornește verificarea automată, iar comenzile „Verifică acum” și „Instalează și repornește” răspund că actualizările vin din Store (`setup.rs:103` și `116-122`, `updater.rs:253`). Setări, Actualizări arată atunci doar versiunea instalată și „Actualizările vin din Microsoft Store.” (`SettingsView.tsx:492`). Fluxul de lucru pentru Store verifică și că pachetul nu are configurare de actualizator (`packaging/msix/README.md`, secțiunea 2). Versiunea din Store nu e publicată și nu a fost rulată dintr-o instalare din Store.

Un build făcut fără cheile actualizatorului nu rulează deloc cod de actualizare: nu există verificare automată, iar „Verifică acum” răspunde că actualizările nu sunt configurate. Build-urile făcute cu `pnpm tauri dev` sunt de acest fel. Fluxul de release construiește cu chei doar dacă proprietarul depozitului le-a setat (`docs/RELEASE.md`, secțiunea 0.2); rularea de probă de la commitul `1d910c9` a construit fără ele (jurnalul ei spune „updater artifacts off”).

### Verificarea licenței

Această subsecțiune a fost verificată cu codul de la `fc6acc0`. Contractul după care e făcută e `docs/LICENSING.md`, „Online check”.

**Când nu e activă.** Adresa serverului și cheia publică vin din două fișiere din depozit, `crates/valet-core/license/api.txt` și `pubkey.txt`, încorporate în program la compilare (`crates/valet-core/src/license.rs:30-37`). Când unul e gol sau lipsește, licențele sunt „neconfigurate”: programul nu construiește niciun client HTTP și nu face nicio cerere, o licență Pro nu se poate activa, iar proba și planul Gratuit merg ca în descrierile lor (`licensing.rs:130-138`, `license_client.rs:114-122`). Teste: `license_client::tests::unconfigured_licensing_builds_no_client` (nu se construiește niciun client, cu fișierele livrate și cu fiecare jumătate lipsă) și `licensing::tests::unconfigured_flows_make_no_request`. `wc -c` a afișat 0 pentru `api.txt`, `pubkey.txt` și `checkout.txt` la `fc6acc0`, deci un build făcut din acest depozit nu face nicio cerere de licență. Un build de release are verificare de licență doar dacă proprietarul depozitului a completat fișierele înainte de build.

**Când rulează** (într-un build cu licențele configurate). Doar când imporți o licență Pro, și apoi doar pentru o licență păstrată. Proba (14 zile) și planul Gratuit nu fac nicio cerere de licență: începutul probei e o dată păstrată în baza de date de pe calculatorul tău (`valet.sqlite3`, cheia `trial_started_at`), și nimic din ea nu merge prin rețea. Cu o licență Pro, cererile sunt:

- la activare, când imporți fișierul licenței sau textul lipit (`POST /v1/activate`, `licensing.rs:470-488`);
- apoi cel mult o dată la 30 de zile: planul se calculează la pornire și din oră în oră, iar o cerere (`POST /v1/validate`) pleacă doar când ultima verificare reușită are 30 de zile sau mai mult, și cel mult o dată la 24 de ore (`licensing.rs:51-53`, `570-620`; constanta din nucleu `ONLINE_CHECK_AFTER` în `crates/valet-core/src/license.rs`; testul `licensing::tests::validation_runs_when_needed_at_most_daily`);
- când apeși „Verifică licența acum” (aceeași cerere de validare, pe loc);
- când scoți licența din Setări (`POST /v1/deactivate`, vezi mai jos).

**Ce se trimite.** Corpul cererilor de activare și de validare are exact trei câmpuri (`license_client.rs:75-79`, trimise la liniile 211-216):

- `license_id`: identificatorul licenței, un cod aleatoriu (UUID versiunea 4) scris în fișierul tău de licență;
- `machine`: un identificator al calculatorului, primele 32 de caractere hexazecimale ale hash-ului blake3 al textului `valet-machine-v1:` urmat de `MachineGuid` din Windows (citit din `HKLM\SOFTWARE\Microsoft\Cryptography`; `crates/valet-platform/src/machine.rs`). E un hash într-un singur sens: serverul poate compara doi identificatori, dar nu poate obține GUID-ul din unul;
- `app_version`: versiunea Valet.

Cererea de dezactivare are doar `license_id` și `machine` (`license_client.rs:82-85`). Teste: `license_client::tests::validate_sends_only_the_three_fields_and_returns_the_token` și `deactivate_sends_license_and_machine_only` compară întregul corp JSON cu câmpurile așteptate. Cererile merg la adresa HTTPS din `api.txt`, cu un timp limită de 10 secunde (`license_client.rs:22`), doar HTTPS (o adresă `http://` simplă e refuzată: `production_client_refuses_plain_http`) și fără redirecționări (`license_client.rs:143-146`). Serverul răspunde cu un token semnat, păstrat lângă fișierul licenței, sau cu un refuz (licență necunoscută, prea multe activări, revocată). Ca orice server web, vede și adresa ta IP publică și ora cererii. Pentru fiecare identificator de licență serverul trebuie să țină minte ce identificatori de calculator au activat-o, pentru că așa numără activările (cel mult 3 pe licență).

**Ce nu se trimite niciodată.** Niciun nume de fișier, nicio cale de folder, nicio regulă, niciun conținut de regulă și niciun număr de reguli sau de fișiere. Nici `MachineGuid` în forma brută, nici numele calculatorului, nici versiunea de Windows, nici adresa de email din fișierul licenței. Emailul rămâne pe calculatorul tău și e afișat în Setări. (E cunoscut celui care ți-a vândut licența, din cumpărare; Valet nu îl trimite.)

**Unde e păstrată licența.** În folderul care are `valet.sqlite3` și `config.json`: `app_config_dir` din Tauri (`state.rs:174-178`; calea bazei de date se construiește din el la `setup.rs:56`). Pe Windows asta e `%APPDATA%\ro.valetapp.desktop`, folderul din secțiunea 2; asta a fost citit din Tauri și nu a fost verificat pe o instalare. Fișierele:

- `license.valet-license`: fișierul licenței, un fișier JSON UTF-8 cu semnătură (`licensing.rs:31`);
- `activation.valet-token`: ultimul răspuns semnat al serverului (`licensing.rs:32`);
- `license-removed\`: un subfolder, creat când o licență sau un token e scos sau înlocuit (`licensing.rs:33`).

Ambele fișiere se scriu întâi ca fișier temporar în același folder, apoi se redenumesc la locul lor; fișierele mai mari de 64 KB nu se citesc (`licensing.rs:48`, folosit la linia 153).

**Ce face scoaterea.** În Setări, „Scoate licența de pe acest calculator” (după o confirmare) face două lucruri (`licensing.rs:519-541`; testul `licensing::tests::remove_deactivates_then_moves_both_files`):

1. Trimite cererea de dezactivare, care eliberează activarea acelui calculator pe server, ca licența să poată fi folosită pe alt calculator. E făcută în limita posibilului: dacă cererea eșuează (de exemplu nu există conexiune), eșecul se scrie în jurnal ca avertisment, iar scoaterea continuă. În acel caz serverul numără în continuare acest calculator până când activarea e eliberată altfel (de exemplu de asistență, care poate goli de pe server calculatoarele active ale unei licențe).
2. Mută `license.valet-license` și `activation.valet-token` în `license-removed\<timp unix>-<nume fișier>` (se adaugă un număr dacă numele există). Nu le șterge niciodată și nu înlocuiește niciun fișier de acolo (`licensing.rs:189-213`; testul `licensing::tests::files_are_moved_never_replaced`). Importul unei licențe cât timp alta e păstrată mută la fel fișierele vechi.

Dacă fișierul licenței nu poate fi verificat (deci nu se poate citi un identificator de licență), nu se trimite nicio cerere de dezactivare, iar fișierele se mută oricum.

Dezinstalarea nu face nimic din toate acestea: dezinstalatorul nu face niciun apel de rețea, deci scoate întâi licența din Setări.

**Ce se întâmplă fără conexiune.** O eroare de rețea păstrează tokenul (`licensing.rs:495-515`). Un token a cărui ultimă verificare reușită are cel mult 60 de zile dă în continuare Pro; mai vechi de atât, Valet merge ca Gratuit până la următoarea verificare reușită, și nu se pierde nimic (`crates/valet-core/tests/license.rs`: `token_31_days_old_is_still_pro_and_asks_for_a_check`, `token_61_days_old_is_free_and_expired`). Dacă serverul răspunde că licența e necunoscută sau revocată (sau, la o nouă verificare, că sunt prea multe activări), Valet mută tokenul în `license-removed\` și merge ca Probă sau Gratuit. Fișierul licenței rămâne, ca Setările să poată arăta de ce (testul `licensing::tests::validate_maps_answers_and_moves_the_token_on_refusal`).

### Alte conexiuni

- Instalatorul descarcă programul de pornire Microsoft WebView2, și WebView2 însuși, doar dacă WebView2 nu e instalat pe calculator (`downloadBootstrapper` în `tauri.conf.json`; conform `docs/RELEASE.md`, secțiunea 6). Aici vorbește instalatorul cu Microsoft, nu Valet.
- WebView2 e o componentă Microsoft cu propriul serviciu de actualizare. Ce contactează ține de alt cod decât cel Valet și nu e tratat aici.
- Nu s-a făcut nicio captură de trafic cât a rulat Valet, deci ce fac Tauri și pluginurile lui în afară de cele de mai sus e citit din cod, nu observat.

### Cumpărarea unei licențe

**Stare.** Cumpărarea nu e deschisă. Vânzarea se deschide abia după ce webhook-ul comerciantului ales a trecut o cumpărare și o rambursare de probă (sandbox) cap-coadă (`docs/LICENSING.md`, „License service”). Această subsecțiune descrie ce e hotărât pentru acel moment. Vine din contractul respectiv, nu din cod care a rulat: comerciantul nu e ales (Paddle sau Stripe Managed Payments), serverul nu e instalat și niciun email nu a fost trimis prin Resend.

- **Comerciantul** tratează plata, TVA-ul și rambursările. Plata se face pe pagina comerciantului, deschisă în browser (butonul „Cumpără Pro” din aplicație deschide adresa din `checkout.txt` și nimic altceva; aplicația nu are formular de plată). Comerciantul primește datele de plată și adresa de email a cumpărătorului. Ce păstrează el ține de politica lui, care nu e tratată aici.
- **Serverul de licențe** rulează pe Fly.io, într-o regiune aleasă de Vlad (nu e aleasă încă; `services/license/fly.toml` nu există încă). Primește de la comerciant o notificare semnată (webhook) pentru cumpărare și pentru rambursare. Semnătura se verifică pe corpul brut, înaintea oricărui alt pas. Serverul păstrează: identificatorul licenței, identificatorii comenzii la comerciant, identificatorii hash ai calculatoarelor care au activat și datele acestor activări. Nu păstrează adresa de email a cumpărătorului (`docs/LICENSING.md`: „The store keeps merchant ids and the license id, never the email”).
- **Emailul** se folosește o singură dată. Serverul îl primește de la comerciant la cumpărare (la Paddle îl citește cu un apel la API-ul de clienți al Paddle), îl pune în fișierul de licență semnat și trimite mesajul cu licența atașată prin Resend. Resend procesează adresa ca să livreze mesajul; ce păstrează Resend ține de politica lui. Apoi emailul există doar în acel mesaj și în fișierul de licență de pe calculatorul cumpărătorului. Un email care a eșuat se retrimite când comerciantul retrimite evenimentul, fără să se emită o a doua licență.
- **Rambursarea** se face în 14 zile de la cumpărare, prin comerciant. Evenimentul de rambursare al comerciantului face ca serverul să revoce licența. Aplicația află la următoarea verificare a licenței (cel mult o dată la 30 de zile, sau pe loc cu „Verifică licența acum”): serverul răspunde 410, Valet mută tokenul în `license-removed\` și merge ca Probă sau Gratuit. Fișierul licenței rămâne pe calculator.
- **Adrese IP.** Serverul limitează cererile pe adresă IP a clientului în memoria procesului; serverul nu le scrie în jurnal și nu le păstrează. Fly.io, ca gazdă, vede ce vede orice gazdă.
- **Neschimbat:** aplicația nu trimite adresa de email, nume de fișiere, căi sau conținut, iar cererile ei sunt cele listate mai sus.

## 2. Ce păstrează Valet pe calculatorul tău

### În `%APPDATA%\ro.valetapp.desktop`

| Fișier | Conținut |
|---|---|
| `valet.sqlite3` (cu fișierele `-wal` și `-shm` cât e deschis) | Regulile și jurnalul de operații. Jurnalul păstrează calea completă a sursei și a destinației fiecărei operații, mărimile și sumele de control (blake3) ale fișierelor mutate, copiate sau trimise la Coș, starea fiecărei operații și textele de eroare. Nu se curăță niciodată: nimic nu șterge rânduri, deci crește cu fiecare lot. Ecranul Istoric îl citește. |
| `config.json` | Folderele urmărite. |
| `app_settings.json` | Limba, notificarea pornită sau oprită, pornirea cu Windows, organizarea automată pornită sau oprită, dacă ai terminat ghidul de prima pornire, dacă ai aprobat o previzualizare, canalul de actualizare, dacă se verifică automat actualizările și comutatorul jurnalului detaliat. |
| `approvals.json` | Pentru fiecare regulă, versiunea aprobată și folderele urmărite ale previzualizării care a aprobat-o. |
| `license.valet-license`, `activation.valet-token` | Doar după ce imporți o licență Pro: fișierul de licență semnat (are identificatorul licenței, emailul pe care s-a vândut, numărul de activări permise și data până la care actualizările sunt incluse) și ultimul răspuns semnat al serverului. Vezi „Verificarea licenței”. |
| `license-removed\` | Doar după ce o licență a fost scoasă sau înlocuită: fișierele vechi de licență și de token, mutate aici și niciodată șterse. |

Data la care a început proba de 14 zile e un rând în baza de date (`meta`, cheia `trial_started_at`), nu un fișier separat.

În build-ul din Microsoft Store (MSIX), Windows redirecționează fișierele noi din AppData către un folder al pachetului, `%LOCALAPPDATA%\Packages\<PackageFamilyName>\LocalCache\Roaming\...` și `...\Local\...` (`packaging/msix/README.md`, secțiunea 5; neverificat într-o instalare din Store). Dezinstalarea aplicației din Store șterge acel folder cu tot ce e în el: regulile, setările, fișierele de licență și jurnalul, deci Istoricul și anularea dispar. Acolo nu există alegerea „Șterge datele aplicației”.

Înainte de această versiune, baza de date avea și un tabel cu statistici de operații (`operation_history`). Migrarea `0005_drop_operation_history.sql` îl șterge. Jurnalul e singurul istoric. Testul `storage::tests::migration_0005_drops_operation_history_and_keeps_the_journal` verifică faptul că tabelul dispare și rândurile jurnalului rămân.

Pornirea cu Windows, dacă o activezi, scrie valoarea `Valet` în `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (doar utilizatorul curent).

Fișierele pe care Valet le trimite la Coșul de reciclare sunt în Coșul de reciclare Windows, pe care îl administrează Windows.

### În `%LOCALAPPDATA%\ro.valetapp.desktop\logs`

Fișierul de jurnal e `valet.log`. Locul vine din `app_log_dir` al Tauri și din ținta `LogDir` a pluginului de jurnal (citit din sursa lor; neverificat pe o instalare Windows).

- Rotire: un fișier se închide la 2 MB și se redenumește `valet_<data și ora>.log`. Se păstrează cel mult 5 fișiere, cel activ și 4 rotite, deci jurnalele ocupă cel mult aproximativ 10 MB. (`logging::tests::log_settings_values`.)
- Nivel implicit: INFO. Liniile INFO din codul Valet conțin numere, id-uri de lot, numere de versiune și text fix, de exemplu „Dry-run completed: 12 operation(s) proposed, 3 skipped, across 2 path(s)”. Nu conțin căi de fișiere. Asta s-a citit din apelurile de jurnal din `apps/desktop/src-tauri/src`; niciun test nu verifică liniile de jurnal, iar valorile de eroare pe care le afișează unele linii WARN și ERROR (erori de sistem de operare și de bază de date) nu au fost verificate pentru căi.
- Jurnal detaliat: Setări, Asistență, „Jurnal detaliat (pentru depanare)”. Trece nivelul pe DEBUG pe loc, fără repornire. Liniile DEBUG pot conține căi de fișiere: textul erorii watcher-ului și textul notificărilor sunt exemple din codul Valet, iar bibliotecile folosite de Valet scriu și ele linii DEBUG. Stratul SQL și stiva HTTP, care scriu multe linii, rămân pe INFO. Opțiunea e oprită din oficiu și rămâne pe acest calculator.
- O panică (o eroare de program care oprește un fir de execuție) se scrie în jurnal ca `Panic at <fișier sursă>:<linie>: <mesaj>`. Căile absolute din mesaj se înlocuiesc cu `<path>` (`logging::tests::panic_hook_logs_message_and_location`, `scrub_replaces_absolute_paths`).
- Valet nu trimite jurnalele nicăieri. Dacă trimiți un jurnal cuiva pentru asistență, citește-l mai întâi, mai ales unul scris cu jurnalul detaliat pornit.

## 3. Ce nu are Valet

- **Nicio telemetrie.** Nimic nu numără ce faci, și nu există nicio bibliotecă de analiză în `Cargo.toml` sau `package.json`.
- **Niciun raport de erori.** Sentry nu e integrat. O cădere sau o panică lasă o linie în jurnalul local și nimic altceva. Dacă rapoartele de erori se adaugă mai târziu, vor fi opt-in (oprite până le pornești), un ecran numit „Ce trimitem” va arăta exact ce se trimite, iar căile vor fi scoase înainte de trimitere (PLAN 4.5). Nimic din toate acestea nu există încă.
- **Nicio verificare de licență, dacă nu imporți o licență Pro.** Gratuit și proba nu contactează niciodată un server de licențe. Cu o licență importată, verificarea e cea descrisă în secțiunea 1, și doar într-un build care are un server de licențe configurat. Build-urile de acum nu au niciunul.
- **Niciun cont, nicio reclamă, nicio copie în cloud** a regulilor, jurnalului sau fișierelor tale.

Nimic nu împiedică o versiune viitoare să adauge un apel de rețea, și niciun test nu pică dacă se adaugă unul. Lucrurile din cod care fixează comportamentul actualizatorului sunt `updater::tests::endpoints_match_the_release_config_script` (cele două adrese de mai sus sunt cele pe care fluxul de release le scrie în build) și `updater::tests::not_configured_without_updater_section` (`tauri.conf.json` nu are secțiune de actualizator; build-urile de release o adaugă). Testele care fixează cererile de licență sunt cele numite în secțiunea 1, „Verificarea licenței”: câmpurile corpului, cazul neconfigurat fără client construit și cele două operații cu fișiere ale scoaterii. Niciun test nu listează toate apelurile de rețea ale programului.

## 4. Dezinstalarea

Instalatorul e per utilizator și folosește șablonul NSIS al Tauri. La dezinstalare:

- Datele tale se păstrează din oficiu. Pagina dezinstalatorului are o casetă „Șterge datele aplicației” (în instalatorul în engleză, „Delete the application data”), nebifată la început.
- Cu caseta bifată, dezinstalatorul șterge `%APPDATA%\ro.valetapp.desktop` (setările, regulile, aprobările și jurnalul, deci Istoricul și anularea dispar) și, de asemenea, `%LOCALAPPDATA%\ro.valetapp.desktop` (jurnalele). Asta s-a citit din șablonul `tauri-bundler` 2.6.1 și 2.10.4 (`RmDir /r` pe ambele foldere); nu s-a rulat nicio dezinstalare pe Windows.
- Intrarea de pornire cu Windows se șterge la dezinstalare, indiferent dacă caseta e bifată (același șablon).
- Cu caseta nebifată, o instalare ulterioară găsește vechiul jurnal și vechile setări și merge mai departe. Testele de upgrade de mai jos acoperă asta pentru baze de date ale versiunilor anterioare.
- Fișierele de licență sunt în același folder, deci se păstrează din oficiu și se șterg cu caseta bifată. Dezinstalatorul nu contactează serverul de licențe și nu eliberează activarea; dacă vrei activarea înapoi, șterge licența din Setări înainte de dezinstalare.
- Versiunea din Microsoft Store nu are pagină de dezinstalare: scoaterea aplicației din Windows șterge folderul de date al pachetului (secțiunea 2). Scoate întâi licența din Setări dacă vrei activarea înapoi și anulează înainte de dezinstalare orice lot vrei să fie anulat.
- Fișierele pe care Valet le-a mutat sau copiat rămân unde sunt. Fișierele din Coșul de reciclare rămân acolo. Dezinstalarea nu anulează nimic.

Ca să ștergi tot cu mâna, șterge cele două foldere de mai sus.

## 5. Cum verifici singur

Din rădăcina depozitului:

```
grep -rn "releases/latest/download\|updater-channel" apps/desktop/src-tauri/src/updater.rs
cargo tree -p app -i reqwest
cargo test -p app updater::
cargo test -p app logging::
cargo test -p valet-core --test upgrade_journal
wc -c crates/valet-core/license/api.txt crates/valet-core/license/pubkey.txt crates/valet-core/license/checkout.txt
cargo test -p app -- license_client:: licensing:: updater::
```

Prima arată cele două adrese. A doua arată ce crate folosește biblioteca HTTP. Testele sunt cele numite mai sus. `wc -c` afișează mărimea fișierelor de configurare a licenței; 0 pentru `api.txt` și `pubkey.txt` înseamnă că build-ul nu are verificare de licență (a afișat 0 pentru toate trei la `fc6acc0`). Ultima comandă rulează testele clientului de licențe, ale licențelor și ale actualizatorului (toate au trecut la `fc6acc0`).
