# Listarea în Microsoft Store (română)

English version: [listing.en.md](listing.en.md)

**CIORNĂ — decide Vlad.** Listarea din Store nu e activă. Nimic din acest text nu a fost introdus în Partner Center. Limitele de mai jos (caractere, număr de elemente) sunt cele date în sarcina acestei faze; verifică-le în Partner Center când introduci textul, pentru că nu au fost verificate acolo. Faptele vin din `README.md`, `docs/LICENSING.md`, `docs/PRIVACY.ro.md`, `docs/SAFETY.md` și `docs/PLAN.md` secțiunile 3.1 și 3.2.

Punctele pe care le poate lămuri doar Vlad sunt marcate `[VLAD: ...]`. Sunt adunate la sfârșitul fișierului.

---

## Numele produsului

Valet

[VLAD: rezervă numele în Partner Center. `docs/PLAN.md` secțiunea 8, întrebarea 1, cere verificarea mărcii (EUIPO) înainte de Store, pentru că și alte produse folosesc numele.]

## Descriere scurtă

Valet pune ordine în Descărcări și pe Desktop după reguli scrise de tine. Arată o previzualizare înainte să mute ceva, trimite fișierele eliminate doar la Coșul de reciclare și poate anula un lot.

(circa 200 de caractere)

## Descriere

Valet ține în ordine foldere precum Descărcări și Desktop, după reguli pe care le poți citi și anula.

Tu alegi folderele pe care le urmărește Valet. O regulă are condiții (extensia fișierului, tipul, o parte din nume, mărimea, vechimea, folder gol, duplicat) și o acțiune: mută, copiază, trimite la Coșul de reciclare sau doar raportează. Opt reguli gata făcute acoperă cazurile obișnuite, de exemplu Descărcări după tip, Capturi de ecran și Facturi și chitanțe.

Înainte să mute ceva, Valet arată o previzualizare: fiecare fișier, unde este, unde ar ajunge și ce regulă a decis. Nu rulează nimic până nu aprobi.

Valet nu șterge niciodată definitiv un fișier: ce elimină ajunge în Coșul de reciclare Windows. Nu suprascrie niciodată un fișier: un nume ocupat devine „raport (2).pdf”. Lasă în pace Windows, Program Files, AppData, .git, node_modules și fișierele ascunse și sare peste fișierele care încă se descarcă, peste cele blocate și peste cele OneDrive sau Dropbox nedescărcate.

Un lot terminat se poate anula cât timp fișierele mutate nu au fost modificate; dacă ai modificat unul, Valet te întreabă. Fiecare operație se scrie într-un jurnal înainte să ruleze, iar după o cădere Valet verifică operațiile neterminate la pornirea următoare.

Fără cont, fără reclame, fără telemetrie. Actualizările vin prin Store; versiunea din Store nu verifică GitHub. Doar cu o licență Pro, Valet verifică licența (identificatorul licenței, un hash într-un singur sens al identificatorului calculatorului, versiunea aplicației). Nu se trimit nume de fișiere, căi sau conținut. Pornirea cu Windows se setează din setările Windows (Valet are un buton care le deschide). Dezinstalarea aplicației din Store șterge datele ei, inclusiv Istoricul și anularea.

Gratuit: toate cele opt reguli gata făcute, 3 reguli proprii, 2 foldere urmărite, istoric de 7 zile, anularea ultimului lot. Pro: fără limite, anularea oricărui lot, export și import de reguli; 29 € cu TVA inclus, o singură dată, cu actualizări 12 luni, cumpărat în afara Store. Proba de 14 zile de Pro nu cere card.

[VLAD: propoziția cu prețul de mai sus („Pro: ... 29 € ...”) e adevărată doar când cumpărarea funcționează. Decizia din 2026-10-29: vânzarea se deschide abia după ce webhook-ul comerciantului a trecut o cumpărare și o rambursare de probă cap-coadă (`docs/LICENSING.md`, „License service”); până atunci site-ul spune că cumpărarea nu e deschisă. Dacă trimiți listarea înainte să se deschidă vânzarea, scoate propoziția cu prețul și pune în loc: „Pro: fără limite, anularea oricărui lot, export și import de reguli. Cumpărarea Pro nu e deschisă încă. Proba de 14 zile de Pro nu cere card.” Dacă vânzarea e deschisă la trimitere, păstrează propoziția, cu rambursarea în 14 zile dacă vrei s-o pomenești.]

Română și engleză. Windows 10 22H2 și Windows 11, pe 64 de biți. Valet e în beta: nu îl îndrepta spre foldere ale căror conținut nu îl poți pierde și folosește previzualizarea.

(circa 2.000 de caractere; limita e 10.000.)

## Puncte principale (cel mult 20, 8 folosite)

