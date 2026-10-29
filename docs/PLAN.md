# Valet. Planul de execuție

Versiunea 1, 27 octombrie 2026. Acest fișier este sursa de adevăr pentru Claude Code (managerul de proiect) și pentru subagenți. Se modifică doar de Vlad sau cu aprobarea lui explicită.

Roluri: Vlad = proprietar, decide și testează manual. Sesiunea principală Claude Code = manager de proiect (PM): citește planul, împarte munca pe subagenți, verifică, scrie raportul de fază, se oprește. Subagenții = executanți pe arii delimitate.

---

## 0. Rezumat executiv

**Teza.** Pe Windows nu există un organizator de fișiere pe bază de reguli care să fie în același timp întreținut, sigur (undo pentru orice) și plăcut de folosit. Hazel face asta pe Mac de 15 ani, pe bani, cu o echipă de câțiva oameni. Pe Windows piața e fragmentată între un proiect open-source îmbătrânit (DropIt), un produs plătit stagnant (File Juggler) și o duzină de aplicații mici, apărute în ultimii doi ani, din care multe sunt „AI organizers” care mută fișiere pe baza unei ghiciri. Valet intră exact în golul ăsta: determinist, local, cu previzualizare și anulare, întreținut, cu interfață în română și engleză.

**Deciziile luate (CEO), verificate contra dovezilor:**

| # | Decizie | Motiv scurt |
|---|---|---|
| 1 | v1 = „Hazel pentru Windows”. Nimic altceva până nu e stabil. | Fiabilitatea e produsul. Un fișier mutat greșit o dată = dezinstalare. |
| 2 | Partițiile: nu în primii doi ani. Analiza spațiului pe disc: da, Faza 5. | Partițiile pierd date ireversibil, cer drepturi de administrator și cod de nivel jos; categoria e ocupată de MiniTool/EaseUS/AOMEI. „Ce îmi mănâncă discul” e o nevoie uriașă, sigură și se potrivește cu organizarea. |
| 3 | Fără reclame. Freemium cu Pro plătit o singură dată. | Reclamele într-o aplicație care umblă prin fișierele omului distrug încrederea; CCleaner și Avast sunt lecția. Un utilizator plătitor valorează mai mult decât mii de afișări. |
| 4 | Totul rulează local. Niciun fișier, nume de fișier sau conținut nu părăsește calculatorul. Telemetrie doar opt-in, fără căi de fișiere. | E argumentul de vânzare împotriva Microsoft și a organizatoarelor „AI” din cloud. |
| 5 | Nucleul (valet-core) rămâne open-source MIT. Pro e în același binar, deblocat prin licență semnată. | Deschide semnarea gratuită sau ieftină pentru open-source (SignPath, Certum), încredere, contribuții. Tehnicienii pot ocoli licența; e un cost acceptabil pentru un solo dev. |
| 6 | AI abia în Faza 6, local, și numai ca generator de reguli și de condiții pe conținut, niciodată ca executant direct. | Vezi secțiunea 2.6. |
| 7 | Doar Windows în 2026–2027. Nucleul rămâne fără cod de sistem de operare; tot ce e Windows stă în valet-platform și src-tauri. | macOS vine după ce v1 face bani. |

**Cum arată succesul (ținte, nu promisiuni):**

| Orizont | Ce există | Cifre țintă |
|---|---|---|
| 3 luni (ianuarie 2027) | v1 beta publică pe GitHub Releases + winget; licențe funcționale | 300 de instalări, 20 de plătitori, zero rapoarte de fișiere pierdute |
| 6 luni (aprilie 2027) | listare Microsoft Store, modul de spațiu pe disc, recenzii | 2.500 de instalări cumulate, conversie 2–3% din utilizatorii activi, ~1.500 € venit cumulat |
| 12 luni (octombrie 2027) | AI local pentru reguli, SEO pe „hazel for windows”, comunitate mică | 15.000 de instalări, 400 de plătitori, ~10.000 € cumulat |

Spus direct: anul unu nu plătește chiria. Plătește dovada că produsul ține, și fiecare lună după aceea e mai bună decât precedenta, pentru că nu e muncă la oră. Dacă la 6 luni conversia e sub 1% și nu există nicio recenzie organică, planul se revizuiește.

---

## 1. Piața și competitorii

### 1.1 Harta

