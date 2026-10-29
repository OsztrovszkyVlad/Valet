# Confidențialitate

English version: [PRIVACY.md](PRIVACY.md)

Pagina spune ce trimite Valet prin rețea, ce păstrează pe calculatorul tău, ce nu are și ce se întâmplă cu datele la dezinstalare. Descrie codul de la commitul `1d910c9`. Unde ceva a fost citit din cod sau dintr-o dependență și nu a fost observat pe o instalare Windows care rulează, pagina spune asta.

Pe scurt: Valet face un singur fel de cerere de rețea, verificarea unei versiuni noi a lui însuși. Nicio denumire de fișier, cale sau conținut de fișier nu face parte din ea. Restul rămâne pe calculatorul tău.

## 1. Ce merge prin rețea

Codul Valet deschide o conexiune într-un singur loc: actualizatorul (`apps/desktop/src-tauri/src/updater.rs`). Singura bibliotecă de rețea din arborele de dependențe, `reqwest`, vine prin `tauri-plugin-updater` și altfel nu apare (`cargo tree -p app -i reqwest` afișează doar acea cale, verificat pentru gazda Linux și pentru `--target x86_64-pc-windows-msvc`).

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

- La 30 de secunde după pornirea Valet, apoi o dată la 24 de ore cât timp rămâne pornit. Codul se uită la ceas din oră în oră și verifică dacă au trecut 24 de ore, deci timpul petrecut în repaus se numără. Așadar fiecare pornire a Valet, într-un build care are actualizări, face o verificare automată.
- Când apeși „Verifică acum” în Setări, Actualizări.
- Când apeși „Instalează și repornește”, Valet descarcă instalatorul.

O verificare automată nu instalează nimic. Doar arată o notificare (o dată pe versiune la fiecare rulare a Valet) și un semn pe butonul Setări. Instalarea cere un click și o confirmare. Instalarea așteaptă cât timp rulează un lot de operații pe fișiere.

### Se poate opri?

Nu din Setări: nu există o opțiune care să oprească verificarea automată. Se poate schimba canalul. Blocarea `Valet.exe` în firewall oprește verificările (eșecul se scrie în jurnal ca avertisment, iar Valet merge mai departe); asta nu a fost testată.

Un build făcut fără cheile actualizatorului nu rulează deloc cod de actualizare: nu există verificare automată, iar „Verifică acum” răspunde că actualizările nu sunt configurate. Build-urile făcute cu `pnpm tauri dev` sunt de acest fel. Fluxul de release construiește cu chei doar dacă proprietarul depozitului le-a setat (`docs/RELEASE.md`, secțiunea 0.2); rularea de probă de la commitul `1d910c9` a construit fără ele (jurnalul ei spune „updater artifacts off”).

### Alte conexiuni

- Instalatorul descarcă programul de pornire Microsoft WebView2, și WebView2 însuși, doar dacă WebView2 nu e instalat pe calculator (`downloadBootstrapper` în `tauri.conf.json`; conform `docs/RELEASE.md`, secțiunea 6). Aici vorbește instalatorul cu Microsoft, nu Valet.
- WebView2 e o componentă Microsoft cu propriul serviciu de actualizare. Ce contactează ține de alt cod decât cel Valet și nu e tratat aici.
- Nu s-a făcut nicio captură de trafic cât a rulat Valet, deci ce fac Tauri și pluginurile lui în afară de cele de mai sus e citit din cod, nu observat.

## 2. Ce păstrează Valet pe calculatorul tău

### În `%APPDATA%\ro.valetapp.desktop`

| Fișier | Conținut |
|---|---|
| `valet.sqlite3` (cu fișierele `-wal` și `-shm` cât e deschis) | Regulile și jurnalul de operații. Jurnalul păstrează calea completă a sursei și a destinației fiecărei operații, mărimile și sumele de control (blake3) ale fișierelor mutate, copiate sau trimise la Coș, starea fiecărei operații și textele de eroare. Nu se curăță niciodată: nimic nu șterge rânduri, deci crește cu fiecare lot. Ecranul Istoric îl citește. |
| `config.json` | Folderele urmărite. |
| `app_settings.json` | Limba, notificarea pornită sau oprită, pornirea cu Windows, organizarea automată pornită sau oprită, dacă ai terminat ghidul de prima pornire, dacă ai aprobat o previzualizare, canalul de actualizare și comutatorul jurnalului detaliat. |
| `approvals.json` | Pentru fiecare regulă, versiunea aprobată și folderele urmărite ale previzualizării care a aprobat-o. |

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
- **Nicio verificare de licență.** În această versiune nu există. Planul permite una mai târziu (PLAN 2.6, punctul 7); când se adaugă, pagina se schimbă.
- **Niciun cont, nicio reclamă, nicio copie în cloud** a regulilor, jurnalului sau fișierelor tale.

Nimic nu împiedică o versiune viitoare să adauge un apel de rețea, și niciun test nu pică dacă se adaugă unul. Cele două lucruri din cod care fixează comportamentul de acum sunt `updater::tests::endpoints_match_the_release_config_script` (cele două adrese de mai sus sunt cele pe care fluxul de release le scrie în build) și `updater::tests::not_configured_without_updater_section` (`tauri.conf.json` nu are secțiune de actualizator; build-urile de release o adaugă).

## 4. Dezinstalarea

Instalatorul e per utilizator și folosește șablonul NSIS al Tauri. La dezinstalare:

- Datele tale se păstrează din oficiu. Pagina dezinstalatorului are o casetă „Șterge datele aplicației” (în instalatorul în engleză, „Delete the application data”), nebifată la început.
- Cu caseta bifată, dezinstalatorul șterge `%APPDATA%\ro.valetapp.desktop` (setările, regulile, aprobările și jurnalul, deci Istoricul și anularea dispar) și, de asemenea, `%LOCALAPPDATA%\ro.valetapp.desktop` (jurnalele). Asta s-a citit din șablonul `tauri-bundler` 2.6.1 și 2.10.4 (`RmDir /r` pe ambele foldere); nu s-a rulat nicio dezinstalare pe Windows.
- Intrarea de pornire cu Windows se șterge la dezinstalare, indiferent dacă caseta e bifată (același șablon).
- Cu caseta nebifată, o instalare ulterioară găsește vechiul jurnal și vechile setări și merge mai departe. Testele de upgrade de mai jos acoperă asta pentru baze de date ale versiunilor anterioare.
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
```

Prima arată cele două adrese. A doua arată ce crate folosește biblioteca HTTP. Testele sunt cele numite mai sus.