1. Reguli cu condiții pe extensie, tip de fișier, nume, mărime, vechime, foldere goale și duplicate.
2. Acțiuni: mută, copiază, trimite la Coșul de reciclare sau doar raportează.
3. Opt reguli gata făcute, gratuite și care nu se numără la limită.
4. Previzualizare a fiecărei mutări înainte să se întâmple; modul automat doar după ce aprobi una.
5. Anularea unui lot terminat, cât timp fișierele mutate nu au fost modificate.
6. Doar Coșul de reciclare: nicio ștergere definitivă, nicio suprascriere a fișierelor existente.
7. Sare peste fișierele în descărcare, peste cele blocate și peste cele OneDrive sau Dropbox nedescărcate.
8. Interfață în română și engleză; fără cont, fără reclame, fără telemetrie.

## Termeni de căutare (cel mult 7)

1. organizator de fișiere
2. hazel for windows
3. organizare descărcări
4. sortare automată fișiere
5. curățare descărcări
6. reguli pentru foldere
7. mutare fișiere

[VLAD: „hazel for windows” e un termen de comparație, cum prevede `docs/PLAN.md` secțiunea 3.4 pentru SEO. Politicile Store pot respinge numele altui produs ca termen de căutare; dacă trimiterea e respinsă din acest motiv, înlocuiește-l cu „curățare desktop”. Verifică textul curent al politicii; nu a fost verificat.]

## Ce e nou în 1.0

Prima versiune în Microsoft Store.

- Reguli cu previzualizare și anulare pentru Descărcări, Desktop și alte foldere alese de tine.
- Opt reguli gata făcute.
- Plan Gratuit și plan Pro (Pro se cumpără în afara Store).
- Probă de 14 zile de Pro, fără card.
- Interfață în română și engleză.

[VLAD: compară cu `CHANGELOG.md` pentru versiunea pe care o trimiți. Nu enumera nimic ce nu e în acel build.]

## Adresa politicii de confidențialitate

https://valetapp.ro/privacy.html

Store o cere pentru că aplicația folosește internetul. Pagina există în depozit (`site/privacy.html`); adresa răspunde doar după ce site-ul e pus pe `valetapp.ro`. [VLAD: pune site-ul online pe domeniu înainte de trimitere și deschide adresa din Partner Center ca s-o verifici.]

## Adresa de asistență

https://github.com/OsztrovszkyVlad/Valet/issues

Deocamdată asistența e doar GitHub Issues (decizia din 2026-10-29); nu există o adresă de email de asistență. Pentru licențe, aplicația arată în Setări, Valet Pro un „Cod licență (pentru asistență)”.

## Capturi de ecran

Capturile vin din `docs/screenshots/` al lui Vlad, făcute pe Windows cu foldere de test (fără nume reale de fișiere). Folderul nu există încă. Imaginile din `docs/images/` arată căi de test de pe Linux și nu sunt pentru Store. Verifică dimensiunile cerute în Partner Center (îmi amintesc 1366x768 sau mai mari; neverificat).

| # | Fișier (de făcut) | Legendă |
|---|---|---|
| 1 | `docs/screenshots/01-preview.png` | Previzualizare: fiecare mutare e listată cu regula care a decis-o. Nu rulează nimic până nu aprobi. |
| 2 | `docs/screenshots/02-presets.png` | Opt reguli gata făcute, alese la prima pornire. |
| 3 | `docs/screenshots/03-after-organize.png` | După un lot: rezultatul și butonul „Anulează lotul”. |
| 4 | `docs/screenshots/04-history.png` | Istoric: fiecare lot, ce s-a mutat și unde. |
| 5 | `docs/screenshots/05-rules.png` | Reguli: condiții și acțiuni pe care le poți citi. |
| 6 | `docs/screenshots/06-settings.png` | Setări: foldere urmărite, limbă, pornire cu Windows, actualizări. |

## Chestionarul de vârstă (IARC, răspunsuri deduse)

Nu pot rula chestionarul. Acestea sunt răspunsurile pe care le sugerează comportamentul aplicației; Vlad îl completează în Partner Center și citește fiecare întrebare așa cum e formulată acolo. Categoria ar fi o aplicație utilitară sau de productivitate.