| Produs | Platformă | Model | Stare în 2026 | Ce fac bine | Ce reproșează oamenii |
|---|---|---|---|---|---|
| Hazel (Noodlesoft) | Mac | plată unică, ~42 $ | activ, matur | reguli flexibile, fiabilitate, rulează în fundal fără să-l simți | nu există pe Windows; curba de învățare a regulilor; preț pentru upgrade-uri majore |
| File Juggler | Windows | plătit | stagnant, comunitate mică | același concept ca Hazel | interfață veche, dezvoltare lentă |
| DropIt | Windows | gratuit, open-source | vechi, UI de alt deceniu | gratuit, portabil, multe acțiuni | nu e „pune și uită”, trebuie să arunci fișiere pe el; UI învechit |
| organize (CLI, Python) | toate | open-source | activ | pentru tehnicieni, configurare YAML | fără interfață, nu pentru utilizatori obișnuiți |
| Dolfer, FilePipe, Floxtop, Neatify, Mouzi, SortSage | Win/Mac | plătite sau gratuite | apărute recent, mici | arată că cererea există | nesigure, abandonate repede, fără undo serios |
| Sparkle, Sortio, Recall Local, „AI File Sorter” | Mac/Win | abonament sau gratuit | val 2024–2026 | promit „zero configurare” | mută fișiere pe baza unei ghiciri; unele trimit date în cloud; nu poți prezice unde ajunge un fișier |
| PowerToys (PowerRename, File Locksmith) | Windows | gratuit, Microsoft | activ | redenumire în masă, cine ține fișierul blocat | nu organizează, nu rulează în fundal |
| Everything (voidtools) | Windows | gratuit | activ, iubit | căutare instantanee | nu organizează; e un model de produs mic și perfect |
| WizTree / TreeSize / WinDirStat / SpaceSniffer | Windows | gratuit personal sau ~10–40 $ | active | „ce îmi mănâncă discul” | WinDirStat lent; TreeSize scump pentru uz personal; niciunul nu organizează |
| CCleaner | Windows | freemium cu reclame și upsell | activ, reputație afectată | curățare | reclame, bundling, telemetria din 2017–2018 după preluarea de Avast; e exemplul a ce nu facem |
| Microsoft, AI actions în File Explorer | Windows 11 | gratuit | lansate în 26H2 (sept. 2026), parțial retrase în nov. 2026 | editare de imagini, rezumat de documente, legate de OneDrive și M365 | nu sunt disponibile în Spațiul Economic European; nu organizează nimic; Microsoft le reduce din cauza reacțiilor la „prea mult Copilot” |

Surse: pagina Microsoft Learn despre opțiunile de semnare, AlternativeTo (paginile Hazel, File Juggler, DropIt), anunțurile Windows Insider pentru AI actions și știrile din octombrie 2026 despre retragerea a opt acțiuni Copilot din File Explorer, documentația Paddle și Lemon Squeezy. Cifrele de preț ale competitorilor sunt din memorie și trebuie verificate de subagentul researcher în Faza 2 înainte de a fixa prețul final.

### 1.2 Ce vor oamenii (sinteză din recenzii și fire de discuție)

Iubesc: „pune și uită” (Hazel), previzualizare înainte de acțiune, reguli pe care le înțeleg, aplicații care nu consumă resurse în fundal, plată unică.

Urăsc: fișiere mutate unde nu se așteaptă, lipsa undo-ului, reguli greu de scris, aplicații care rup OneDrive (fișiere „la cerere” descărcate forțat sau mutate pe jumătate), abonamente pentru utilități mici, reclame și upsell agresiv, telemetrie, instalatoare cu bundling.

Cer cel mai des: Hazel pe Windows (apare constant în discuțiile despre alternative), undo, reguli pe dată („mai vechi de 30 de zile”), reguli pe conținut (facturi), curățarea Downloads și a Desktop-ului.

### 1.3 Poziționare în o frază

Valet pune ordine în fișierele tale, pe calculatorul tău, după reguli pe care le vezi și le poți anula. Nu un chat. Nu un cloud. Nu o ghicire.

Împotriva Microsoft: ei fac acțiuni pe un fișier (rezumă, editează), legate de OneDrive și de abonamentul M365, indisponibile în UE. Valet face organizare continuă, deterministă, locală, pe toate folderele, cu istoric și undo. Nu ne batem pe același teren.

---

## 2. Definiția produsului v1

### 2.1 Utilizatorul țintă

Persoana care are 3.000 de fișiere în Downloads, un Desktop plin și un sentiment de vinovăție. Nu e programator. Vrea să instaleze ceva o dată, să bifeze două lucruri și să nu se mai gândească. A doua țintă: tehnicienii care cunosc Hazel și vor echivalentul pe Windows.

### 2.2 Sarcinile pe care le rezolvă (jobs to be done)

1. „Ține-mi Downloads curat fără să fac nimic.”
2. „Pune capturile de ecran, facturile și instalatoarele la locul lor.”
3. „Arată-mi ce ai de gând să muți înainte să muți.”
4. „Dacă ai greșit, adu totul înapoi cu un click.”
5. (Faza 5) „Spune-mi ce îmi ocupă discul și ce pot șterge fără risc.”

### 2.3 Funcții v1, lista fixă

**IN:**
- Reguli: condiții pe extensie, grup de tip (imagine, document, arhivă, instalator, video, audio, cod), nume (regex și „conține”), mărime, cale, vechime (modificat/creat mai vechi de N zile), folder gol, duplicat după hash (doar raport în v1).
- Acțiuni: mută, copiază, trimite la Coșul de reciclare (niciodată ștergere definitivă), raportează fără acțiune. Redenumirea cu șablon e prima funcție după lansare (backlog); etichetarea a ieșit din plan (decizia lui Vlad, 29 octombrie 2026).
- Previzualizare (dry run) obligatorie la prima rulare și la orice regulă nouă.
- Jurnal de operații și anulare: ultimul lot, orice lot din istoric, orice fișier individual.
- Foldere urmărite (watcher) cu aplicare automată, pornit abia după ce utilizatorul a aprobat cel puțin o previzualizare.
- 8 reguli predefinite care merg din prima (2.4).
- Onboarding în 3 pași: alege foldere (Downloads și Desktop prebifate), alege presetări, vezi previzualizarea.
- Tray, autostart opțional, notificare scurtă după un lot.
- Interfață în română, cu engleză ca a doua limbă, comutabilă.
- Instalator per utilizator (fără drepturi de administrator), updater semnat, raportare de erori opt-in.

