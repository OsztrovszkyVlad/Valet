# Șabloane de răspuns pentru asistență (română)

English version: [templates.en.md](templates.en.md)

Răspunsuri scurte pentru cazurile cele mai frecvente. Înlocuiește `{marcajele}`. Modifică liber; sunt puncte de plecare, nu scenarii.

Reguli pentru orice răspuns:

- Nu cere niciodată conținutul unui fișier. Și numele și căile pot fi sensibile: cere-le doar când e nevoie și lasă omul să se uite la ce trimite înainte.
- Jurnalul poate conține căi de fișiere dacă jurnalul detaliat a fost pornit. Spune asta de fiecare dată când îl ceri.
- Nu promite o dată pentru o remediere.

Unde sunt lucrurile (ca în `docs/PRIVACY.ro.md`):

- Fișierul de jurnal: `%LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log` (cele mai vechi sunt `valet_<data și ora>.log` în același folder, cel mult 5 fișiere).
- Jurnal detaliat: Setări, Asistență, „Jurnal detaliat (pentru depanare)”. Se aplică pe loc, fără repornire. E oprit din oficiu.
- Folderul de date: `%APPDATA%\ro.valetapp.desktop` (baza de date cu jurnalul de operații, setările, fișierele de licență).
- Versiunea: Setări, Despre.

---

## 1. Raport de eroare primit

```
Bună, {nume},

Mulțumesc pentru raport. Ca să găsesc cauza, am nevoie de câteva lucruri:

1. Versiunea Valet (Setări, Despre).
2. Versiunea ta de Windows (Start, scrie „winver”, Enter) și dacă e Windows 10 sau 11.
3. Pașii care duc la problemă, cât mai exact: ce ecran, ce buton, ce te așteptai să se întâmple și ce s-a întâmplat.
4. Fișierul de jurnal: %LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log
   (lipește calea în bara de adresă din Explorer). Te rog să-l atașezi sau să trimiți ultimele 50 de linii.

Dacă jurnalul nu arată destul, poți porni mai multe detalii:
Setări, Asistență, „Jurnal detaliat (pentru depanare)”. Repetă pașii, apoi trimite din nou jurnalul.
Atenție: jurnalul detaliat poate conține căi de fișiere. Deschide fișierul în Notepad și citește-l
înainte să-l trimiți; șterge liniile pe care nu vrei să le împarți. Valet nu trimite niciodată jurnalele singur.

Te rog să nu trimiți niciun fișier de-al tău.

Vlad
```

---

## 2. Un fișier a dispărut (urgent)

Răspunde în aceeași zi. Un fișier pierdut e singurul raport care poate încheia produsul (`docs/PLAN.md`, secțiunea 8, riscul 1). Întâi ajută omul să găsească fișierul, apoi adună ce trebuie ca să afli cauza.

```
Bună, {nume},

Îmi pare rău și vreau să rezolv asta. Valet nu șterge niciodată definitiv un fișier,
deci fișierul ar trebui să existe undeva. Te rog să verifici, în ordinea asta:

1. Coșul de reciclare. Dacă o regulă a trimis fișierul acolo, e în Coș. Caută în Coș numele fișierului.
   Dacă nu ai golit Coșul, îl poți restaura de acolo.
2. Valet, Istoric. Găsește lotul din momentul în care a dispărut fișierul. Fiecare rând arată unde era
   fișierul și unde a ajuns. Deschide folderul de destinație. Dacă numele era ocupat acolo,
   fișierul are un număr în nume, de exemplu „raport (2).pdf”.
3. Anulare. În Istoric, butonul „Anulează lotul” de pe lot pune fișierele înapoi, cât timp
   nu au fost modificate între timp. (În planul Gratuit se poate anula doar ultimul lot;
   dacă lotul e mai vechi, Istoricul tot arată destinația, iar fișierul îl poți muta înapoi cu mâna.)
4. Căutarea Windows după numele fișierului, în caz că l-ai mutat tu sau alt program.

Dacă nu-l găsești, te rog să-mi trimiți:

- versiunea Valet (Setări, Despre) și versiunea de Windows;
- numele regulii care era activă și ce scrie în rândul din Istoric (o captură de ecran a rândului e de ajuns);
- dacă fișierul era pe OneDrive, Dropbox, pe un disc USB sau într-un folder de rețea;
- fișierul de jurnal: %LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log
  În mod normal nu are căi de fișiere. Dacă ai pornit jurnalul detaliat, poate avea, deci citește-l întâi.

Te rog să nu închizi Valet și să nu golești Coșul de reciclare până nu aflăm mai mult, și să nu-mi trimiți fișierul în sine.

Vlad
```

Note interne pentru acest caz:

- Jurnalul de operații (`valet.sqlite3` din folderul de date) are calea completă a fiecărei operații. Nu cere baza de date din oficiu: conține căi și sume de control pentru multe fișiere. Dacă e nevoie, cere întâi acordul omului și cere doar rândurile lotului în cauză.
- Un fișier care a ajuns în Coșul de reciclare nu mai poate fi restaurat de Valet după ce Coșul a fost golit (`docs/SAFETY.md`, invariantul 4). Nu promite altă recuperare.
- Ține o evidență a acestor rapoarte, cu versiunea Valet și rezultatul. Ținta planului pentru primele trei luni e zero rapoarte de fișiere pierdute (`docs/PLAN.md`, secțiunea 0).

---

## 3. Cerere de funcție

```
Bună, {nume},

Mulțumesc pentru idee. Ca s-o înțeleg bine: ce ai vrea să facă Valet, pentru ce fel de fișiere,
și ce faci acum în loc? Un exemplu (tip de fișier, folder, unde ar trebui să ajungă) ajută mai mult decât o descriere.

Țin o listă de cereri și o citesc toată. Nu pot spune dacă sau când se va face.

Vlad
```

Note: redenumirea fișierelor după un șablon e prima funcție planificată după lansare. Etichetele nu sunt în plan. Acțiunile din modulul de spațiu pe disc sunt planificate pentru o fază ulterioară. Spune „planificat”, niciodată o dată.

---

## 4. Cerere de rambursare

```
Bună, {nume},

[VLAD DECIDE: politica de rambursare. Completează înainte de prima vânzare: numărul de zile,
cine face rambursarea (procesatorul de plăți sau Vlad) și ce se întâmplă cu licența după rambursare.
Nimic din depozit nu stabilește asta încă. Nu trimite acest șablon până nu e completat.]

Ca să caut comanda ta, am nevoie de adresa de email folosită la cumpărare și, dacă o ai, de numărul comenzii.

Vlad
```

---

## 5. Licența nu se activează

```
Bună, {nume},

Hai să aflăm de ce. Ce spune Valet când imporți licența (propoziția exactă sau o captură de ecran)?
Cauze frecvente:

- „Prea multe activări”. O licență merge pe cel mult 3 calculatoare. Dacă nu mai folosești unul dintre ele,
  deschide Valet pe acel calculator, mergi la Setări, șterge licența și importă-o din nou aici.
  Ștergerea eliberează locul acelui calculator. Dezinstalarea Valet nu îl eliberează.
  Dacă vechiul calculator nu mai există (stricat, formatat), scrie-mi înapoi și spune-mi; mă uit eu manual.
- Fără internet în momentul importului. Importul are nevoie de o conexiune la serverul de licențe.
  După aceea, Valet verifică din nou cel mult o dată la 30 de zile. Dacă conexiunea cade mai târziu,
  Pro merge în continuare până la 60 de zile de la ultima verificare reușită, apoi aplicația merge ca Gratuit
  (nu se pierde nimic) până la următoarea verificare reușită.
- „Licența nu acoperă această versiune”. Licența ta include actualizările pentru 12 luni de la cumpărare.
  Mesajul numește data. Versiunile apărute până la acea dată rămân Pro; cele de după cer
  reînnoirea actualizărilor (15 € pe an). Versiunile mai vechi merg în continuare.
- Fișierul a fost schimbat sau deteriorat (de exemplu deschis și salvat într-un editor). Folosește fișierul
  .valet-license original din email.

Verificarea trimite doar identificatorul licenței, un identificator al calculatorului (un hash într-un singur sens)
și versiunea aplicației. Detalii: pagina de confidențialitate.

Te rog să-mi spui și versiunea Valet (Setări, Despre) și versiunea de Windows.

Vlad
```

[VLAD DECIDE: ce se face când vechiul calculator nu mai există. Contractul serviciului de licențe nu are resetare făcută de utilizator; mai sus se presupune o resetare manuală, care trebuie să fie posibilă ca propoziția să rămână.]

---

## 6. Întrebare despre SmartScreen

```
Bună, {nume},

Avertismentul e normal. Instalatorul Valet nu e semnat digital încă, iar Windows SmartScreen arată
„Windows protected your PC” pentru programe nesemnate și încă puțin descărcate.
Nu e rezultatul unei analize a lui Valet. Un certificat de semnare gratuit pentru proiecte open-source
nu a fost obținut încă, deci nu pot spune când dispare avertismentul.

Ca să continui: apasă „More info”, apoi „Run anyway”.

Dacă vrei să verifici fișierul înainte:
- Valet e open source (licență MIT): https://github.com/OsztrovszkyVlad/Valet
- În PowerShell: Get-FileHash .\Valet_<versiune>_x64-setup.exe -Algorithm SHA256
  și compară rezultatul cu suma SHA-256 listată de rularea „Release” din GitHub Actions.

Dacă „Run anyway” nu apare, calculatorul tău poate fi setat de un administrator (sau de modul S din Windows)
să blocheze programele nesemnate. Nu pot schimba asta de aici.

Vlad
```