| Subiect | Răspuns | Temei |
|---|---|---|
| Violență, sânge, frică | Nu | Niciun astfel de conținut. |
| Conținut sexual sau nuditate | Nu | Niciun astfel de conținut. |
| Limbaj vulgar sau umor grosolan | Nu | Textele interfeței sunt fixe, în `ro.json` și `en.json`. |
| Substanțe controlate (droguri, alcool, tutun) | Nu | Niciun astfel de conținut. |
| Jocuri de noroc sau simulări | Nu | Nu există. |
| Conținut generat de utilizatori, chat, interacțiune între utilizatori | Nu | Fără conturi și fără schimb între utilizatori. Aplicația arată doar numele fișierelor proprii ale utilizatorului. |
| Partajează locația utilizatorului | Nu | Nu se folosește. |
| Partajează informații personale cu alții | [VLAD: verifică] | Doar cu o licență Pro: identificatorul licenței, identificatorul hash al calculatorului, versiunea aplicației, către serverul propriu de licențe. Fără nume de fișiere sau căi. Vezi `docs/PRIVACY.ro.md`. Răspunde după formularea chestionarului. |
| Acces nerestricționat la internet sau un browser | Nu | Interfața încarcă doar fișierele proprii ale aplicației; nu are browser. Verificarea licenței (și, în afara Store, a versiunii) merge la adrese fixe. |
| Reclame | Nu | Nu există. |
| Achiziții în aplicație prin Store | Nu | Pro se cumpără în afara Store (`docs/PLAN.md` secțiunea 3.2). |
| Achiziții digitale de orice fel în aplicație | [VLAD: verifică] | Dacă ecranul Pro are un buton „Cumpără” care deschide browserul, chestionarul sau politica Store îl pot număra. Vezi mai jos. |

Rezultat așteptat, neverificat: cea mai joasă clasă de vârstă (potrivit pentru toate vârstele).

## Întrebare de politică Store pentru Vlad: achiziții externe

`docs/PLAN.md` secțiunea 3.2 spune că pentru aplicații (nu jocuri) care își folosesc propriul sistem de plată, comisionul Store e 0%, și că Pro se cumpără prin Paddle (sau Polar) din aplicație. Asta a fost scris din memorie în plan; nu a fost verificat aici. Înainte de trimitere:

1. Citește politicile curente Microsoft Store despre plăți și despre aplicațiile care deblochează funcții cu o achiziție făcută în afara Store. Verifică dacă un buton „Cumpără” în aplicație e permis, dacă un link trebuie să deschidă browserul și dacă listarea are voie să numească prețul sau site-ul extern.
2. Verifică dacă o aplicație gratuită în Store, cu limite pe care o licență externă le ridică, cere o precizare anume în descriere. Descrierea de mai sus spune „cumpărat în afara Store” din acest motiv.
3. Dacă politica nu permite butonul „Cumpără” în build-ul pentru Store, acel build are nevoie de un ecran care doar primește un fișier de licență, iar linkul de cumpărare rămâne pe site. E o schimbare în aplicație; cere-o frontendului abia după ce decizi.

## Note pentru testerii de certificare (ciornă)

- Planul Gratuit merge fără licență și fără cont.
- Funcțiile Pro cer un fișier de licență cumpărat în afara Store. [VLAD: pune aici o licență de test pentru testeri, emisă manual, și spune cum se importă (Setări).]
- Aplicația citește și mută fișiere în folderele alese de utilizator și trimite fișierele eliminate la Coșul de reciclare. O aplicație desktop împachetată care face asta are de obicei nevoie de capabilitatea full-trust (`runFullTrust`); Partner Center poate cere un motiv. [VLAD: neverificat. Manifestul din `packaging/msix/AppxManifest.xml` cere `runFullTrust`, iar fluxul `.github/workflows/msix.yml` construiește pachetul, dar pachetul nu a fost trimis în Partner Center.]
- Folosirea rețelei: în versiunea din Store, doar verificarea licenței, și numai cu o licență Pro importată într-un build cu server de licențe (cele de acum nu au). Verificarea versiunii nu rulează în versiunea din Store. Nicio altă conexiune. `docs/PRIVACY.ro.md`.
- Pornirea cu Windows: în versiunea din Store nu e o bifă în aplicație; Setări are un buton care deschide setările Windows pentru aplicațiile de la pornire (`packaging/msix/README.md`, secțiunea 5).

## Întrebări deschise pentru Vlad

1. Numele: verificarea mărcii și rezervarea „Valet” înainte de trimitere.
2. Adresa politicii de confidențialitate: `https://valetapp.ro/privacy.html` e hotărâtă; site-ul trebuie să fie online pe domeniu înainte de trimitere.
3. Politica Store despre achiziții externe (secțiunea de mai sus).
4. Dacă „hazel for windows” rămâne termen de căutare.
5. Răspunsurile de vârstă marcate `[VLAD: verifică]`.
6. O licență de test pentru testerii de certificare.
7. Capturi de pe Windows în `docs/screenshots/`.
8. Pachetul MSIX se construiește în CI (`packaging/msix/README.md`), dar nu a fost trimis în Partner Center și nu a fost încercat într-o instalare din Store; identitatea pachetului vine din Partner Center.
9. Cumpărarea Pro: se deschide după testul de probă al webhook-ului comerciantului (plată și rambursare). La trimitere, alege varianta de text din secțiunea „Descriere” care se potrivește cu starea de atunci.