**OUT (explicit, ca să nu intre „pe furiș”):** etichete (tag), partiții, curățare de registru, dezinstalare de aplicații, sincronizare cloud, chat, AI de orice fel, mobile, macOS, funcții de echipă, reclame.

### 2.4 Cele 8 presetări

| Nume în UI | Ce face | Scop implicit |
|---|---|---|
| Descărcări după tip | mută fișierele din Downloads în Downloads/Imagini, Documente, Arhive, Instalatoare, Video, Audio | Downloads |
| Capturi de ecran | nume care conțin Screenshot, Captură, Snip, Clipboard → Imagini/Capturi de ecran/AAAA | Downloads, Desktop, Imagini |
| Instalatoare vechi | .exe, .msi, .msix mai vechi de 30 de zile → Coșul de reciclare, cu previzualizare | Downloads |
| Facturi și chitanțe | PDF cu invoice, factura, chitanta, receipt, bon în nume → Documente/Facturi/AAAA | Downloads |
| Desktop curat | orice fișier de pe Desktop mai vechi de 7 zile → Documente/De pe Desktop/AAAA-LL | Desktop |
| Fișiere mari | orice fișier peste 1 GB → doar raport | Downloads, Documente |
| Duplicate | fișiere cu același hash → doar raport, cu opțiunea de a trimite copiile la Coș | Downloads |
| Foldere goale | foldere goale mai vechi de 7 zile → Coșul de reciclare | Downloads |

Fiecare presetare se livrează cu un test de integrare pe un folder sintetic și cu textul de previzualizare în română și engleză.

### 2.5 Principii de interfață

- Nimic nu se mută fără o previzualizare aprobată, la prima rulare și la fiecare regulă nouă.
- Butonul „Anulează” e vizibil tot timpul după un lot, nu ascuns într-un meniu.
- Fiecare rând din previzualizare spune: fișier, de unde, unde, de ce (numele regulii). Fără jargon.
- Fișierele sărite au motivul afișat (blocat, în descărcare, la cerere OneDrive, exclus).
- Un singur ecran principal: previzualizare + reguli + istoric. Setările sunt secundare.
- Fără cuvinte de marketing în aplicație. „Organizează”, nu „transformă”.

### 2.6 Ce înseamnă „fiabil”, ca invarianți măsurabili

1. Valet nu șterge niciodată definitiv. Orice eliminare merge în Coșul de reciclare.
2. Valet nu suprascrie niciodată un fișier existent. Coliziunile primesc un nume nou determinist.
3. Orice operație are o intrare în jurnal scrisă înainte de execuție și marcată după. O repornire după o cădere în mijlocul unui lot lasă fiecare fișier fie la sursă, fie la destinație, niciodată pierdut, niciodată dublu.
4. Orice lot se poate anula complet cât timp fișierele de la destinație nu au fost modificate de utilizator; dacă au fost, utilizatorul e întrebat.
5. Valet nu atinge niciodată folderele de sistem, AppData, Program Files, repo-uri git, node_modules sau fișiere ascunse/de sistem, decât dacă utilizatorul le include explicit.
6. Valet nu atinge fișierele care se scriu încă (descărcări în curs, fișiere blocate) și nici fișierele OneDrive „la cerere” nehidratate.
7. Valet nu iese pe internet decât pentru verificarea actualizărilor, validarea licenței și rapoartele de eroare opt-in. Nimic din conținutul sau numele fișierelor nu pleacă de pe calculator.
8. În fundal: sub 1% CPU și sub 150 MB RAM la relanti (private bytes ale aplicației și ale proceselor WebView2; working set-ul se raportează separat); indexarea a 100.000 de fișiere sub 60 de secunde; previzualizarea pe 10.000 de fișiere sub 3 secunde; pornire sub 2 secunde.

---

## 3. Monetizare

### 3.1 Modelul

**Gratuit, pentru totdeauna:** toate cele 8 presetări, toate tipurile de condiții și acțiuni, previzualizare, undo pe ultimul lot, 2 foldere urmărite, 3 reguli proprii, istoric 7 zile. Suficient ca să fie util și ca să fie recomandat mai departe. Niciun tip de condiție nu e Pro.

**Pro, 29 € cu TVA inclus (prețul se afișează brut), o singură dată, cu actualizări 12 luni:** reguli și foldere urmărite nelimitate, istoric nelimitat, undo pe orice lot din istoric, export/import de reguli; din Faza 5, acțiunile din modulul de spațiu pe disc. După 12 luni versiunea pe care o ai merge pentru totdeauna; reînnoirea actualizărilor costă 15 €/an, opțional. (Modelul Sublime/JetBrains, adaptat la o utilitate mică.)

**Probă:** 14 zile de Pro complet, fără card, apoi cade pe Gratuit fără să piardă nimic din ce a configurat: regulile peste cele 3 gratuite rămân salvate, dar dezactivate, iar utilizatorul alege care 3 rămân active (la fel pentru folderele urmărite peste 2).

