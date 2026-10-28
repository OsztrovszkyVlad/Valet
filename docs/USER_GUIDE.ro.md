# Ghid de utilizare Valet

English version: [USER_GUIDE.en.md](USER_GUIDE.en.md)

Valet pune ordine în fișierele de pe calculatorul tău, după reguli pe care le vezi și le poți anula. Ghidul descrie ce face aplicația în versiunea din acest depozit.

Valet e încă în faza de test. Nu are instalator public și nu are lansare. Se pornește din codul sursă (vezi [README.md](../README.md)). Până nu ai încredere în el, lucrează pe foldere în care ai copii ale fișierelor.

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

## 1. Ce face Valet și ce nu face

**Ce face**

- Urmărește folderele pe care le alegi, de exemplu Descărcări și Desktop.
- Aplică regulile active pe fișierele din aceste foldere. O regulă poate muta un fișier, îl poate copia, îl poate trimite la Coșul de reciclare sau doar îl poate raporta.
- Arată lista cu ce ar face (previzualizarea) înainte să atingă un fișier.
- Scrie fiecare operație într-un jurnal înainte să o facă. Jurnalul se vede în ecranul Istoric.
- Poate anula un lot întreg.
- Dacă aplicația sau calculatorul se oprește în mijlocul unui lot, la pornirea următoare verifică operațiile rămase neterminate.
- Poate organiza singur fișierele noi, dar numai după ce ai aprobat o previzualizare (vezi [secțiunea 9](#9-organizarea-automată)).

**Ce nu face**

- Nu șterge nimic definitiv. Orice eliminare merge în Coșul de reciclare Windows.
- Nu suprascrie fișiere. Dacă numele de la destinație e ocupat, fișierul primește `nume (2).ext`, apoi `nume (3).ext` și așa mai departe.
- Nu redenumește fișiere și nu le etichetează. Aceste acțiuni nu există în versiunea de acum.
- Nu se uită la conținutul fișierelor ca să le clasifice. Regulile se uită la nume, extensie, tip, mărime, date și cale. Singura citire de conținut e verificarea cu amprentă (hash) a fișierelor pe care le mută, le copiază sau le trimite la Coș, și compararea fișierelor de aceeași mărime pentru regulile de duplicate.
- Nu iese pe internet (vezi [secțiunea 14](#14-confidențialitate)).
- Nu atinge folderele de sistem, fișierele ascunse și fișierele OneDrive sau Dropbox nedescărcate (vezi [secțiunea 12](#12-ce-nu-atinge-valet-niciodată)).

## 2. Prima pornire

La prima pornire apare un ghid în 3 pași. Nimic nu se mută în timpul lui. Ecranele sunt în română. Limba se schimbă în Setări, după ce termini ghidul. Presetările instalate la prima pornire folosesc numele de foldere din limba activă atunci, deci în română (Imagini, Documente și așa mai departe).

**Pasul 1 din 3: folderele**

![Pasul 1: alegerea folderelor](images/onboarding-1-folders.png)

- Descărcări și Desktop sunt bifate. Documente și Imagini nu sunt.
- Poți adăuga alt folder scriind calea lui în câmpul „Adaugă alt folder”, de exemplu `C:\Users\nume\Proiecte`. Nu există fereastră de alegere a folderului.
- Trebuie să bifezi cel puțin un folder.
- Valet refuză un folder dacă e un disc întreg (`C:\`, `D:\`, `\\server\share`), dacă în calea lui apare un nume exclus implicit (de exemplu `AppData` sau `Program Files`) sau dacă nu există. Valet verifică folderele când apeși „Continuă” la pasul 2. Dacă unul e refuzat, primești un mesaj și revii la pasul 1.
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

Orice lot de organizare terminat se poate anula și din Istoric, nu doar ultimul.

### Ce se întâmplă în fiecare caz

| Situație | Ce face Valet |
|---|---|
| Fișierul de la destinație e neschimbat, iar locul inițial e liber | Îl mută înapoi. |
| Fișierul a fost schimbat după lot (conținutul diferă) | Nu mută nimic. Fișierul apare la „Fișiere care cer confirmare” cu „Fișierul s-a schimbat după lot”. Cu „Anulează oricum” e mutat înapoi așa cum e acum. |
| Locul inițial e ocupat (un fișier cu același nume există din nou acolo) | Nu mută nimic. Apare „Locul inițial e ocupat; fișierul va primi un nume nou”. Cu „Anulează oricum”, fișierul e pus lângă cel existent, ca `nume (2).ext`. Rezultatul spune „X restaurat ca Y”. Fișierul existent nu e atins. |
| Fișierul a fost trimis la Coș (regulă „La Coș”) | Valet îl restaurează din Coșul de reciclare la locul lui. În listă apare „restaurare din Coș”. Se restaurează și folderele goale trimise la Coș. |
| Fișierul trimis la Coș nu mai e în Coș (l-ai golit) | Anularea eșuează pentru acel fișier cu „Fișierul nu mai există”. |
| Fișier trimis la Coș, iar locul inițial e ocupat | La „Anulează oricum”, Valet nu restaurează lângă fișierul existent. Elementul rămâne în Coș și apare „Locul inițial e ocupat; elementul a rămas în Coșul de reciclare”. Îl poți restaura tu din Coș. Atenție: lista de confirmare scrie și pentru el „fișierul va primi un nume nou”, dar nu e adevărat pentru fișierele din Coș. |
| Fișier restaurat din Coș cu conținut diferit de cel de la trimitere | Rămâne restaurat și e listat: „restaurat, dar conținutul diferă de cel de la momentul trimiterii la Coș”. |
| Fișierul era o copie (regulă „Copiază în”) | Copia e trimisă la Coș. Dacă originalul nu mai există, copia e ultima, deci Valet cere confirmare: „Originalul nu mai există; anularea trimite această copie în Coșul de reciclare”. |
| Fișierul de la destinație a dispărut, e blocat sau e la cerere | Eșuează pentru acel fișier, cu motivul. |
| Operații care au eșuat în lotul inițial | Nu sunt atinse. Apar la „Sărite”. |
| Foldere create de lot | Rămân, goale. Anularea nu le trimite la Coș. |
| Anulare între două discuri | Cere Coș de reciclare pe ambele. Altfel eșuează cu „Volumul nu are Coș” și fișierul rămâne unde e. |

Restaurarea din Coșul de reciclare Windows nu are încă un rezultat de test reușit pe Windows (vezi [SAFETY.md](SAFETY.md)). Dacă anularea eșuează pentru un fișier trimis la Coș, deschide Coșul de reciclare și restaurează-l cu mâna.

Anularea e un lot nou, de tip „Anulare”. Un lot de tip „Anulare” sau „Recuperare după oprire” nu se poate anula. A doua anulare a aceluiași lot nu mai găsește nimic de făcut.

### Restaurare din Coșul de reciclare, cu mâna

Fișierele trimise la Coș de Valet sunt în Coșul de reciclare Windows obișnuit. Le poți restaura de acolo ca pe orice fișier, cu clic dreapta și opțiunea de restaurare. Pentru fișierele trimise la Coș de pasul de recuperare (vezi [secțiunea 6](#6-istoric)), Valet nu are anulare. Acolo restaurarea din Coș se face cu mâna.

## 6. Istoric

Ecranul Istoric listează loturile, cele mai noi primele. Se văd cel mult 50. „Reîncarcă” reîncarcă lista.

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

O presetare e un set de reguli gata făcute. Se aleg la prima pornire. După aceea apar în ecranul Reguli, cu eticheta „presetare”, și le poți opri, edita sau șterge ca pe orice regulă.

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
- Dacă limba aplicației era engleza când s-au instalat presetările, numele folderelor sunt în engleză (Images, Documents, Archives, Installers, Videos, Music, Pictures\Screenshots, Documents\Invoices, Documents\From Desktop). Schimbarea limbii mai târziu nu redenumește regulile deja instalate.
- Ecranul Reguli nu are buton pentru a reinstala o presetare ștearsă. O poți reface cu o regulă proprie.

## 8. Reguli proprii

Ecranul Reguli listează regulile. Pentru fiecare vezi:

- bifa „Activă”;
- numele, cu eticheta „presetare” la cele din presetări;
- un rezumat (condiții → acțiune);
- folderele în care se aplică („În toate folderele urmărite” sau „În: …”);
- butoanele „Editează” și „Șterge”. La ștergere Valet întreabă, iar fișierele nu sunt atinse.

![Ecranul Reguli](images/rules.png)

„Adaugă regulă” deschide editorul. Salvarea unei reguli nu atinge niciun fișier.

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

- O regulă nouă se adaugă la sfârșitul listei, deci după presetări.
- O regulă editată rămâne pe locul ei.
- Nu există buton pentru a muta o regulă mai sus.
- Dacă o presetare ia deja fișierele pe care le vrei, oprește-o din bifa „Activă” sau editează-o. Previzualizarea arată în coloana „Regula” ce regulă a decis.

### Exemple

Exemplele folosesc un folder adăugat de tine în Setări, ca să nu se bată cap în cap cu presetările.

1. **PDF-uri scanate, pe ani.** Folderul urmărit: `C:\Users\nume\Scanari`. Regula „Scanări PDF”: condiția „Extensie (una)” = `pdf`, acțiunea „Mută în” `{documents}/Scanari/{year}`, folder: doar `Scanari`.
2. **Arhive vechi la Coș.** Condițiile: „Tip de fișier” = Arhive și „Modificat acum mai mult de” 90 de zile. Acțiunea: „La Coș”. Folder: `Scanari`. Rulează întâi o previzualizare și citește lista.
3. **Videoclipuri mari, doar raport.** Condițiile: „Tip de fișier” = Video și „Mai mare de” 500 MB. Acțiunea: „Doar raport”. Presetările nu o încurcă, pentru că regulile de raport nu opresc alte reguli.

### După salvare

O regulă nouă sau modificată primește o versiune nouă. Orice schimbare o schimbă, inclusiv bifa „Activă”. Până la o previzualizare aprobată, organizarea automată nu rulează operațiile ei (vezi [secțiunea 9](#9-organizarea-automată)). Ștergerea unei reguli îi șterge și aprobarea.

## 9. Organizarea automată

Valet poate organiza singur fișierele noi din folderele urmărite. E oprită din oficiu.

### Cum o pornești

1. Rulează o previzualizare și apasă „Organizează fișierele”. Asta e „previzualizarea aprobată”.
2. În Setări, bifează „Aplicare automată în folderele urmărite”. Până la prima previzualizare aprobată, bifa e inactivă și scrie „Disponibil după ce aprobi prima previzualizare.”

Butonul „Organizează” apare doar când previzualizarea are cel puțin o operație.

### Ce reguli rulează singure

Doar regulile aprobate la versiunea lor de acum. O regulă e aprobată când apeși „Organizează” pe o previzualizare care o conținea: regula avea o operație, o linie de raport sau un fișier sărit în acea previzualizare. Regula nouă, regula modificată (inclusiv oprită și repornită) sau regula care n-a avut nimic de făcut în previzualizare așteaptă o previzualizare nouă. Operațiile ei se văd la următoarea previzualizare manuală.

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

### Atenție

Aprobarea ține de regulă, nu de folder. Dacă adaugi mai târziu un folder urmărit nou, regulile deja aprobate se aplică și în el, fără o previzualizare separată pentru acel folder. După ce adaugi un folder, rulează o previzualizare manuală înainte de a lăsa organizarea automată pornită, sau oprește-o.

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

## 11. Setări

![Setări](images/settings.png)

- **Foldere urmărite:** lista folderelor. Scrii calea și apeși „Adaugă”, sau apeși „Elimină”. Valet refuză un disc întreg, un folder cu un nume exclus în cale (de exemplu `AppData`) și un folder care nu există.
- **Limba:** Română sau English. Se schimbă imediat. Regulile și presetările deja instalate își păstrează numele și folderele.
- **Notificare scurtă după un lot:** pornită din oficiu.
- **Pornește Valet odată cu Windows:** vezi [secțiunea 10](#10-zona-de-notificare-și-pornirea-cu-windows).
- **Aplicare automată în folderele urmărite:** vezi [secțiunea 9](#9-organizarea-automată).

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

- Valet nu folosește internetul pentru fișierele tale. În codul aplicației nu există apeluri de rețea. Căutarea în cod, la versiunea din acest depozit, nu găsește nicio bibliotecă sau apel de rețea. Singurele adrese HTTP din proiect sunt `http://localhost:5173` (serverul local de dezvoltare, în `tauri.conf.json`) și `http://127.0.0.1` în testul automat al interfeței.
- Actualizarea automată, verificarea licenței și raportarea de erori nu sunt încă în aplicație.
- Nume de fișiere, căi și conținut nu pleacă de pe calculator.
- Jurnalul (Istoricul) păstrează căile întregi ale fișierelor, pe discul tău.
- Nu s-a făcut o captură a traficului de rețea la rularea aplicației. Nu se poate spune sigur că Tauri sau pluginurile lui nu fac nicio cerere la rulare.
- Jurnalele de rulare scriu în dezvoltare căi întregi, la nivelul DEBUG. Un build de release, la nivelul INFO, e făcut să nu scrie căi, dar nu s-a construit un build de release ca să se verifice.

## 15. Unde sunt datele

În `%APPDATA%\ro.valetapp.desktop`:

- `valet.sqlite3`: regulile și jurnalul de operații;
- `config.json`: folderele urmărite;
- `app_settings.json`: limba, notificările, pornirea cu Windows, aplicarea automată, dacă ai terminat ghidul de prima pornire și dacă ai aprobat o previzualizare;
- `approvals.json`: regulile aprobate și versiunea la care au fost aprobate.

Valet nu șterge niciodată rânduri din jurnal. Jurnalul crește cu fiecare lot.

## 16. Limite cunoscute

- Versiune de test. Valet nu are instalator și nu a fost lansat.
- Acțiunile „redenumește” și „etichetează” din plan nu există.
- Anularea merge pe lot întreg, nu pe fișier. Regulile nu se pot reordona din interfață. O presetare ștearsă nu se poate reinstala din interfață.
- Aprobarea pentru organizarea automată e pe regulă, nu pe folder (vezi [secțiunea 9](#9-organizarea-automată)).
- Windows poate pierde evenimente de schimbare când sunt foarte multe. Verificarea completă la 10 minute le prinde. Valet nu poate cere singur o rescanare în acel caz, așa că un fișier poate aștepta până la 10 minute.
- Testele care folosesc Windows real (Coș, atribute, restaurare din Coș) rulează în CI pe `windows-latest`. Pentru versiunea curentă nu au un rezultat reușit; detalii în [SAFETY.md](SAFETY.md). Restaurarea din Coșul de reciclare Windows e neconfirmată. Pana de curent, OneDrive și Dropbox reale, USB și resursele de rețea nu s-au testat.
- Alte limite și cazuri, cu testele care le acoperă, sunt în [SAFETY.md](SAFETY.md).
