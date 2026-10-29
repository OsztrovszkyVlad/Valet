# Ghid de utilizare Valet

English version: [USER_GUIDE.en.md](USER_GUIDE.en.md)

Valet pune ordine în fișierele de pe calculatorul tău, după reguli pe care le vezi și le poți anula. Ghidul descrie ce face aplicația în versiunea din acest depozit.

Valet e încă în faza de test. Nicio versiune nu a fost publicată încă. Când va exista una, instalatorul va fi pe pagina GitHub Releases ([secțiunea 17](#17-instalare-actualizări-și-dezinstalare) spune cum se instalează și de ce Windows avertizează); până atunci se pornește din codul sursă (vezi [README.md](../README.md)). Până nu ai încredere în el, lucrează pe foldere în care ai copii ale fișierelor.

Valet are un plan Gratuit cu limite și un plan Pro. Primele 14 zile au toate funcțiile Pro. Licențele nu sunt încă active în versiunile de acum: cumpărarea și activarea unei licențe nu funcționează încă. Detalii în [secțiunea 18](#18-valet-pro-și-planul-gratuit).

Capturile de ecran din ghid arată foldere de test, de aceea căile încep cu `/tmp/valet-e2e-…`. Au fost făcute din aplicația reală, pe Linux, de testul automat al interfeței. Pe Windows căile arată ca `C:\Users\nume\Descărcări`.

## Cuprins

1. [Ce face Valet și ce nu face](#1-ce-face-valet-și-ce-nu-face)
2. [Prima pornire](#2-prima-pornire)
3. [Previzualizarea](#3-previzualizarea)
4. [Organizează](#4-organizează)
5. [Anularea unui lot](#5-anularea-unui-lot)
6. [Istoric](#6-istoric)
7. [Cele 8 presetări](#7-cele-8-presetări)
8. [Reguli proprii](#8-reguli-proprii)
9. [Organizarea automată](#9-organizarea-automată)
10. [Zona de notificare și pornirea cu Windows](#10-zona-de-notificare-și-pornirea-cu-windows)
11. [Setări](#11-setări)
12. [Ce nu atinge Valet niciodată](#12-ce-nu-atinge-valet-niciodată)
13. [Discuri fără Coș de reciclare](#13-discuri-fără-coș-de-reciclare)
14. [Confidențialitate](#14-confidențialitate)
15. [Unde sunt datele](#15-unde-sunt-datele)
16. [Limite cunoscute](#16-limite-cunoscute)
17. [Instalare, actualizări și dezinstalare](#17-instalare-actualizări-și-dezinstalare)
18. [Valet Pro și planul Gratuit](#18-valet-pro-și-planul-gratuit)

## 1. Ce face Valet și ce nu face

**Ce face**

- Urmărește folderele pe care le alegi, de exemplu Descărcări și Desktop.
- Aplică regulile active pe fișierele din aceste foldere. O regulă poate muta un fișier, îl poate copia, îl poate trimite la Coșul de reciclare sau doar îl poate raporta.
- Arată lista cu ce ar face (previzualizarea) înainte să atingă un fișier.
- Scrie fiecare operație într-un jurnal înainte să o facă. Jurnalul se vede în ecranul Istoric.
- Poate anula un lot întreg.
- Dacă aplicația sau calculatorul se oprește în mijlocul unui lot, la pornirea următoare verifică operațiile rămase neterminate.
- Poate organiza singur fișierele noi, dar numai după ce ai aprobat o previzualizare (vezi [secțiunea 9](#9-organizarea-automată)).
- Are un plan Gratuit, cu limite, și un plan Pro (vezi [secțiunea 18](#18-valet-pro-și-planul-gratuit)).

**Ce nu face**

- Nu șterge nimic definitiv. Orice eliminare merge în Coșul de reciclare Windows.
- Nu suprascrie fișiere. Dacă numele de la destinație e ocupat, fișierul primește `nume (2).ext`, apoi `nume (3).ext` și așa mai departe.
- Nu redenumește fișiere și nu le etichetează. Aceste acțiuni nu există în versiunea de acum.
- Nu se uită la conținutul fișierelor ca să le clasifice. Regulile se uită la nume, extensie, tip, mărime, date și cale. Singura citire de conținut e verificarea cu amprentă (hash) a fișierelor pe care le mută, le copiază sau le trimite la Coș, și compararea fișierelor de aceeași mărime pentru regulile de duplicate.
- Pe internet merge doar pentru două lucruri: ca să verifice dacă există o versiune nouă a lui Valet și, după ce îi dai voie, ca s-o descarce; și, numai dacă ai importat o licență Pro, ca să verifice licența. Nu trimite nume de fișiere, căi sau conținut (vezi [secțiunea 14](#14-confidențialitate)).
- Nu atinge folderele de sistem, fișierele ascunse și fișierele OneDrive sau Dropbox nedescărcate (vezi [secțiunea 12](#12-ce-nu-atinge-valet-niciodată)).

## 2. Prima pornire

La prima pornire apare un ghid în 3 pași. Nimic nu se mută în timpul lui. Ghidul pornește în română.

**Pasul 1 din 3: folderele**

![Pasul 1: alegerea folderelor](images/onboarding-1-folders.png)

- Sus, lista „Limba” oferă Română și English. Ecranele se schimbă imediat, iar alegerea se salvează, deci numele și descrierile presetărilor de la pasul 2 și numele folderelor presetărilor instalate la final (Imagini sau Images, Documente sau Documents și așa mai departe) o urmează. Dacă limba nu poate fi salvată, ecranele revin la limba de dinainte și apare un mesaj de eroare. „Continuă” e inactiv cât timp se salvează limba.
- Descărcări și Desktop sunt bifate. Documente și Imagini nu sunt.
- Poți adăuga alt folder scriind calea lui în câmpul „Adaugă alt folder”, de exemplu `C:\Users\nume\Proiecte`. Nu există fereastră de alegere a folderului.
- Trebuie să bifezi cel puțin un folder.
- Valet refuză un folder dacă e un disc întreg (`C:\`, `D:\`, `\\server\share`) sau dacă în calea lui apare un nume exclus implicit (de exemplu `AppData` sau `Program Files`). Un folder care nu există acum, de exemplu un disc extern scos, nu e refuzat: se salvează și e marcat „indisponibil” (vezi [secțiunea 11](#11-setări)). Valet verifică folderele când apeși „Continuă” la pasul 2. Dacă unul e refuzat, primești un mesaj și revii la pasul 1.
- Lista se poate schimba oricând în Setări.

**Pasul 2 din 3: regulile**

![Pasul 2: alegerea presetărilor](images/onboarding-2-presets.png)

- Vezi cele 8 presetări, fiecare cu descriere și numărul de reguli. Cele bifate din oficiu sunt Descărcări după tip, Capturi de ecran, Facturi și chitanțe și Fișiere mari.
- Eticheta „trimite la Coș” arată presetările care trimit fișiere la Coșul de reciclare. Eticheta „doar raport” arată presetările care nu mută nimic.
- „Sari peste” continuă fără nicio presetare. „Continuă” salvează folderele și presetările bifate.
- Ce face fiecare presetare e în [secțiunea 7](#7-cele-8-presetări).

**Pasul 3 din 3: previzualizarea**

După „Continuă”, Valet salvează alegerile și rulează o previzualizare. O vezi în ecranul Previzualizare, cu o notă albastră „Pasul 3 din 3”. Nimic nu se mută până nu apeși „Organizează fișierele”.

![Previzualizarea de la pasul 3](images/preview.png)

Proba Pro de 14 zile începe la prima pornire a lui Valet. Nu cere card și nu folosește internetul (vezi [secțiunea 18](#18-valet-pro-și-planul-gratuit)).

## 3. Previzualizarea

Ecranul Previzualizare are butonul „Rulează previzualizarea”. Previzualizarea citește folderele urmărite și listează ce ar face regulile. Nu schimbă niciun fișier.

După rulare vezi două numere: „Operații planificate” și „Fișiere sărite”. Dacă nu există nicio operație planificată, vezi „Nimic de organizat”. Rapoartele, dacă există, apar oricum.

### Tabelul „Ce se va face”

| Coloană | Ce înseamnă |
|---|---|
| Fișier | Numele fișierului sau al folderului. |
| Operație | `mutare`, `copiere` sau `în Coș`. |
| De unde | Calea de acum. |
| Unde | Calea de destinație. Pentru „în Coș” scrie „La Coș”. |
| Regula | Numele regulii care a decis. |

- Căile lungi sunt scurtate la mijloc. Calea întreagă apare când ții mouse-ul deasupra.
- Dacă numele de la destinație e deja ocupat, coloana „Unde” arată deja numele nou, de exemplu `raport (2).pdf`.
- Sub numere poate apărea „Reguli ignorate pentru fișiere deja planificate: N”. Înseamnă că un fișier se potrivește cu mai multe reguli. Se aplică prima regulă din lista Reguli care mută, copiază sau trimite la Coș. Celelalte sunt ignorate pentru acel fișier.
- Un fișier primește cel mult o operație.

### Fișierele sărite

„Fișiere sărite: N” are butonul „Arată”. Tabelul are coloanele Fișier, Motiv și Regulă. Fișierele pe care nicio regulă nu le vrea nu apar acolo.

| Motiv afișat | Ce înseamnă |
|---|---|
| Exclus (folder de sistem, AppData, .git, node_modules etc.) | Folderul e exclus implicit sau conține un fișier `.valetignore`. E o singură linie pentru tot folderul. |
| Fișier ascuns sau de sistem | Numele începe cu punct sau are atributul ascuns sau de sistem din Windows. Regula poate cere „Include fișierele ascunse”. |
| Legătură (symlink sau joncțiune) | Valet nu urmează și nu mută legături. |
| Se scrie încă sau e blocat | Fișierul are extensia `.crdownload`, `.part`, `.tmp`, `.download` sau `.partial`, numele începe cu `~$`, sau a fost modificat în ultimele 10 secunde. |
| Fișier OneDrive/Dropbox la cerere, nedescărcat | Fișierul e doar în cloud. Valet nu îl deschide, ca să nu îl descarce. |
| Altă regulă se aplică înaintea acesteia | Fișierul se potrivește cu mai multe reguli. Apare ca rând doar dacă fișierul nu a primit nicio operație. |
| E deja la destinație | Fișierul e chiar în folderul în care ar fi mutat. |
| Fișierul nu are extensie; regula folosește {ext} | Destinația regulii conține `{ext}`, iar fișierul nu are extensie. |
| Destinația regulii nu e validă | Folderul destinație nu se poate forma: variabilă de mediu nesetită, simbol necunoscut, folder lipsă sau destinație într-un folder exclus ori sub un `.valetignore`. |
| Acțiunea regulii nu e suportată încă | Regula are o acțiune pe care Valet nu o execută (de exemplu `rename` sau `tag`). Regulile noi din aplicație nu pot avea așa ceva. |
| Nu poate fi citit | Valet nu a putut citi folderul sau fișierul, sau nu a putut calcula amprenta pentru o regulă de duplicate. |
| Folder gol păstrat: e destinația unei reguli | Un folder gol pe care o regulă l-ar trimite la Coș, dar o altă regulă mută fișiere în el sau sub el. Exemplu: un folder `Audio` încă gol. |

Un fișier blocat de alt program nu apare aici. Blocajul se vede abia la „Organizează”, ca eroare (vezi [secțiunea 4](#4-organizează)).

### Rapoarte

Regulile cu acțiunea „Doar raport” nu fac nicio operație. Fișierele lor apar sub „Rapoarte”, cu textul „Doar informație: nimic nu se întâmplă cu aceste fișiere.”

- **Fișiere mari:** un tabel cu calea și mărimea fiecărui fișier găsit. Presetarea Fișiere mari ia în calcul fișierele de peste 1 GB, adică peste 1.073.741.824 de octeți.
- **Duplicate:** grupe de fișiere cu conținut identic: „Grupul 1, 12 MB fiecare”, urmat de căile fișierelor. Dacă o regulă trimite duplicatele la Coș, copia care se păstrează are eticheta „se păstrează”. E cea mai veche după data modificării. La egalitate se alege calea mai scurtă, apoi prima în ordine alfabetică.
- Un fișier poate apărea și într-un raport, și în tabelul de operații. De exemplu, o arhivă mare e mutată de „Descărcări după tip” și listată de „Fișiere mari”.
- Fișierele goale nu sunt duplicate. Fișierele ascunse nu intră în raportul de duplicate.
- Pentru duplicate, Valet citește conținutul numai pentru fișierele care au aceeași mărime ca un alt candidat, și niciodată pentru fișiere OneDrive sau Dropbox nedescărcate.

### Cât timp e valabilă o previzualizare

- Previzualizarea e păstrată doar în memorie. Se folosește o singură dată.
- Se pierde dacă rulezi alta, dacă o golești („Golește rezultatul”) sau dacă repornești aplicația. Dacă apeși „Organizează” pe una pierdută, vezi „Previzualizarea a expirat. Rulează din nou previzualizarea.”
- Cât timp o previzualizare e deschisă, organizarea automată așteaptă (vezi [secțiunea 9](#9-organizarea-automată)).

## 4. Organizează

Butonul „Organizează fișierele (N)” apare când previzualizarea are cel puțin o operație. N e numărul de operații.

- Valet execută exact lista pe care o vezi. Nu face o listă nouă, deci fișierele apărute după previzualizare nu sunt atinse.
- Înainte de fiecare fișier, Valet verifică din nou: fișierul există, are aceeași mărime și aceeași dată de modificare ca în previzualizare, nu e un fișier la cerere și nu e blocat.
- O operație eșuată nu oprește restul lotului.
- Mutare pe același disc: fișierul e mutat direct. Valet nu înlocuiește niciun fișier existent.
- Mutare între două discuri: Valet copiază fișierul, compară copia cu originalul prin amprentă, apoi trimite originalul la Coșul de reciclare. Dacă ceva nu se potrivește, copia merge la Coș și originalul rămâne unde era.
- La „în Coș” pentru un folder, Valet verifică din nou, chiar înainte de operație, că folderul e încă gol. Dacă între timp a apărut ceva în el, operația eșuează și folderul rămâne.
- La final apare „Operații făcute: N.” sau, dacă au fost erori, „Făcute: X. Eșuate: Y. Detalii mai jos.” cu un tabel al operațiilor eșuate.
- Apăsarea lui „Organizează” aprobă și regulile din acea previzualizare pentru organizarea automată (vezi [secțiunea 9](#9-organizarea-automată)).
- După lot apare o notificare scurtă, de exemplu „5 fișiere organizate”. Se poate opri în Setări.

![După „Organizează”: caseta „Ultimul lot” cu butonul „Anulează lotul”](images/after-organize.png)

### Mesaje de eroare

| Mesaj | Ce înseamnă |
|---|---|
| Fișierul nu mai există | Fișierul a dispărut între previzualizare și operație. |
| Fișierul s-a schimbat de la previzualizare | Mărimea sau data s-au schimbat, nu mai e un fișier obișnuit, sau un folder nu mai e gol. Nu s-a atins nimic. |
| Fișier la cerere, nedescărcat | Fișier OneDrive sau Dropbox doar în cloud. |
| Fișierul e blocat de alt program | Alt program ține fișierul deschis. Nu s-a atins nimic. |
| Volumul nu are Coș de reciclare; Valet nu mută și nu copiază aici. | Vezi [secțiunea 13](#13-discuri-fără-coș-de-reciclare). |
| Verificarea copiei a eșuat | Copia nu seamănă cu originalul. Originalul rămâne, copia merge la Coș. |
| Nu s-a putut trimite în Coșul de reciclare | Windows a refuzat. Fișierul rămâne unde era. |
| Destinația e ocupată | Cod păstrat în jurnal. La anulare, când un nume se ocupă chiar în acel moment, Valet cere confirmare în loc să arate această eroare. |
| Întrerupt de o oprire a aplicației | Operația a rămas neterminată după o oprire. Vezi [secțiunea 6](#6-istoric). |
| Cale nevalidă | Calea nu e text Unicode valid sau nu are folder părinte. |
| Eroare de citire sau scriere | Altă eroare a discului sau lipsă de drepturi (de exemplu un folder protejat). Valet nu încearcă din nou. |

După unele mesaje poate apărea între paranteze o notă tehnică, în engleză.

## 5. Anularea unui lot

După un lot, caseta „Ultimul lot” apare sus, pe fiecare ecran, cu butonul „Anulează lotul”. Rămâne până îl anulezi sau până apare un lot nou. Dacă repornești aplicația, Valet o arată din nou pentru cel mai nou lot de organizare, dacă mai are operații făcute.

![Confirmarea anulării](images/undo-confirm.png)

Pașii:

1. Apeși „Anulează lotul”. Vezi lista „Fișiere de mutat înapoi: N”.
2. Apeși „Confirmă anularea” (sau „Renunță”).
3. Vezi „Rezultatul anulării”: „Operații anulate”, plus, dacă e cazul, „Rămase neanulate (fără confirmare)”, „Sărite (nu erau făcute în lot)” și „Eșuate”.

Anularea merge pe lot întreg. Nu există anulare pentru un singur fișier. Dacă vrei un singur fișier înapoi, Istoricul îți arată unde a ajuns și îl poți muta tu.

Orice lot de organizare terminat se poate anula și din Istoric, nu doar ultimul. Asta e valabil în Pro și în proba de 14 zile. În planul Gratuit se poate anula doar cel mai nou lot de tip „Organizare”; despre cum se numără loturile, vezi [secțiunea 18](#18-valet-pro-și-planul-gratuit).

### Ce se întâmplă în fiecare caz

| Situație | Ce face Valet |
|---|---|
| Fișierul de la destinație e neschimbat, iar locul inițial e liber | Îl mută înapoi. |
| Fișierul a fost schimbat după lot (conținutul diferă) | Nu mută nimic. Fișierul apare la „Fișiere care cer confirmare” cu „Fișierul s-a schimbat după lot”. Cu „Anulează oricum” e mutat înapoi așa cum e acum. |
| Locul inițial e ocupat (un fișier cu același nume există din nou acolo) | Nu mută nimic. Apare „Locul inițial e ocupat; fișierul va primi un nume nou”. Cu „Anulează oricum”, fișierul e pus lângă cel existent, ca `nume (2).ext`. Rezultatul spune „X restaurat ca Y”. Fișierul existent nu e atins. |
| Fișierul a fost trimis la Coș (regulă „La Coș”) | Valet îl restaurează din Coșul de reciclare la locul lui. În listă apare „restaurare din Coș”. Se restaurează și folderele goale trimise la Coș. |
| Fișierul trimis la Coș nu mai e în Coș (l-ai golit) | Anularea eșuează pentru acel fișier cu „Fișierul nu mai există”. |
| Fișier trimis la Coș, iar locul inițial e ocupat | Nu se restaurează nimic. Lista de confirmare scrie „Locul inițial e ocupat; elementul nu poate fi restaurat și rămâne în Coșul de reciclare. Mută sau redenumește fișierul de acolo, apoi anulează din nou.” Și cu „Anulează oricum”, Valet nu restaurează lângă fișierul existent: elementul rămâne în Coș, iar rezultatul spune „Locul inițial e ocupat; elementul a rămas în Coșul de reciclare”. Poți muta sau redenumi fișierul care încurcă și anula din nou, sau poți restaura tu elementul din Coș. |
| Fișier restaurat din Coș cu conținut diferit de cel de la trimitere | Rămâne restaurat și e listat: „restaurat, dar conținutul diferă de cel de la momentul trimiterii la Coș”. |
| Fișierul era o copie (regulă „Copiază în”) | Copia e trimisă la Coș. Dacă originalul nu mai există, copia e ultima, deci Valet cere confirmare: „Originalul nu mai există; anularea trimite această copie în Coșul de reciclare”. |
| Fișierul de la destinație a dispărut, e blocat sau e la cerere | Eșuează pentru acel fișier, cu motivul. |
| Operații care au eșuat în lotul inițial | Nu sunt atinse. Apar la „Sărite”. |
| Foldere create de lot | Rămân, goale. Anularea nu le trimite la Coș. |
| Anulare între două discuri | Cere Coș de reciclare pe ambele. Altfel eșuează cu „Volumul nu are Coș” și fișierul rămâne unde e. |

Restaurarea din Coșul de reciclare Windows a fost testată pe mașina Windows a testelor automate (CI), nu pe Coșul de reciclare al unui utilizator real (vezi [SAFETY.md](SAFETY.md)). Dacă anularea eșuează pentru un fișier trimis la Coș, deschide Coșul de reciclare și restaurează-l cu mâna.

Anularea e un lot nou, de tip „Anulare”. Un lot de tip „Anulare” sau „Recuperare după oprire” nu se poate anula. A doua anulare a aceluiași lot nu mai găsește nimic de făcut.

### Restaurare din Coșul de reciclare, cu mâna

Fișierele trimise la Coș de Valet sunt în Coșul de reciclare Windows obișnuit. Le poți restaura de acolo ca pe orice fișier, cu clic dreapta și opțiunea de restaurare. Pentru fișierele trimise la Coș de pasul de recuperare (vezi [secțiunea 6](#6-istoric)), Valet nu are anulare. Acolo restaurarea din Coș se face cu mâna.

## 6. Istoric

Ecranul Istoric listează loturile, cele mai noi primele. Se văd cel mult 50. „Reîncarcă” reîncarcă lista. În planul Gratuit se văd doar loturile din ultimele 7 zile, iar ecranul scrie „Planul Gratuit arată ultimele 7 zile. Istoricul complet rămâne salvat.” Nimic nu se șterge din jurnal din cauza planului.

![Istoricul](images/history.png)

Lista are coloanele Tip, Data și Operații.

- **Tip:** „Organizare”, „Anulare” sau „Recuperare după oprire”. Un lot rămas deschis are „(neterminat)”.
- **Operații:** câte operații sunt în fiecare stare: făcută, eșuată, anulată, în curs, planificată.

Apeși pe un lot și vezi detaliile: identificatorul lotului, textul „Ce s-a întâmplat” și tabelul operațiilor.

- Coloanele tabelului sunt Nr., Operație, De la, La, Stare și Eroare.
- Operația poate fi `mutare`, `copiere`, „Trimis la Coș” sau „Restaurat din Coș”.
- La loturile de organizare terminate, cu operații făcute, apare butonul „Anulează lotul”. Funcționează ca în [secțiunea 5](#5-anularea-unui-lot).
- „Anulate ulterior: N” înseamnă că operațiile au fost anulate de un lot de anulare.

### Recuperare după oprire

Dacă Valet sau calculatorul se oprește în mijlocul unui lot, unele operații rămân neterminate. La pornirea următoare, înainte să pornească urmărirea folderelor, Valet verifică fiecare operație neterminată, în funcție de ce găsește pe disc. Nu te întreabă.

- Operația care nu a pornit e marcată eșuată. Nu s-a atins nimic.
- Operația aflată la jumătate e terminată sau abandonată, după starea fișierelor. O copie temporară rămasă (`.valet-tmp-…`) merge la Coș.
- Rezultatul e un lot „Recuperare după oprire”, cu explicația fiecărei decizii la „Detalii tehnice” (în engleză).
- Lotul oprit e închis. În detalii apare „Lotul a fost întrerupt și încheiat la pornirea următoare.”
- Scopul e ca fiecare fișier să rămână într-un singur loc, la sursă sau la destinație.

Ce s-a verificat: teste automate, inclusiv unul cu 100 de iterații, în care procesul e oprit forțat la un moment aleator din mijlocul unui lot, apoi rulează recuperarea. Testul rulează pe Linux, cu un Coș simulat. Nu s-a testat o pană de curent. Două cazuri rare, după o dublă eroare, pot lăsa un fișier și la sursă, și la destinație. Fișierul nu se pierde. Operația apare eșuată, cu ambele căi în coloana Eroare. Detalii în [SAFETY.md](SAFETY.md), invariantul 3.

## 7. Cele 8 presetări

O presetare e un set de reguli gata făcute. Se aleg la prima pornire, iar mai târziu poți instala altele cu „Adaugă presetări” din ecranul Reguli (vezi [secțiunea 8](#8-reguli-proprii)). Presetările instalate apar în ecranul Reguli, cu eticheta „presetare”, și le poți opri, edita sau șterge ca pe orice regulă.

Regulile unei presetări se aplică într-un folder doar dacă acel folder e în lista folderelor urmărite. De exemplu, dacă Desktop nu e urmărit, „Desktop curat” nu face nimic. Dacă urmărești `C:\Users\nume` în loc de Descărcări, presetările pentru Descărcări nu se potrivesc.

| Presetare | Ce face | Foldere | Coș | Bifată |
|---|---|---|---|---|
| Descărcări după tip | Mută fișierele în subfoldere ale Descărcărilor, după tip: Imagini, Documente, Arhive, Instalatoare, Video, Audio. Doar fișierele direct în Descărcări, nu cele din subfoldere. | Descărcări | nu | da |
| Capturi de ecran | Mută imaginile cu Screenshot, Captură, Snip sau Clipboard în nume în Imagini\Capturi de ecran\AAAA. | Descărcări, Desktop, Imagini | nu | da |
| Facturi și chitanțe | Mută PDF-urile cu factură, chitanță, invoice, receipt sau bon în nume în Documente\Facturi\AAAA. | Descărcări | nu | da |
| Fișiere mari | Listează fișierele de peste 1 GB. Nu mută nimic. | Descărcări, Documente | nu | da |
| Duplicate | Listează fișierele cu conținut identic. Nu mută nimic. | Descărcări | nu | nu |
| Desktop curat | Mută fișierele de pe Desktop modificate acum mai mult de 7 zile în Documente\De pe Desktop\AAAA-LL. Scurtăturile (`.lnk`, `.url`) și `.ini` rămân. | Desktop | nu | nu |
| Instalatoare vechi | Trimite la Coș instalatoarele (`.exe`, `.msi`, `.msix`, `.msixbundle`, `.appx`, `.appxbundle`) modificate acum mai mult de 30 de zile. | Descărcări | **da** | nu |
| Foldere goale | Trimite la Coș folderele goale modificate acum mai mult de 7 zile. | Descărcări | **da** | nu |

Note:

- Nicio presetare care trimite la Coș nu e bifată din oficiu.
- „Descărcări după tip” are 6 reguli, câte una pentru fiecare tip. Tipurile se stabilesc după extensie:
  - Imagini: jpg, jpeg, png, gif, bmp, webp, tif, tiff, heic, heif, avif, svg, ico, psd, raw, cr2, nef, arw, dng.
  - Documente: pdf, doc, docx, xls, xlsx, ppt, pptx, odt, ods, odp, rtf, txt, md, csv, epub.
  - Arhive: zip, rar, 7z, tar, gz, tgz, bz2, xz, zst, cab, iso.
  - Instalatoare: exe, msi, msix, msixbundle, appx, appxbundle.
  - Video: mp4, mkv, avi, mov, wmv, webm, m4v, mpg, mpeg, flv, 3gp.
  - Audio: mp3, wav, flac, aac, ogg, m4a, wma, opus, aiff, mid, midi.
  - Fișierele de alt tip și cele fără extensie rămân pe loc.
- AAAA e anul și LL e luna din data modificării fișierului, nu data de azi.
- „Capturi de ecran” și „Facturi și chitanțe” caută cuvântul întreg în numele fișierului, fără să conteze literele mari și mici. `Screenshot 2024.png`, `Snip_1.jpg`, `clipboard.png`, `Factura_123.pdf`, `bon fiscal.pdf` și `Invoice2024.pdf` se potrivesc. `Snippet.png`, `screenshots-tutorial.png`, `bonus.pdf` și `refacturare.pdf` nu se potrivesc.
- „Facturi și chitanțe”, „Fișiere mari”, „Duplicate”, „Instalatoare vechi” și „Foldere goale” caută și în subfolderele folderelor lor. Celelalte presetări se uită doar la ce e direct în folder.
- „Foldere goale” nu atinge niciodată folderul urmărit însuși. Păstrează folderele goale care sunt destinații ale altor reguli, de exemplu un folder `Audio` încă gol.
- Ordinea de evaluare a presetărilor: Facturi și chitanțe, Capturi de ecran, Instalatoare vechi, Descărcări după tip, Desktop curat, Fișiere mari, Duplicate, Foldere goale. Cu „Instalatoare vechi” și „Descărcări după tip” pornite, un instalator mai vechi de 30 de zile merge la Coș, iar unul mai nou merge în Instalatoare.
- Regulile „doar raport” nu opresc alte reguli să mute același fișier.
- Dacă limba aplicației era engleza când s-au instalat presetările, numele folderelor sunt în engleză (Images, Documents, Archives, Installers, Videos, Music, Pictures\Screenshots, Documents\Invoices, Documents\From Desktop). Schimbarea limbii mai târziu nu redenumește regulile deja instalate. Când instalezi din nou o presetare, se folosește limba activă în acel moment.
- O presetare ștearsă, sau o regulă ștearsă dintr-o presetare, revine când instalezi presetarea din nou cu „Adaugă presetări”. Instalarea unei presetări deja instalate readuce toate regulile ei la versiunea livrată: modificările tale la acele reguli se pierd. O regulă schimbată astfel rulează automat doar după o nouă previzualizare aprobată (vezi [secțiunea 9](#9-organizarea-automată)).
- Regulile presetărilor sunt mereu listate după regulile tale (vezi „Ordinea regulilor” în [secțiunea 8](#8-reguli-proprii)).

## 8. Reguli proprii

Ecranul Reguli listează regulile. Pentru fiecare vezi:

- bifa „Activă”;
- numele, cu eticheta „presetare” la cele din presetări;
- un rezumat (condiții → acțiune);
- folderele în care se aplică („În toate folderele urmărite” sau „În: …”);
- butoanele „Editează” și „Șterge”. La ștergere Valet întreabă, iar fișierele nu sunt atinse.

![Ecranul Reguli](images/rules.png)

„Adaugă regulă” deschide editorul. Salvarea unei reguli nu atinge niciun fișier.

### Adaugă presetări

„Adaugă presetări”, sus în ecranul Reguli, deschide lista celor 8 presetări, în limba interfeței, fiecare cu descrierea, numărul de reguli și, unde e cazul, eticheta „trimite la Coș” sau „doar raport”. O presetare deja instalată are eticheta „instalată”.

- Bifează presetările de instalat și apasă „Instalează (N)”. Regulile lor se adaugă după regulile tale. Instalarea nu schimbă niciun fișier.
- Dacă bifezi o presetare deja instalată, panoul o listează la „Se reinstalează” și avertizează că regulile ei revin la versiunea livrată, deci modificările tale la ele se pierd.
- La final vezi „Presetări instalate: N.” și nota că regulile noi sau schimbate rulează automat doar după o previzualizare aprobată.
- „Renunță” închide panoul.

### Condițiile

Regula se aplică unui fișier doar dacă îndeplinește toate condițiile. Poți bifa și „Se aplică tuturor fișierelor (fără condiții)”.

| Condiție | Ce verifică |
|---|---|
| Extensie (una) | Extensia fișierului, de exemplu `pdf`. Literele mari și mici nu contează. |
| Extensie, una dintre | Extensia e în listă. |
| Extensie, alta decât | Extensia nu e în listă. Trec și fișierele fără extensie. |
| Tip de fișier | Imagini, Documente, Arhive, Instalatoare, Video, Audio sau Cod. Listele sunt în [secțiunea 7](#7-cele-8-presetări); „Cod” are rs, py, js, ts, html, css, json, xml, yaml, sql și altele. |
| Numele conține | Unul sau mai multe texte. E de ajuns să apară unul. Literele mari și mici nu contează. Se uită doar la numele fișierului. |
| Numele se potrivește cu expresia | O expresie regulată pe numele fișierului. Ține cont de litere mari și mici, afară de cazul în care începe cu `(?i)`. O expresie greșită primește mesajul „Expresia regulată nu e validă.” |
| Calea conține | Un text oriunde în calea întreagă. |
| Mai mare de, Mai mic de | Mărimea, în KB, MB sau GB (1 KB = 1024 de octeți). |
| Modificat acum mai mult de (zile) | Au trecut mai mult de N zile întregi de la ultima modificare. N e între 0 și 36500. |
| Creat acum mai mult de (zile) | La fel, după data creării. Dacă sistemul nu dă data creării, condiția nu se îndeplinește. |
| Folder gol | Regula vede doar foldere fără nimic în ele. Nu se combină cu condiții despre fișiere (extensie, nume, tip, mărime, duplicat). Acțiunea poate fi doar „La Coș” sau „Doar raport”. |
| Duplicat (același conținut) | Fișierul are o copie identică printre fișierele pe care le acceptă celelalte condiții. Acțiunea poate fi doar „La Coș” sau „Doar raport”. Cu „La Coș” merg la Coș toate copiile în afară de cea mai veche. |

### Acțiunile

Fiecare regulă are o singură acțiune.

| Acțiune | Ce face |
|---|---|
| Mută în | Mută fișierul în folderul destinație. Numele fișierului rămâne. |
| Copiază în | Pune o copie în folderul destinație. Originalul rămâne. |
| La Coș | Trimite fișierul sau folderul gol la Coșul de reciclare, de unde se poate recupera. Nu șterge definitiv. |
| Doar raport | Fișierul apare în previzualizare la „Rapoarte”. Nu se întâmplă nimic cu el. |

### Folderul destinație și simbolurile

Destinația e un folder, nu un fișier. Poți scrie:

- o cale întreagă, de exemplu `D:\Arhiva`;
- o cale fără literă de disc, de exemplu `Imagini`. Ea e socotită față de folderul urmărit în care e fișierul;
- `~` la început (folderul personal) sau o variabilă Windows între semnele `%`, de exemplu `%USERPROFILE%`;
- un simbol de folder ca prim element al căii.

Simboluri de folder, doar la început: `{downloads}` (Descărcări), `{documents}` (Documente), `{pictures}` (Imagini), `{desktop}` (Desktop), `{home}` (folderul personal). Merg și `{videos}` și `{music}`, dacă le scrii cu mâna. Valet ia folderele așa cum le știe Windows pentru contul tău.

Simboluri de dată și extensie, oriunde în cale:

| Simbol | Devine |
|---|---|
| `{year}` | Anul din data modificării fișierului, de exemplu `2026`. |
| `{month}` | Luna, `01`–`12`. |
| `{day}` | Ziua, `01`–`31`. |
| `{ext}` | Extensia fișierului, cu litere mici. Un fișier fără extensie e sărit. |

Exemple: `{documents}/Facturi/{year}` devine `Documente\Facturi\2026`. `{pictures}/{year}-{month}` devine `Imagini\2026-05`.

Orice alt text între acolade e refuzat la salvare, cu mesajul „Destinația nu e validă. Folosește doar simbolurile din listă, de exemplu {year}.” Valet refuză și o destinație într-un folder exclus (vezi [secțiunea 12](#12-ce-nu-atinge-valet-niciodată)).

### Foldere și opțiuni

- **Foldere:** alegi în ce foldere urmărite se aplică regula. Dacă nu bifezi nimic, se aplică în toate. O regulă se aplică într-un folder doar dacă folderul e chiar cel urmărit, nu unul de deasupra lui.
- **Include fișierele ascunse:** regula poate lua și fișiere ascunse. Folderele ascunse rămân sărite oricum.
- **Doar fișierele direct în folder, fără subfoldere:** regula nu coboară în subfoldere.

### Ordinea regulilor

Valet parcurge regulile în ordinea din lista Reguli. Primul fișier care se potrivește cu o regulă care mută, copiază sau trimite la Coș primește acea operație. Regulile următoare sunt ignorate pentru el. Regulile „Doar raport” se adaugă oricum.

- Lista arată întâi regulile tale, în ordinea în care le-ai creat, apoi regulile presetărilor, în ordinea de evaluare din [secțiunea 7](#7-cele-8-presetări). O regulă pe care o creezi merge după celelalte reguli ale tale și înaintea oricărei reguli de presetare, indiferent dacă o creezi înainte sau după instalarea presetărilor. Deci, când regula ta și o presetare se potrivesc cu același fișier, decide regula ta.
- O regulă editată rămâne pe locul ei.
- Nu există buton pentru a muta o regulă mai sus.
- Ca să decidă o presetare în locul regulii tale, oprește regula ta din bifa „Activă” sau editeaz-o. Previzualizarea arată în coloana „Regula” ce regulă a decis.

### Exemple

Exemplele folosesc un folder adăugat de tine în Setări, ca să nu se bată cap în cap cu presetările.

1. **PDF-uri scanate, pe ani.** Folderul urmărit: `C:\Users\nume\Scanari`. Regula „Scanări PDF”: condiția „Extensie (una)” = `pdf`, acțiunea „Mută în” `{documents}/Scanari/{year}`, folder: doar `Scanari`.
2. **Arhive vechi la Coș.** Condițiile: „Tip de fișier” = Arhive și „Modificat acum mai mult de” 90 de zile. Acțiunea: „La Coș”. Folder: `Scanari`. Rulează întâi o previzualizare și citește lista.
3. **Videoclipuri mari, doar raport.** Condițiile: „Tip de fișier” = Video și „Mai mare de” 500 MB. Acțiunea: „Doar raport”. Presetările nu o încurcă, pentru că regulile de raport nu opresc alte reguli.

### După salvare

O regulă nouă sau modificată primește o versiune nouă. Orice schimbare o schimbă, inclusiv bifa „Activă”. Până la o previzualizare aprobată, organizarea automată nu rulează operațiile ei (vezi [secțiunea 9](#9-organizarea-automată)). Ștergerea unei reguli îi șterge și aprobarea.

### Planul și regulile

- În planul Gratuit pot fi active cel mult 3 reguli proprii în același timp. Regulile din presetări nu se numără. Dacă bifezi „Activă” la a patra regulă proprie sau salvezi una nouă, activă, peste limită, Valet refuză cu „Gratuit permite 3 reguli proprii active. Dezactivează una sau treci la Pro.” Nu dezactivează nimic pe tăcute. Poți oricând opri, edita sau șterge o regulă.
- Butoanele „Exportă regulile” și „Importă reguli” din ecranul Reguli sunt incluse în Pro și în probă. În planul Gratuit ecranul scrie „Exportul și importul de reguli sunt incluse în Pro și în probă.” Exportul scrie un fișier nou și refuză un fișier care există deja. Regulile importate sunt reguli noi; ca orice regulă nouă, rulează automat doar după o previzualizare aprobată.
- Detalii și ce se întâmplă când treci pe Gratuit: [secțiunea 18](#18-valet-pro-și-planul-gratuit).

## 9. Organizarea automată

Valet poate organiza singur fișierele noi din folderele urmărite. E oprită din oficiu.

### Cum o pornești

1. Rulează o previzualizare și apasă „Organizează fișierele”. Asta e „previzualizarea aprobată”.
2. În Setări, bifează „Aplicare automată în folderele urmărite”. Până la prima previzualizare aprobată, bifa e inactivă și scrie „Disponibil după ce aprobi prima previzualizare.”

Butonul „Organizează” apare doar când previzualizarea are cel puțin o operație.

### Ce reguli rulează singure

Doar regulile aprobate la versiunea lor de acum, și doar pentru fișierele din folderele urmărite pe care le-a acoperit previzualizarea care le-a aprobat. O regulă e aprobată când apeși „Organizează” pe o previzualizare care o conținea: regula avea o operație, o linie de raport sau un fișier sărit în acea previzualizare. Aprobarea acoperă toate folderele urmărite pe care le-a planificat acea previzualizare, adică toate folderele urmărite care existau când ai apăsat „Rulează previzualizarea”, chiar dacă niciun fișier din ele n-a fost listat. Regula nouă, regula modificată (inclusiv oprită și repornită) sau regula care n-a avut nimic de făcut în previzualizare așteaptă o previzualizare nouă. Operațiile ei se văd la următoarea previzualizare manuală. La fel și operațiile dintr-un folder urmărit adăugat după ultima previzualizare aprobată (vezi „Foldere noi și foldere eliminate” mai jos).

### Cum lucrează

- Valet planifică folderele ca la o previzualizare și execută doar operațiile regulilor aprobate.
- Fiecare operație trece prin aceleași verificări ca la „Organizează”. Se scrie în jurnal înainte, se verifică din nou fișierul înainte, nu se suprascrie nimic.
- Primești o notificare scurtă. Caseta „Ultimul lot” are mențiunea „organizat automat” și butonul „Anulează lotul”. Lotul apare și în Istoric.

### Când rulează

- La schimbări în folderele urmărite: după ce se liniștesc 2 secunde, plus cel mult 2 secunde ca să se adune schimbările apropiate.
- La fiecare 10 minute, o verificare completă a tuturor folderelor urmărite. Prinde și schimbările pe care Windows nu le-a raportat.
- La pornirea aplicației și după ce schimbi lista de foldere, la câteva secunde.
- La aproximativ 12 secunde după o trecere declanșată de schimbări care a sărit un fișier ca „se scrie încă”, de exemplu o descărcare tocmai terminată.

### Când așteaptă

- Cât timp e deschisă o previzualizare: din clipa în care apeși „Rulează previzualizarea” până apeși „Organizează fișierele”, „Golește rezultatul” sau părăsești ecranul Previzualizare. Dacă o lași deschisă, organizarea automată pornește din nou după 15 minute.
- Cât timp e în pauză din zona de notificare (vezi [secțiunea 10](#10-zona-de-notificare-și-pornirea-cu-windows)).
- Cât timp aplicarea automată e oprită în Setări sau nu ai aprobat nicio previzualizare.
- Cât timp rulează un lot manual sau o anulare. Pașii nu se suprapun.

### Fișiere blocate

Dacă un fișier e blocat de alt program într-o trecere automată, Valet îl încearcă din nou la următoarele 3 treceri. Dacă e tot blocat, îl lasă în pace până apare o schimbare în el sau în folderul lui. Contorul stă doar în memorie și se reia la repornire. Trecerile automate au loc la schimbări și la fiecare 10 minute, deci reîncercările pot fi la 10 minute distanță.

### Foldere noi și foldere eliminate

Aprobarea ține de o regulă, la o versiune, și de folderele urmărite ale previzualizării care a aprobat-o.

- Un folder urmărit pe care îl adaugi, inclusiv unul eliminat mai devreme și adăugat din nou, nu se organizează automat până nu rulezi o previzualizare și nu apeși „Organizează”. Până atunci Valet lasă fișierele din el unde sunt, chiar și pentru regulile aprobate înainte. Operațiile lor se văd la următoarea previzualizare manuală.
- După ce adaugi un folder în Setări, o notă scrie „Folderul nou se organizează automat doar după o previzualizare aprobată.” și are butonul „Deschide Previzualizarea”.
- Eliminarea unui folder din listă îi șterge aprobările. Folderele rămase le păstrează pe ale lor.
- Dacă un folder urmărit e în interiorul altuia, un fișier din folderul interior are nevoie de aprobare sub amândouă.
- Butonul „Organizează” apare doar când previzualizarea are cel puțin o operație. Un folder în care nu e încă nimic de organizat se aprobă printr-o previzualizare ulterioară care are ceva de făcut.
- Un folder urmărit indisponibil (de exemplu un disc extern scos) e sărit cât lipsește, își păstrează aprobările și e urmărit din nou când apare, în cel mult 10 minute (vezi [secțiunea 11](#11-setări)).
- Un `approvals.json` scris de o versiune mai veche de Valet nu reținea foldere, deci nu aprobă niciun folder. După actualizare, rulează o previzualizare și apasă „Organizează” o dată.

## 10. Zona de notificare și pornirea cu Windows

**Zona de notificare (tray)**

- Pictograma Valet apare lângă ceas. Clic stânga arată fereastra. Clic dreapta deschide meniul.
- Meniul are „Arată Valet”, „Pauză organizare automată” (devine „Reia organizarea automată”) și „Ieșire”.
- Închiderea ferestrei nu oprește Valet. Fereastra se ascunde, iar urmărirea folderelor și organizarea automată continuă. Valet se oprește doar din „Ieșire”.
- Pauza ține până reiei sau până repornești aplicația. Nu se salvează.
- Dacă pornești Valet a doua oară, nu pornește a doua copie. Se arată fereastra primei.

**Pornirea cu Windows**

- În Setări, bifa „Pornește Valet odată cu Windows” adaugă o intrare în registrul utilizatorului curent (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`). Nu cere drepturi de administrator și nu atinge alte conturi.
- Valet pornește cu argumentul `--minimized`, deci rămâne ascuns în zona de notificare.
- Dacă dezactivezi intrarea din Managerul de activități, fila „Startup”, Setările arată bifa debifată. Dacă o bifezi din nou, intrarea se reactivează.
- În versiunea din Microsoft Store nu există această bifă. Setările scriu „În versiunea din Microsoft Store, pornirea odată cu Windows se setează din setările Windows.” și au butonul „Deschide setările Windows”, care deschide lista Windows de aplicații la pornire. Acolo pornești sau oprești Valet (vezi [secțiunea 18](#18-valet-pro-și-planul-gratuit)).

## 11. Setări

![Setări](images/settings.png)

Captura arată ecranul Setări așa cum era înainte să fie adăugate secțiunile Valet Pro, Actualizări, Asistență și Despre (descrise mai jos). Ecranul are, de sus în jos: folderele urmărite, opțiunile generale (limba, notificarea, pornirea cu Windows, aplicarea automată), Valet Pro, Actualizări, Asistență și Despre.

- **Foldere urmărite:** lista folderelor. Scrii calea și apeși „Adaugă”, sau apeși „Elimină”. Valet refuză un disc întreg și un folder cu un nume exclus în cale (de exemplu `AppData`, `Program Files`, `.git`, `node_modules`). Un folder care nu există acum e primit (vezi „Foldere indisponibile”). Un folder adăugat nu se organizează automat până nu aprobi o previzualizare (vezi [secțiunea 9](#9-organizarea-automată)); o notă cu butonul „Deschide Previzualizarea” o spune. În planul Gratuit pot fi active cel mult 2 foldere; al treilea nu se adaugă, cu mesajul „Gratuit permite 2 foldere urmărite active. Elimină unul sau treci la Pro.” (vezi [secțiunea 18](#18-valet-pro-și-planul-gratuit)).
- **Foldere indisponibile:** un folder din listă care nu există în acest moment, de exemplu pentru că discul extern e scos, se salvează și apare cu textul „Indisponibil acum (de exemplu, disc deconectat). Se reia automat când apare.” Are butonul „Elimină”, ca orice folder. Cât lipsește, Valet nu-l urmărește și nu-l planifică; celelalte foldere merg ca de obicei, iar fișierele din el nu sunt atinse. Dacă lipsesc toate folderele, previzualizarea scrie „Niciun folder urmărit nu există pe disc. Adaugă unul în Setări.” O previzualizare manuală citește discul de fiecare dată, deci vede folderul imediat ce a apărut. Organizarea automată citește lista din nou înainte de fiecare verificare completă (la 10 minute), deci îl reia în cel mult 10 minute, fără repornire. Folderul își păstrează aprobările. S-a testat cu foldere simulate, nu cu un disc extern real.
- **Foldere neurmărite:** aceleași verificări de disc întreg și de nume exclus se fac și când Valet citește `config.json`, deci și un folder adăugat acolo cu mâna e verificat. O intrare care e un disc întreg sau are un nume exclus în cale nu e urmărită și nu e planificată. Rămâne în fișier. Setările o listează cu „Nu e urmărit:” și motivul, fără buton „Elimină”, și arată „Elimină folderele neurmărite”. Cât timp o asemenea intrare e în listă, „Adaugă” și celelalte butoane „Elimină” sunt inactive; un singur buton le elimină pe toate odată.
- **Limba:** Română sau English. Se schimbă imediat. Regulile și presetările deja instalate își păstrează numele și folderele.
- **Notificare scurtă după un lot:** pornită din oficiu.
- **Pornește Valet odată cu Windows:** vezi [secțiunea 10](#10-zona-de-notificare-și-pornirea-cu-windows).
- **Aplicare automată în folderele urmărite:** vezi [secțiunea 9](#9-organizarea-automată).
- **Valet Pro:** planul curent, tabelul „Ce include fiecare plan”, prețul, butoanele „Cumpără Pro”, „Am o licență” și „Am licența ca text”, iar când există o licență pe acest calculator, „Verifică licența acum” și „Scoate licența de pe acest calculator”. Tot ce face fiecare e în [secțiunea 18](#18-valet-pro-și-planul-gratuit). În partea de sus a ferestrei, un indicator mic arată „Gratuit” sau „Probă Pro: N zile”; un clic pe el deschide această secțiune. Pe Pro nu apare indicatorul.
- **Actualizări:** arată „Versiunea instalată: …”, lista „Canal de actualizare” (Stabil sau Beta; „Beta primește versiunile noi mai devreme, înainte să fie marcate stabile.”), caseta „Verifică automat actualizările” și butonul „Verifică acum”. Rezultatul e „Ai cea mai nouă versiune.” sau „Versiunea … e disponibilă.”, cu data publicării și „Ce s-a schimbat” (textul notelor de versiune, afișat ca text simplu). „Instalează și repornește” cere o confirmare („Valet se va reporni. Nicio operație nu e întreruptă: instalarea așteaptă lotul în curs.”); apoi apare progresul descărcării. Dacă un lot rulează, Valet scrie „Aștept să se termine lotul în curs.” și reîncearcă singur; „Renunță” oprește așteptarea. Dacă verificarea automată găsește o versiune, apare o notificare, un semn „Actualizare disponibilă” pe butonul Setări și, pe celelalte ecrane, o notă cu butonul „Vezi în Setări”. Valet instalează doar după ce apeși și confirmi. Mai multe în [secțiunea 17](#17-instalare-actualizări-și-dezinstalare).
  - Dacă versiunea ta nu are actualizările configurate (cum e cazul la o pornire din codul sursă), „Verifică acum” răspunde „Actualizările nu sunt configurate în această versiune.”
  - Fără internet, răspunsul e „Serverul de actualizări nu a putut fi contactat. Verifică conexiunea la internet.”
  - „Verifică automat actualizările” e bifată din oficiu. Textul de sub ea: „La pornire, apoi o dată la 24 de ore. „Verifică acum” funcționează oricum.” Debifată, Valet nu mai face nicio verificare automată și nu mai arată notificări de versiune nouă; „Verifică acum” merge ca înainte. Se aplică fără repornire (Valet citește setarea din oră în oră). Nu oprește verificarea licenței, care are loc doar dacă ai importat o licență Pro.
  - Dacă o versiune nouă nu e acoperită de licența Pro (actualizările incluse s-au terminat înainte de data publicării ei), confirmarea de la „Instalează și repornește” scrie asta și spune că versiunea nouă va rula ca Gratuit. O poți instala oricum; versiunea de acum rămâne Pro. Vezi [secțiunea 18](#18-valet-pro-și-planul-gratuit).
  - În versiunea din Microsoft Store, secțiunea arată doar versiunea instalată și textul „Actualizările vin din Microsoft Store.” Nu are canal, casetă sau buton de verificare.
- **Asistență:** caseta „Jurnal detaliat (pentru depanare)”. Pornită, jurnalul scrie mai multe linii, care pot conține căi de fișiere; rămân pe calculatorul tău. Se aplică pe loc, fără repornire. Unde e jurnalul: [secțiunea 15](#15-unde-sunt-datele). Aplicația nu are un buton care să deschidă folderul de jurnal.
- **Despre:** o linie cu versiunea, licența și adresa depozitului: „Valet … · Licență MIT · github.com/OsztrovszkyVlad/Valet”.

Fiecare schimbare se salvează pe loc.

## 12. Ce nu atinge Valet niciodată

Valet lucrează doar cu fișierele aflate sub folderele urmărite. În plus, nu atinge următoarele. Singura excepție e opțiunea „Include fișierele ascunse” a unei reguli:

- **Foldere excluse.** Orice folder cu unul dintre aceste nume, la orice adâncime sub un folder urmărit, e sărit împreună cu tot ce e în el: `Windows`, `Program Files`, `Program Files (x86)`, `ProgramData`, `AppData`, `$Recycle.Bin`, `System Volume Information`, `.git`, `node_modules`, `target`, `.venv`, `venv`, `__pycache__`. Literele mari și mici nu contează. Un folder oarecare numit `target` sau `venv` e sărit și el.
- **Foldere cu `.valetignore`.** Un folder care conține un fișier cu numele `.valetignore` (conținutul nu contează) e sărit cu tot ce e în el. Valet nu mută fișiere nici în el.
- **Fișiere și foldere ascunse.** Nume care încep cu punct, sau cu atributul ascuns ori de sistem din Windows. O regulă poate cere „Include fișierele ascunse”, dar folderele ascunse rămân sărite oricum.
- **Legături.** Symlink-urile și joncțiunile nu sunt urmate și nu sunt mutate.
- **Fișiere OneDrive și Dropbox „la cerere”, nedescărcate.** Valet le recunoaște după atributele fișierului, fără să le deschidă. Nu le citește și nu le mută, ca să nu le descarce. Detectarea s-a testat cu atributul simulat, nu cu un OneDrive sau Dropbox real.
- **Fișiere în curs de scriere.** Extensiile `.crdownload`, `.part`, `.tmp`, `.download`, `.partial`, nume care încep cu `~$` și fișiere modificate în ultimele 10 secunde.
- **Fișiere blocate de alt program.** Eșuează cu mesaj, fără să fie atinse.
- **Discuri întregi.** `C:\`, `D:\`, `\\server\share` nu se acceptă ca foldere urmărite.
- **Destinații excluse.** Valet nu mută fișiere într-un folder exclus sau sub un `.valetignore`.

## 13. Discuri fără Coș de reciclare

Un disc USB sau o resursă de rețea nu are, de obicei, Coș de reciclare. Valet trimite fișiere la Coș doar pe discuri fixe, unde Windows are Coș. Pe un volum fără Coș, un „Șterge” al Windows ar fi definitiv, de aceea Valet refuză operația. Verificarea cere un disc fix, politica `NoRecycleFiles` nescrisă și setarea `NukeOnDelete` neactivată.

- **Trimitere la Coș** dintr-un volum fără Coș: eșuează cu „Volumul nu are Coș de reciclare; Valet nu mută și nu copiază aici.” Fișierul rămâne.
- **Copiere** într-un folder de pe un volum fără Coș: eșuează la fel, înainte să se copieze ceva.
- **Mutare între două discuri**, dacă unul nu are Coș: eșuează la fel.
- **Mutare în același volum** (de exemplu în interiorul unui USB): merge, pentru că nu cere Coș.
- **Anularea unei mutări între discuri** cere Coș pe ambele. Altfel eșuează, iar fișierul rămâne unde e.

Cazul „disc fără Coș” s-a testat cu un Coș simulat. Nu s-a testat cu un USB sau o resursă de rețea reale.

## 14. Confidențialitate

Pagina completă, cu ce conține o cerere și ce se păstrează unde: [PRIVACY.ro.md](PRIVACY.ro.md). Pe scurt:

- Valet nu trimite fișierele tale pe internet. Codul lui Valet care se conectează la internet are două părți: actualizarea și verificarea licenței.
- Verificarea cere un fișier static (JSON) de pe una din două adrese GitHub, cea a canalului tău (Stabil sau Beta). Cererea nu conține nume de fișiere, căi, reguli sau conținut. GitHub vede ce vede orice server web: adresa ta IP, ora și programul care a cerut (`tauri-plugin-updater/2.14.1`). Versiunea instalată nu se trimite.
- Se verifică la 30 de secunde după pornire, apoi o dată la 24 de ore cât rulează Valet, și când apeși „Verifică acum”. Verificarea automată nu instalează nimic. O poți opri din Setări, Actualizări, cu caseta „Verifică automat actualizările”; „Verifică acum” merge și atunci.
- Instalatorul unei versiuni noi se descarcă doar după ce apeși „Instalează și repornește” și confirmi. Semnătura lui e verificată cu o cheie publică din aplicație (o face pluginul de actualizare; Valet nu are un test pentru asta).
- O versiune făcută fără cheile de actualizare, cum e orice pornire din codul sursă, nu face nicio verificare.
- Verificarea licenței are loc doar dacă ai importat o licență Pro, într-o versiune care are un server de licențe configurat. Cererea conține trei lucruri: identificatorul licenței (un cod aleatoriu din fișierul licenței), un identificator al calculatorului (un hash într-un singur sens) și versiunea aplicației. La scoaterea licenței se trimit doar primele două. Nu se trimit adresa de email din licență, numele calculatorului, căi, nume de fișiere sau reguli. Planul Gratuit și proba de 14 zile nu fac nicio cerere de licență. Versiunile de acum nu au server de licențe configurat, deci nu fac nicio cerere de acest fel.
- Nu există telemetrie și nu există rapoarte de erori (Sentry nu e integrat). Dacă rapoartele de erori se adaugă, vor fi opt-in, cu un ecran „Ce trimitem”.
- Nume de fișiere, căi și conținut nu pleacă de pe calculator.
- Jurnalul (Istoricul) păstrează căile întregi ale fișierelor, pe discul tău.
- Nu s-a făcut o captură a traficului de rețea la rularea aplicației. Nu se poate spune sigur ce face WebView2 (o componentă Microsoft) sau ce fac pluginurile Tauri în afară de ce se vede în cod.
- Jurnalele de rulare: nivelul INFO (implicit) nu scrie căi de fișiere, după cum s-au citit apelurile de jurnal din cod; niciun test nu verifică textul lor. Nivelul DEBUG (caseta „Jurnal detaliat” din Setări) poate scrie căi. Nu s-a rulat o versiune instalată ca să se verifice ce scrie.

## 15. Unde sunt datele

În `%APPDATA%\ro.valetapp.desktop`:

- `valet.sqlite3`: regulile și jurnalul de operații;
- `config.json`: folderele urmărite;
- `app_settings.json`: limba, notificările, pornirea cu Windows, aplicarea automată, dacă ai terminat ghidul de prima pornire, dacă ai aprobat o previzualizare, canalul de actualizare, caseta „Verifică automat actualizările” și caseta „Jurnal detaliat”;
- `approvals.json`: regulile aprobate, versiunea la care a fost aprobată fiecare și folderele urmărite ale previzualizării care a aprobat-o;
- `license.valet-license` și `activation.valet-token`: doar după ce imporți o licență Pro. Primul e fișierul licenței (conține identificatorul licenței, adresa de email pe care a fost vândută și data până la care sunt incluse actualizările), al doilea e ultimul răspuns semnat al serverului de licențe;
- `license-removed\`: apare când scoți sau înlocuiești o licență. Fișierele vechi se mută aici, cu numele `<număr>-license.valet-license` și `<număr>-activation.valet-token`. Valet nu le șterge.

Data la care a început proba de 14 zile e păstrată în `valet.sqlite3`.

Valet nu șterge niciodată rânduri din jurnal, nici când planul Gratuit arată doar 7 zile. Jurnalul crește cu fiecare lot. Versiunile mai vechi păstrau și un tabel de statistici; a fost eliminat, iar Istoricul se vede în continuare din jurnal.

**Jurnalele de rulare** sunt în `%LOCALAPPDATA%\ro.valetapp.desktop\logs`. Fișierul activ e `valet.log`. La 2 MB se închide și se redenumește `valet_<data și ora>.log`; se păstrează cel mult 5 fișiere (cel activ și 4 mai vechi), deci cel mult aproximativ 10 MB. Ca să le deschizi, scrie calea în bara de adrese din Explorer. Calea vine din Tauri și nu s-a verificat pe o instalare Windows.

- Din oficiu scriu nivelul INFO: ce a pornit, câte operații, id-uri de lot, avertismente și erori.
- „Jurnal detaliat (pentru depanare)” în Setări, Asistență, trece pe DEBUG. Acolo pot apărea căi de fișiere. Jurnalul nu se trimite nicăieri; dacă îl trimiți cuiva, citește-l mai întâi.
- Dacă programul se oprește dintr-o eroare internă (panică), se scrie în jurnal o linie „Panic at …” (căile din mesaj sunt înlocuite cu `<path>`). Operațiile rămase la jumătate se hotărăsc la următoarea pornire, ca după orice oprire (vezi [secțiunea 6](#6-istoric)).

## 16. Limite cunoscute

- Versiune de test. Nicio versiune nu a fost publicată, iar instalatorul nu e semnat (Windows SmartScreen avertizează, vezi [secțiunea 17](#17-instalare-actualizări-și-dezinstalare)).
- Licențele nu sunt active în versiunile de acum: nu e configurat un server de licențe, deci cumpărarea și activarea nu funcționează. Limitele planului Gratuit și proba de 14 zile funcționează (testate cu teste automate; nu au fost încercate pe un calculator Windows real). Vezi [secțiunea 18](#18-valet-pro-și-planul-gratuit).
- Versiunea din Microsoft Store nu e publicată, iar pachetul nu a fost încercat într-o instalare reală din Store.
- Actualizarea nu a fost încă încercată pe un calculator Windows real: nu există o versiune publicată de la care să se actualizeze.
- Acțiunile „redenumește” și „etichetează” din plan nu există.
- Anularea merge pe lot întreg, nu pe fișier. Regulile nu se pot reordona din interfață.
- Un folder urmărit adăugat mai târziu se organizează automat doar după ce aprobi o previzualizare (vezi [secțiunea 9](#9-organizarea-automată)).
- Windows poate pierde evenimente de schimbare când sunt foarte multe. Verificarea completă la 10 minute le prinde. Valet nu poate cere singur o rescanare în acel caz, așa că un fișier poate aștepta până la 10 minute.
- Testele care folosesc Windows real (Coș, atribute, restaurare din Coș) rulează în CI pe `windows-latest`. Starea lor pe fiecare commit, inclusiv ce a trecut și ce nu s-a rulat, e în [SAFETY.md](SAFETY.md). Restaurarea din Coșul de reciclare Windows a fost testată doar pe mașina CI. Pană de curent, OneDrive și Dropbox reale, USB, resursele de rețea și un disc extern real scos și reconectat nu s-au testat.
- Alte limite și cazuri, cu testele care le acoperă, sunt în [SAFETY.md](SAFETY.md).

## 17. Instalare, actualizări și dezinstalare

**Instalare.** Nicio versiune nu a fost publicată încă. Când va exista una, instalatorul va fi pe pagina [GitHub Releases](https://github.com/OsztrovszkyVlad/Valet/releases), cu numele `Valet_<versiune>_x64-setup.exe`. Instalatorul e făcut să:

- se instaleze doar pentru utilizatorul curent, fără drepturi de administrator, în `%LOCALAPPDATA%\Valet` (folderul implicit al șablonului Tauri; nu s-a verificat pe Windows);
- ofere limba engleză sau română, un folder „Valet” în meniul Start și, opțional, o scurtătură pe desktop. Scurtătura se oferă pe ultima pagină a unei instalări cu ferestre. O instalare silențioasă (`/S`, winget, scripturi) nu creează scurtătură pe desktop; o scurtătură care exista deja rămâne. O actualizare păstrează ce ai deja (`packaging/nsis/hooks.nsh`);
- descarce Microsoft WebView2 dacă lipsește (singurul caz în care instalatorul are nevoie de internet).

Instalatorul nu a fost rulat pe Windows în sesiunile care au scris acest ghid.

**Avertismentul Windows SmartScreen.** Instalatorul și aplicația nu sunt semnate digital încă. Din cauza asta, Windows SmartScreen arată la rulare „Windows protected your PC” (în Windows cu limba română textul e tradus; nu s-a verificat cum). Ca să continui, apasă „More info”, apoi „Run anyway”. Avertismentul apare pentru programe nesemnate și puțin descărcate; nu e rezultatul unei analize a lui Valet. Un certificat de semnare gratuit pentru proiecte open-source (SignPath Foundation sau Certum Open Source) nu a fost obținut încă. Dacă vrei să verifici fișierul descărcat, rularea „Release” din GitHub Actions listează suma SHA-256 a instalatorului; o compari cu rezultatul comenzii `Get-FileHash Valet_<versiune>_x64-setup.exe -Algorithm SHA256`.

**Actualizări.** În Setări, Actualizări, alegi canalul (Stabil sau Beta) și apeși „Verifică acum”, sau aștepți verificarea automată (vezi [secțiunea 14](#14-confidențialitate)). Dacă există o versiune nouă, apeși „Instalează și repornește” și confirmi. Dacă un lot de operații rulează, instalarea așteaptă să se termine. Valet se repornește singur; regulile, istoricul și setările rămân. Actualizările există doar în versiunile făcute cu cheile de semnare ale actualizatorului; o versiune fără ele răspunde „Actualizările nu sunt configurate în această versiune.”, iar atunci instalezi manual instalatorul nou. Dacă ai debifat „Verifică automat actualizările”, nu se face nicio verificare automată; „Verifică acum” merge oricum. Nu instala o versiune mai veche peste una mai nouă: versiunea veche s-ar putea să nu poată deschide baza de date pe care a migrat-o cea nouă (nu s-a testat).

**Dezinstalare.** Din Windows, Setări, Aplicații, Valet, Dezinstalează (sau cu dezinstalatorul din folderul de instalare).

- Datele tale rămân din oficiu: setările, regulile, aprobările și jurnalul, deci Istoricul și anularea merg și după o reinstalare.
- Dezinstalatorul are o casetă „Șterge datele aplicației”, nebifată la început. Bifată, șterge `%APPDATA%\ro.valetapp.desktop` (inclusiv jurnalul de operații, deci Istoricul și anularea dispar) și `%LOCALAPPDATA%\ro.valetapp.desktop` (jurnalele de rulare). Asta s-a citit din șablonul Tauri; nu s-a rulat o dezinstalare pe Windows.
- Intrarea „Pornește Valet odată cu Windows” se șterge la dezinstalare.
- Fișierele licenței sunt în același folder de date, deci rămân din oficiu și dispar dacă bifezi caseta. Dezinstalatorul nu contactează serverul de licențe și nu eliberează activarea: dacă vrei să folosești licența pe alt calculator, scoate-o din Setări înainte (vezi [secțiunea 18](#18-valet-pro-și-planul-gratuit)).
- Fișierele pe care Valet le-a mutat sau copiat rămân unde sunt, iar cele din Coșul de reciclare rămân în Coș. Dezinstalarea nu anulează nimic. Dacă vrei să anulezi un lot, fă-o din Istoric înainte de dezinstalare.

**Versiunea din Microsoft Store.** Un pachet în Microsoft Store e în pregătire și nu e disponibil încă. Diferențele, după `packaging/msix/README.md`, secțiunea 5 (nu s-au verificat într-o instalare reală din Store):

- Actualizările vin din Store. Valet nu face nicio verificare de versiune și nu are canal de actualizare.
- Pornirea cu Windows se setează din setările Windows, nu dintr-o bifă a lui Valet (vezi [secțiunea 10](#10-zona-de-notificare-și-pornirea-cu-windows)).
- Datele sunt într-un folder al pachetului, sub `%LOCALAPPDATA%\Packages\<numele pachetului>\LocalCache`, nu în `%APPDATA%\ro.valetapp.desktop`.
- **Dezinstalarea aplicației din Store șterge acel folder: regulile, setările, fișierele licenței și jurnalul, deci Istoricul și anularea dispar.** Nu există o casetă „Șterge datele aplicației” ca la instalatorul obișnuit. Dacă vrei să anulezi un lot, fă-o din Istoric înainte. Dacă vrei să folosești licența pe alt calculator, scoate-o din Setări înainte (vezi [secțiunea 18](#18-valet-pro-și-planul-gratuit)). Fișierele pe care Valet le-a mutat rămân unde sunt.
- Instalatorul nu există în această variantă, deci nu există nici scurtătura de desktop, nici avertismentul SmartScreen: pachetul e semnat de Store.
- Licența Pro se importă la fel ca în varianta obișnuită.

## 18. Valet Pro și planul Gratuit

Valet are trei stări: **probă Pro** (primele 14 zile), **Gratuit** și **Pro**. Starea de acum se vede în Setări, secțiunea „Valet Pro”, și într-un indicator mic sus („Gratuit” sau „Probă Pro: N zile”; pe Pro nu apare nimic).

**Ce funcționează acum.** Licențele nu sunt încă active în versiunile de acum: nu e configurat un server de licențe (fișierele `crates/valet-core/license/api.txt`, `pubkey.txt` și `checkout.txt` din depozit au 0 octeți). De aceea cumpărarea și activarea unei licențe nu funcționează. Secțiunea „Valet Pro” scrie „Licențele nu sunt încă active în această versiune. Planurile Gratuit și Probă funcționează normal.”, butonul „Cumpără Pro” e inactiv, cu textul „Cumpărarea nu e deschisă încă.”, iar butoanele de import al licenței sunt inactive. Proba de 14 zile și planul Gratuit funcționează normal. Restul secțiunii descrie cum e făcută partea de licență, ca să știi la ce să te aștepte când va fi activă; partea de licență s-a testat cu teste automate și un server de probă pe calculatorul de dezvoltare, nu cu un server real.

### Ce include fiecare plan

| | Gratuit | Pro și proba de 14 zile |
|---|---|---|
| Cele 8 presetări | da, nu se numără la limită | da |
| Reguli proprii active în același timp | 3 | nelimitat |
| Foldere urmărite active în același timp | 2 | nelimitat |
| Istoric afișat | loturile din ultimele 7 zile | tot istoricul |
| Anulare | doar cel mai nou lot de organizare | orice lot din istoric |
| Export și import de reguli | nu | da |
| Tipuri de condiții și acțiuni | toate | toate |

- „Regulă proprie” e o regulă care nu vine dintr-o presetare. Regulile presetărilor nu se numără.
- Limitele se verifică în program, nu doar în interfață. Interfața le arată.
- Nu se șterge nimic din cauza unei limite: nici reguli, nici foldere, nici rânduri din jurnal.

### Proba de 14 zile

- Începe la prima pornire a unei versiuni cu această funcție. Cine vine dintr-o versiune de test mai veche începe proba la prima pornire după actualizare.
- Are toate funcțiile Pro. Nu cere card și nu folosește internetul: data de început e păstrată doar pe calculatorul tău.
- Zilele rămase se văd în indicatorul de sus și în Setări, Valet Pro.

### Când treci pe Gratuit

Valet trece pe Gratuit când proba s-a terminat și nu ai o licență Pro valabilă pe acest calculator, sau când licența nu mai e valabilă (vezi mai jos). Se verifică la pornire și din oră în oră.

- Dacă ai cel mult 3 reguli proprii active și cel mult 2 foldere, nu se schimbă nimic.
- Dacă ai mai multe, apare dialogul „Alege ce rămâne activ în planul Gratuit”. Textul lui: „Nimic nu se șterge: regulile și folderele pe care nu le alegi rămân salvate, dezactivate, și revin cu Pro.” Bifezi cel mult 3 reguli proprii și 2 foldere, apoi apeși „Salvează alegerea”. Presetările nu se numără și rămân cum sunt.
- „Decid mai târziu” (sau tasta Esc) închide dialogul fără să alegi. Până alegi, rămân active primele 3 reguli proprii, în ordinea din lista Reguli, și primele 2 foldere din lista Setări. Dialogul reapare la următoarea pornire.
- Regulile nealese sunt oprite și au eticheta „dezactivată de planul Gratuit”. Rămân salvate.
- Folderele nealese rămân în lista din Setări, cu textul „Inactiv în planul Gratuit. Nu e urmărit și nu se organizează.” Valet nu le urmărește și nu le planifică, ca pe un folder indisponibil. Aprobările lor se păstrează.
- Pe Gratuit, bifarea unei reguli proprii peste limită sau adăugarea unui folder peste limită e refuzată cu un mesaj (vezi [secțiunea 8](#8-reguli-proprii) și [secțiunea 11](#11-setări)). Poți oricând opri, edita sau șterge reguli și elimina foldere.
- Când treci din nou pe Pro sau pe probă, regulile oprite de plan se pornesc la loc și toate folderele devin active. Regulile pe care le-ai oprit tu rămân oprite.
- Istoricul arată doar loturile din ultimele 7 zile; restul rămâne în jurnal și reapare pe Pro (vezi [secțiunea 6](#6-istoric)).

### Anularea pe planul Gratuit

- Se poate anula doar cel mai nou lot de tip „Organizare” (inclusiv unul „organizat automat”), chiar dacă a fost deja anulat.
- Loturile de tip „Anulare” și „Recuperare după oprire” nu se numără ca „cel mai nou lot”.
- După ce anulezi acel lot, el rămâne cel mai nou lot de organizare. Loturile mai vechi rămân blocate: nu poți anula lot după lot. Mesajul: „Planul Gratuit anulează doar ultimul lot. Anularea loturilor mai vechi e inclusă în Pro.”
- Cel mai nou lot se caută în tot jurnalul, nu doar în cele 7 zile afișate.
- Dacă ai nevoie de un fișier dintr-un lot mai vechi, Istoricul (în limita celor 7 zile) arată unde a ajuns, iar fișierele de la Coșul de reciclare se pot restaura și cu mâna (vezi [secțiunea 5](#5-anularea-unui-lot)).

### Importul unei licențe

1. În Setări, secțiunea „Valet Pro”, apasă „Am o licență” și alege fișierul cu extensia `.valet-license` pe care l-ai primit după cumpărare. Sau apasă „Am licența ca text”, lipește tot conținutul licenței, de la prima acoladă la ultima, și apasă „Activează licența”.
2. Valet verifică semnătura licenței. Apoi se conectează la serverul de licențe și activează licența pentru acest calculator. La acest pas ai nevoie de internet.
3. Dacă merge, vezi „Licența e activă pe acest calculator.” și planul „Pro, licență pe adresa ta de email”, cu „Actualizări incluse până la” o dată. Adresa de email rămâne pe calculator și nu se trimite serverului.

O licență poate fi activă pe cel mult 3 calculatoare. Valet păstrează fișierul licenței și răspunsul serverului în folderul de date (vezi [secțiunea 15](#15-unde-sunt-datele)). La o eroare, nimic nu se schimbă pe disc. Mesajele posibile:

| Mesaj | Ce înseamnă |
|---|---|
| Licența nu e validă: semnătura nu se potrivește. Verifică dacă ai copiat tot textul. | Fișierul sau textul a fost schimbat sau tăiat. Folosește fișierul original. |
| Licența nu e pentru Valet Pro. | Fișierul e semnat, dar pentru alt produs. |
| Licența nu acoperă această versiune: actualizările incluse s-au terminat înainte de publicarea ei. | Vezi „Actualizările incluse” mai jos. |
| Serverul de licențe nu a putut fi contactat. Verifică conexiunea la internet și încearcă din nou. | Fără internet sau serverul nu răspunde. |
| Serverul de licențe nu cunoaște această licență. | Identificatorul licenței nu există pe server. |
| Licența e deja activă pe 3 calculatoare. Scoate-o de pe unul dintre ele din Setări, apoi încearcă din nou. | Toate cele 3 locuri sunt ocupate. |
| Licența a fost revocată. | Serverul a retras licența. |
| Identificatorul acestui calculator nu a putut fi citit, așa că licența nu poate fi activată. | Valet n-a putut citi identificatorul Windows al calculatorului. |
| Fișierul licenței nu a putut fi citit sau salvat. Detaliile sunt în jurnal. | Eroare de disc la citire sau la salvare. |
| Licențele nu sunt încă active în această versiune. | Versiunea nu are server de licențe configurat (cazul de acum). |

### Actualizările incluse

Pro include actualizările timp de 12 luni de la cumpărare. Data apare ca „Actualizări incluse până la …”. Licența acoperă versiunile publicate până la acea dată. O versiune publicată după ea rulează ca Gratuit și Setările spun data. Versiunile publicate până la acea dată rămân Pro, iar o versiune pe care o ai deja nu se oprește. Înainte să instalezi o actualizare, Valet compară data ei de publicare cu data licenței; dacă nu e acoperită, confirmarea spune că versiunea nouă va rula ca Gratuit. O poți instala oricum. Prețul reînnoirii actualizărilor (15 € pe an, opțional) e în planul produsului; nu există încă un mod de a o cumpăra din aplicație.

### Verificarea licenței și lucrul fără internet

- După activare, Valet cere serverului un răspuns nou doar când ultimul răspuns reușit are 30 de zile sau mai mult. Se uită la pornire și din oră în oră, iar o cerere pleacă cel mult o dată la 24 de ore. Planul Gratuit și proba nu fac nicio cerere de licență.
- „Verifică licența acum” face verificarea pe loc, chiar înainte de 30 de zile. Sub butoane se vede „Ultima verificare a licenței: …”.
- Fără internet, Pro merge mai departe, cât timp ultima verificare reușită are cel mult 60 de zile (cele 30 de zile până la verificarea următoare, plus 30 de zile de toleranță). O eroare de rețea nu schimbă răspunsul păstrat.
- Dacă au trecut mai mult de 60 de zile, Valet merge ca Gratuit și Setările scriu „Licența nu a mai fost verificată online de peste 60 de zile. Conectează-te la internet și apasă „Verifică licența acum”.” Nu se pierde nimic; următoarea verificare reușită readuce Pro.
- Dacă serverul răspunde că licența e necunoscută sau revocată, sau că e activă pe prea multe calculatoare, Valet scoate răspunsul păstrat din folderul de date (îl mută în `license-removed`, nu îl șterge) și merge ca probă sau Gratuit. Fișierul licenței rămâne, ca Setările să poată spune de ce.

### Scoaterea licenței de pe un calculator

1. În Setări, Valet Pro, apasă „Scoate licența de pe acest calculator” și confirmă cu „Scoate licența”. Textul de confirmare: „Licența se scoate de pe acest calculator și eliberează una dintre cele 3 activări, ca s-o poți folosi pe alt calculator. Fișierul licenței nu se șterge; îl poți importa din nou oricând.”
2. Valet trimite serverului o cerere de dezactivare, cu identificatorul licenței și al calculatorului. Dacă nu merge (fără internet), licența se scoate oricum de pe acest calculator, dar locul de pe server rămâne ocupat până îl eliberezi.
3. Fișierul licenței și răspunsul serverului se mută în `license-removed` din folderul de date, cu un număr în față. Nu se șterg și nu se înlocuiesc. Importul unei alte licențe sau al aceleiași mută la fel fișierele vechi.

Dezinstalarea lui Valet nu eliberează locul. Scoate licența din Setări înainte de dezinstalare.

**Dacă vechiul calculator nu mai există** (stricat, formatat) și nu poți scoate licența de pe el, cere asistenței resetarea activărilor, cu numărul comenzii din emailul de la comerciant. Asistența poate goli de pe server lista calculatoarelor active ale licenței (o unealtă a serviciului de licențe, `services/license/reset.mjs`); apoi imporți licența pe calculatorul nou. Istoricul activărilor se păstrează pe server. Asistența se cere pe [GitHub Issues](https://github.com/OsztrovszkyVlad/Valet/issues) (vezi „Rambursare și asistență” mai jos).

### Rambursare și asistență

- **Rambursare:** în 14 zile de la cumpărare, prin comerciantul care a procesat plata (numărul comenzii e în emailul de confirmare de la el). La rambursare licența este revocată. Valet află la următoarea verificare a licenței (cel mult o dată la 30 de zile; „Verifică licența acum” o face pe loc) și trece pe proba de 14 zile, dacă încă n-a trecut, sau pe planul Gratuit. Nu se pierde nimic: regulile, folderele, istoricul și setările rămân, iar ce depășește limitele planului Gratuit se dezactivează, nu se șterge. Fișierul licenței rămâne pe calculator, ca Setări să poată spune de ce nu mai e activă (mesajul „Licența a fost revocată.”).
- **Cumpărarea nu e deschisă încă.** Se deschide abia după ce plata și rambursarea au fost încercate cap-coadă în modul de probă al comerciantului. Până atunci nu există nimic de rambursat.
- **Asistență:** [GitHub Issues](https://github.com/OsztrovszkyVlad/Valet/issues). Deocamdată nu există altă cale de contact și nici o adresă de email. Paginile sunt publice: nu pune în ele adresa ta de email, fișierul licenței sau fișiere de-ale tale, iar jurnalul citește-l înainte să-l lipești. Pentru o problemă de licență, scrie numărul comenzii din emailul primit de la comerciant. Nu posta codul licenței din Setări, Valet Pro și nici fișierul licenței: oricine are codul poate ocupa una dintre cele 3 activări.

### Versiunea din Microsoft Store

Diferențele față de instalatorul obișnuit sunt în [secțiunea 17](#17-instalare-actualizări-și-dezinstalare): actualizările vin din Store, pornirea cu Windows se setează din setările Windows (butonul „Deschide setările Windows”), iar dezinstalarea aplicației din Store șterge datele, inclusiv Istoricul și licența de pe calculator. Planurile și limitele sunt aceleași.