**Reclame:** nu. Dovezi: reacția la CCleaner după 2017 (bundling, reclame, telemetrie) a devenit referința negativă în toate discuțiile despre utilitare Windows; recomandările din r/software și r/windows migrează sistematic spre unelte fără reclame (Everything, WizTree, BleachBit). Venitul din reclame pe o utilitate de desktop cu zeci de mii de utilizatori e de ordinul zecilor de euro pe lună; un singur utilizator Pro face cât o lună de reclame.

**Abonament pur:** nu. Pentru utilitare mici, abonamentul e reproșul numărul unu în recenzii. Modelul cu plată unică plus actualizări opționale păstrează venitul recurent fără să stârnească reacția.

Verificat în Faza 2 (docs/reports/phase-2.md, R1): 29 € e sub Hazel (42 $) și peste organizatoarele mici (15–25 $); prețul rămâne. Decizii Vlad, 29 octombrie 2026: limitele de mai sus, TVA inclus.

### 3.2 Plăți și licențe pentru un PFA din România

- **Merchant of record (vinde în numele tău, ține TVA și taxele globale):** Paddle și Lemon Squeezy au același tarif, 5% + 0,50 $ pe tranzacție, fără abonament lunar. Lemon Squeezy a fost cumpărat de Stripe în 2024 și e absorbit treptat în Stripe; în 2026 pagina lor de prețuri nu mai era accesibilă constant. Recomandare: **Paddle** ca opțiune principală (ai lucrat deja cu el la Livrabill, acceptă entități din România), **Polar.sh** ca a doua opțiune (merchant of record open-source, 4% + 0,40 $, chei de licență integrate). Sarcină pentru Faza 4: confirmă în scris, cu suportul lor, că acceptă un PFA (nu doar SRL) și ce acte cer.
- **Chei de licență:** fișier de licență semnat Ed25519 generat de un serviciu minim la tine (o funcție pe Cloud Run sau un endpoint pe Supabase) la webhook-ul de plată; aplicația verifică semnătura offline, cu cheia publică în binar; validare online la cel mult 30 de zile, cu perioadă de grație offline de 30 de zile; maximum 3 activări per licență. Fără DRM mai mult de atât.
- **Microsoft Store:** cont de dezvoltator individual, taxă unică (~19 $). Store semnează pachetul MSIX, ceea ce rezolvă SmartScreen și încrederea fără certificat propriu. Pentru aplicații (nu jocuri), dacă folosești propriul sistem de plată în aplicație, comisionul Store e 0%. Deci: Store = canal de încredere; Pro se cumpără prin Paddle din aplicație.
- **winget:** gratuit, manifest în repo-ul microsoft/winget-pkgs; publicul tehnic îl folosește.

### 3.3 Semnarea codului (decizie cu dovezi din octombrie 2026)

- Azure Artifact Signing (fostul Trusted Signing) este cea mai ieftină opțiune (9,99 $/lună), dar **nu e disponibilă pentru dezvoltatori individuali din afara SUA și Canadei**, iar organizațiile din UE au nevoie de 3 ani de istoric. Un PFA nou din România nu se califică. Se reverifică la fiecare 6 luni; Microsoft a spus că extinde eligibilitatea.
- Din 1 martie 2026, certificatele de semnare de cod au valabilitate maximă de 460 de zile, ceea ce a scumpit certificatele OV/EV clasice (150–300 $/an plus token hardware sau HSM în cloud).
- Plan: (1) Store semnează MSIX-ul pentru publicul larg; (2) pentru GitHub Releases și winget, cerere la **SignPath Foundation** (semnare gratuită pentru proiecte open-source construite în CI public) sau certificat **Certum Open Source** (ieftin, pentru proiecte open-source, cu cititor de card); (3) dacă ambele pică, certificat OV de la SSL.com cu eSigner, bugetat la ~250 $/an. Până la semnare, versiunile beta de pe GitHub poartă avertismentul SmartScreen și spunem asta clar în README.

### 3.4 Canale, în ordine

1. GitHub Releases + winget (Faza 3). Public: tehnicieni, r/software, r/windows, Hacker News „Show HN”.
2. Microsoft Store (Faza 4). Public: toată lumea; recenziile din Store sunt motorul de descoperire.
3. Site minimal cu o pagină, un GIF de 20 de secunde, prețul, linkul de descărcare, FAQ despre privacy (Faza 4). SEO pe „hazel for windows”, „organize downloads folder automatically”, „file organizer windows”.
4. Listări: AlternativeTo (ca alternativă la Hazel, File Juggler, DropIt), Softpedia, MajorGeeks, Product Hunt o singură dată, după Store.

---

## 4. Arhitectura de fiabilitate

### 4.1 Structura codului (ținta după Faza 0)

```
crates/valet-core/        fără cod de OS; reguli, matcher, planificare, jurnal, storage (SQLite)
  src/model.rs rules.rs matcher.rs planner.rs journal.rs executor.rs storage/ config.rs
crates/valet-platform/    tot ce e Windows: watcher, coș de reciclare, atribute OneDrive, căi lungi, context menu, autostart, notificări
  src/lib.rs fs_win.rs trash.rs watcher.rs cloud_placeholders.rs shell.rs
apps/desktop/src-tauri/   comenzi Tauri subțiri, pe module: commands/{rules,plan,execute,history,settings,platform,license}.rs, state.rs
apps/desktop/src/         React: views/{Preview,Rules,History,Settings}, i18n/{ro,en}.json
```

Regula de aur: valet-core compilează și se testează pe Linux în CI, fără nicio dependență Windows. Așa rămâne drumul deschis spre macOS.

### 4.2 Jurnalul de operații (write-ahead)

- SQLite în mod WAL, tabel `operations`: id, batch_id, op_type, source_path, dest_path, size, hash_before, hash_after, state (planned → in_progress → done | failed | rolled_back), error, created_at, finished_at.
- Fiecare operație: scrie `planned` → `in_progress` → execută → verifică (existență, mărime, hash pentru copii) → `done`. Totul într-o tranzacție per operație, nu per lot.
- La pornire: pentru fiecare `in_progress`, verifică starea reală pe disc și finalizează sau anulează. Fără intervenția utilizatorului; raport în istoric.
- Anulare: operațiile inverse, în ordine inversă, cu verificare de hash la destinație; dacă hash-ul diferă (utilizatorul a modificat fișierul), întreabă.

### 4.3 Politica operațiilor pe fișiere (Windows)

| Situație | Politică |
|---|---|
| Mutare pe același volum | `rename` atomic |
| Mutare între volume | copiază → verifică hash → trimite originalul la Coșul de reciclare (nu șterge) |
| Coliziune de nume la destinație | nu suprascrie niciodată; `nume (2).ext`, `nume (3).ext`; determinist, testat |
| Fișier blocat de alt proces | sare, motiv afișat, reîncercare la următoarele 3 treceri ale watcher-ului |
| Fișier în curs de scriere | ignoră `.crdownload .part .tmp .download ~$*` și orice fișier modificat în ultimele 10 secunde sau a cărui mărime se schimbă între două verificări |
| OneDrive / Dropbox „la cerere” (placeholder nehidratat) | sare în v1, motiv afișat; nu-l hash-ui niciodată (ar forța descărcarea) |
| Junction, symlink, reparse point | nu traversează, nu mută |
| Cale peste 260 de caractere | prefix `\\?\` pe toate apelurile; test dedicat |
| Fișier ascuns sau de sistem | exclus implicit |
| Folder protejat de Controlled Folder Access | eroare explicată, fără reîncercare agresivă |
| Eliminare | doar Coșul de reciclare (crate `trash`); niciodată `remove_file` |
| Excluderi implicite | `C:\Windows`, `Program Files*`, `ProgramData`, `AppData`, `$Recycle.Bin`, `System Volume Information`, `.git`, `node_modules`, `target`, `.venv`, orice folder cu `.valetignore` |

### 4.4 Watcher

`notify` cu debounce de 2 secunde; pe overflow al bufferului ReadDirectoryChangesW, rescanare completă a folderului; aplicarea automată pornește doar după prima previzualizare aprobată; pauză automată când utilizatorul e în modul „Previzualizare”; o singură instanță a aplicației (plugin single-instance).

### 4.5 Jurnale, erori, telemetrie

- Loguri locale: `%LOCALAPPDATA%\Valet\logs`, rotație 5 × 2 MB, nivel implicit INFO fără căi complete (doar nume de fișier trunchiat și hash de folder); DEBUG activabil din setări pentru suport.
- Raportare de erori: Sentry (crate `sentry` în Rust, SDK în frontend) **doar opt-in**, cu un ecran care arată exact ce se trimite; scrub de căi; fără conținut de fișiere.
- Panică în Rust = niciodată crash fără jurnal: hook de panică scrie în log și marchează operațiile `in_progress`.

### 4.6 Piramida de testare

| Nivel | Unealtă | Ce acoperă | Prag |
|---|---|---|---|
| Unit | `cargo test` | matcher, planificare, politica de coliziuni, parsing reguli | 100% din condiții și acțiuni au test |
| Property | `proptest` | matcher-ul nu produce niciodată două acțiuni pentru același fișier; redenumirea de coliziune e injectivă; jurnalul rămâne consistent după orice secvență de operații | rulate în CI |
| Integrare | tempdir cu 10.000 de fișiere sintetice | presetările, mutări pe același volum și între volume, coliziuni, undo complet | toate presetările trec |
| Injecție de defecte | proces copil omorât în mijlocul unui lot | recuperarea la pornire lasă fiecare fișier exact o dată, la sursă sau la destinație | 100 de rulări fără pierdere |
| Windows | CI pe `windows-latest` | căi lungi, atribute, Coș de reciclare, watcher | verde la fiecare commit |
| UI | `tauri-driver` + WebDriver (Edge) | onboarding, previzualizare, aplicare, anulare | smoke test în CI, restul manual |
| Performanță | `criterion` + script | indexare 100k fișiere, previzualizare 10k, RAM la relanti | bugetele din 2.6 |

### 4.7 Livrare

- CI: GitHub Actions, matrice `ubuntu-latest` (core) + `windows-latest` (tot); `cargo clippy -D warnings`, `cargo fmt --check`, `cargo test`, `pnpm build`, `pnpm test`.
- Build de release cu `tauri-action`, instalator NSIS per utilizator, WebView2 în modul `downloadBootstrapper`.
- Updater: plugin `tauri-plugin-updater` cu chei minisign (`tauri signer generate`); cheia privată nu intră niciodată în repo, doar în GitHub Secrets.
- Lansare în trepte: `beta` (GitHub pre-release, updater pe canalul beta) → `stable` după 7 zile fără rapoarte de fișiere pierdute.
- MSIX pentru Store construit în CI din output-ul de release (makeappx), semnat de Store.

---

## 5. Fazele de execuție

Reguli comune tuturor fazelor:
- O fază se termină cu `docs/reports/phase-N.md` scris de PM și cu oprirea sesiunii. PM-ul nu începe faza următoare până nu primește de la Vlad „Faza N aprobată”.
- Fiecare subagent primește o sarcină cu criteriu de acceptare verificabil mecanic (test, comandă, prag). „Am terminat” fără test nu se acceptă.
- O schimbare la un moment dat; după fiecare, `cargo check`, `cargo test` pe crate-ul atins, `pnpm build` dacă e frontend.
- Commit direct pe `main`, mesaje scurte, la obiect. Worktree-uri doar când PM-ul rulează doi executanți în paralel pe arii disjuncte (de la Faza 2), și tot PM-ul le integrează local, fără PR.
- Nicio dependență nouă fără aprobarea lui Vlad, cu excepția listei din CLAUDE.md.
- Textele din interfață în română (fișier `ro.json`), engleză în `en.json`; cod, identificatori și comentarii în engleză.

### Faza 0. Adevăr și curățenie (1 săptămână)

Obiectiv: repo-ul spune adevărul, compilează curat, are CI verde și o structură în care se poate lucra.

| Subagent | Sarcini |
|---|---|
| researcher (read-only) | inventar: ce compilează, warnings, lista celor ~38 de comenzi Tauri cu „folosită din frontend: da/nu”, dependențe și versiuni, ce face fiecare view |
| docs-writer | README real în engleză (ce e, captură placeholder, build pe Windows cu scripts/windows_dev_setup.ps1, formatul regulilor cu exemplul din sample_rules.json, licență MIT); șterge DEVELOPMENT_COMPLETE.md; rescrie sau șterge TESTING_GUIDE.md și CONTRIBUTING.md în funcție de inventar |
| rust-core | sparge `main.rs` în module (`commands/rules.rs, commands/operations.rs, commands/backup.rs, commands/settings.rs, commands/platform.rs, commands/diagnostics.rs, state.rs`) fără nicio schimbare de comportament; comenzile de „health/security audit/performance/system tests” se izolează în `diagnostics.rs` și se marchează pentru decizie |
| qa | `cargo clippy -D warnings` și `cargo fmt` curate; teste existente rulează; adaugă teste de caracterizare pentru `matcher.rs` și `engine::dry_run_for_paths` ca plasă înainte de Faza 1; CI pe `windows-latest` + `ubuntu-latest` verde |

Criterii de ieșire: build fără warnings; `cargo test` verde pe ambele OS; README corect; `main.rs` sub 300 de linii; raport cu lista comenzilor de păstrat/șters propusă lui Vlad.

Checklist manual Vlad: clonează curat, rulează `scripts/windows_dev_setup.ps1`, `pnpm tauri dev` pornește aplicația, previzualizarea pe un folder de test arată aceleași rezultate ca înainte de refactor.

### Faza 1. Nucleul de siguranță (3 săptămâni)

Obiectiv: invarianții 1–6 din 2.6 implementați și testați, înainte de orice funcție nouă.

| Subagent | Sarcini |
|---|---|
| rust-core | `journal.rs` (write-ahead, stări, recuperare la pornire); `executor.rs` cu politica din 4.3 (rename pe același volum, copy+verify+trash între volume, coliziuni deterministe); `planner.rs` separat de matcher; excluderi implicite și `.valetignore`; eliminarea definitivă dispare din cod |
| tauri-platform | `valet-platform`: Coș de reciclare prin crate `trash`; detecție placeholder OneDrive/Dropbox prin atribute; căi lungi `\\?\`; detecție fișier blocat și fișier în scriere; watcher cu debounce și rescanare pe overflow |
| qa | proptest pe matcher și coliziuni; test de injecție de defecte (proces copil omorât în mijlocul unui lot, 100 de rulări); integrare pe tempdir cu 10.000 de fișiere; teste Windows pentru căi lungi, blocate, placeholder (simulat cu atribute) |
| docs-writer | `docs/SAFETY.md`: invarianții, politica, ce face aplicația în fiecare caz-limită; e și textul viitorului FAQ |

Criterii de ieșire: toate testele de mai sus verzi în CI; zero apeluri `remove_file`/`remove_dir_all` în cod în afara testelor (hook-ul le blochează); recuperarea după cădere demonstrată de test; raport cu rezultatele.

Checklist manual Vlad: pe un folder de test cu 500 de fișiere, rulează un lot, închide aplicația din Task Manager în timpul lotului, redeschide: nimic pierdut, nimic dublat, istoricul explică ce s-a întâmplat. Anulează lotul: folderul e identic cu înainte (comparare cu un snapshot).

### Faza 2. Produsul v1 (3 săptămâni)

Obiectiv: cele 8 presetări, onboarding, previzualizare, istoric și anulare în interfață, română și engleză.

| Subagent | Sarcini |
|---|---|
| rust-core | condiții noi: vechime, grup de tip, folder gol, duplicat (hash, raport); acțiuni: trimite la Coș, raportează; presetările ca fișiere JSON în `crates/valet-core/presets/` cu teste |
| frontend | ecran principal (Previzualizare, Reguli, Istoric), onboarding în 3 pași, buton „Anulează” permanent, motive pentru fișiere sărite, i18n ro/en, stare goală pentru fiecare ecran |
| tauri-platform | tray, autostart opțional, notificări, single-instance, `show_in_explorer` |
| qa | smoke test UI cu `tauri-driver`; teste de integrare pentru fiecare presetare; bugete de performanță măsurate cu `criterion` |
| researcher | verifică prețurile competitorilor și textele de poziționare; propune ordinea presetărilor în onboarding |
| docs-writer | ghidul utilizatorului în `docs/USER_GUIDE.ro.md` și `.en.md`; capturi de ecran reale |

Criterii de ieșire: onboarding → previzualizare → aplicare → anulare funcționează end-to-end în smoke test; toate presetările au test verde; bugetele din 2.6 respectate; traduceri complete (script care verifică chei lipsă).

Checklist manual Vlad: instalează pe un calculator curat (VM), trece prin onboarding fără să citească documentația, lasă watcher-ul pornit 3 zile pe Downloads real; notează orice surpriză.

### Faza 3. Livrare (2 săptămâni)

Obiectiv: instalator, updater, semnare, raportare de erori, winget.

| Subagent | Sarcini |
|---|---|
| release | `tauri.conf.json` final (NSIS per utilizator, WebView2 bootstrapper), `tauri-action` în GitHub Actions, canale beta/stable pentru updater, chei minisign generate de Vlad și puse în Secrets, manifest winget, pagină de release cu changelog |
| tauri-platform | Sentry opt-in cu scrub de căi; loguri cu rotație; ecranul „Ce trimitem” |
| qa | test de instalare/actualizare/dezinstalare pe VM Windows 10 și 11; test de upgrade de la versiunea anterioară cu jurnal existent (migrații SQLite) |
| docs-writer | README cu avertismentul SmartScreen până la semnare, `docs/PRIVACY.md`, `CHANGELOG.md` |

Decizie Vlad înainte de fază: cererea la SignPath Foundation și/sau certificatul Certum; contul Microsoft Partner Center.

Criterii de ieșire: release `v1.0.0-beta.1` publicat automat din tag; updater-ul face upgrade de la beta.1 la beta.2 pe VM; manifest winget acceptat; raport de eroare de test ajunge în Sentry doar după opt-in.

### Faza 4. Beta publică, licențe, plăți (3 săptămâni, în paralel cu feedback)

Obiectiv: primii utilizatori reali, Pro cumpărabil, Store.

| Subagent | Sarcini |
|---|---|
| rust-core | modulul de licență: verificare Ed25519 offline, activări, grație; gating-ul funcțiilor Pro |
| release | serviciul minim de licențe (webhook Paddle → fișier semnat → email), MSIX pentru Store, listare Store cu capturi în ro/en |
| frontend | ecranul Pro (ce primești, preț, cumpără, introdu licența), probă de 14 zile |
| qa | teste pe licență (expirată, falsă, prea multe activări, offline 31 de zile); test de cumpărare în sandbox Paddle |
| docs-writer | site cu o pagină (HTML static în `site/`), FAQ, pagina de confidențialitate, șabloane de răspuns la feedback |

Decizii Vlad: Paddle sau Polar (confirmare scrisă că acceptă PFA), prețul final, textul anunțului pentru r/software și Show HN.

Criterii de ieșire: o cumpărare reală de test; licența deblochează Pro pe două calculatoare și refuză al patrulea; Store listing aprobat; 50 de instalări externe fără raport de fișiere pierdute.

### Faza 5. Spațiu pe disc (3 săptămâni)

Obiectiv: „ce îmi mănâncă discul”, integrat cu regulile.

Funcții: scanare rapidă pe volum (MFT pe NTFS, ca WizTree, prin crate dedicat sau prin fallback walkdir), treemap, top foldere și fișiere, categorii (instalatoare, arhive, video, cache cunoscute: browser, pip, npm, Steam shader cache), acțiuni doar prin reguli și Coș, niciodată ștergere directă. Gratuit: scanare și vizualizare; Pro: acțiuni din treemap și reguli generate din selecție.

Subagenți: rust-core (scanare, categorii), frontend (treemap), qa (performanță: 1 TB cu 2 milioane de fișiere sub 2 minute cu MFT), docs-writer.

### Faza 6. Stratul AI, local (4–6 săptămâni)

Obiectiv: regulile se scriu în limbaj natural; condițiile pe conținut.

1. Generator de reguli: text → JSON de regulă, afișat și confirmat de utilizator; model mic local (prin `llama.cpp` sau ONNX Runtime, model de 1–3 miliarde de parametri, descărcat opțional); fără internet. Acceptare: 50 de formulări în română și engleză generează reguli valide, testate.
2. Condiții pe conținut: `contentIs: invoice|contract|ticket` prin clasificare locală a PDF-urilor (text + OCR mic), `imageIs` prin model de vision mic; rezultatele intră ca condiții de regulă, cu aceeași previzualizare și undo. Aici intră experiența ta de TensorRT/ONNX.
3. Sugestii: după indexare, aplicația propune reguli pe baza statisticilor („340 de instalatoare din 2023”).

Regulă fixă: nicio acțiune generată de model nu ocolește jurnalul, previzualizarea sau anularea.

### Mai târziu, cu dovezi

macOS (după ce v1 face bani 6 luni); funcții pentru echipe (reguli partajate) doar dacă apar cereri; partiții doar dacă utilizatorii le cer constant și doar ca redirecționare spre Disk Management, nu implementare proprie.

---

## 6. Configurarea Claude Code

Fișierele sunt în repo: `CLAUDE.md`, `.claude/settings.json`, `.claude/hooks/*.ps1`, `.claude/agents/*.md`, `.claude/skills/*/SKILL.md`. Pe scurt:

- **PM = sesiunea principală.** Nu are fișier de agent. CLAUDE.md îi spune cum lucrează. Comanda `/phase N` (skill) pornește o fază.
- **Executanți:** `researcher` (doar citire), `rust-core`, `tauri-platform`, `frontend`, `qa`, `release`, `docs-writer`. Fiecare are unelte limitate și arie de fișiere delimitată.
- **Hook-uri:** după orice editare de `.rs`, `cargo check` pe crate-ul atins; înainte de orice comandă shell, blocare pentru `git push --force`, ștergeri recursive în afara `target/`, `Remove-Item -Recurse` pe căi din afara repo-ului, orice atingere a `.env`, a cheilor de updater sau a `tauri.conf.json` fără aprobarea explicită a lui Vlad în prompt. Hook-ul `Stop` refuză oprirea dacă `cargo test` nu a rulat în sesiune.
- **Setări:** adâncimea de subagenți 1 (doar PM-ul creează executanți); fork mode oprit, ca PM-ul să poată aștepta un executant înainte să treacă mai departe; permisiuni de editare acceptate automat în repo, comenzi `cargo`, `pnpm`, `git` (fără push) permise fără întrebare.
- **MCP:** niciunul necesar în Faza 0–2. Opțional din Faza 3: GitHub MCP pentru release notes.
- **Cost:** o fază înseamnă zeci de milioane de tokeni cu subagenți; modelul principal pe `opus`, `docs-writer` și `researcher` pe `sonnet`.

Ce rămâne uman, indiferent de autonomie: instalarea și rularea aplicației pe Windows real, testele din checklist, aprobarea fiecărei faze, cheile de semnare, contul Store, contul de plăți, prețul.

---

## 7. Promptul de pornire pentru Faza 0

Vezi `docs/KICKOFF_PHASE0.md`. Se lipește o singură dată, din rădăcina repo-ului, după ce fișierele din `.claude/` și `CLAUDE.md` sunt în repo. Fazele următoare se pornesc cu `/phase N` după aprobare.

---

## 8. Riscuri, întrebări deschise, presupuneri

**Riscuri**
1. Fiabilitate: o singură pierdere de fișier raportată public în beta poate termina produsul. De aceea Faza 1 precede orice funcție.
2. SmartScreen fără semnare: descărcările de pe GitHub vor avea avertisment până la SignPath/Certum/Store. Acceptat pentru beta, inacceptabil pentru lansare.
3. Microsoft adaugă organizare automată în Explorer. Probabilitate mică în 2 ani (direcția lor e inversă, retrag acțiuni Copilot), dar e singurul concurent care poate anula piața.
4. Timp: masterul ia prioritate în sesiuni; o fază de 3 săptămâni poate ajunge la 6. Planul tolerează asta; ce nu tolerează e lucrul la două faze deodată.
5. Open-source + Pro gated: cineva publică un build fără gating. Răspuns: nu ne batem cu el; majoritatea plătitorilor plătesc pentru comoditate și actualizări.
6. Cost Claude Code: fazele cu subagenți consumă mult; dacă limita abonamentului e atinsă, PM-ul rulează executanții secvențial, pe `sonnet`.

**Întrebări deschise pentru Vlad**
1. Numele „Valet”: există produse cu acest nume (inclusiv un instrument de dezvoltare PHP, „Laravel Valet”). Nu e o problemă de utilizare, dar verifică marca în EUIPO înainte de Store și cumpără un domeniu disponibil (de exemplu `valetapp.ro` sau `getvalet.app`).
2. Buget anual acceptat pentru servicii: Store 19 $ o dată, Sentry gratuit la volumul inițial, certificat 0–250 $, Paddle 0 fix, domeniu ~15 €.
3. Versiunea minimă de Windows: propun Windows 10 22H2 și Windows 11; WebView2 e preinstalat pe 11 și se instalează pe 10.
4. Ore pe săptămână pe care te bazezi; planul presupune 10–15.
5. Repo public sau privat pentru aplicația desktop în Faza 4 (nucleul rămâne public oricum).

**Presupuneri**
- Tauri 2 stabil, pnpm, Rust stable; versiunile exacte le fixează Faza 0 după inventar.
- Prețurile competitorilor din secțiunea 1 sunt orientative până la verificarea din Faza 2.
- Cifrele țintă din secțiunea 0 sunt estimări pentru decizie, nu previziuni.
