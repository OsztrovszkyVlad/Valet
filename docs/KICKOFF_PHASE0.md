# Prompt de pornire pentru Faza 0

Se lipește o singură dată în Claude Code, din `C:\osz\Projects\Valet`, după ce `CLAUDE.md`, `.claude/` și `docs/PLAN.md` sunt în repo și committed. Fazele următoare pornesc cu `/phase N` după „Faza N-1 aprobată”.

---

Lucrăm pe Valet. Tu ești managerul de proiect; eu sunt proprietarul. Regulile de lucru sunt în CLAUDE.md, planul produsului este în docs/PLAN.md, subagenții sunt în .claude/agents/, skill-urile în .claude/skills/. Citește-le pe toate înainte să faci orice.

Înainte de orice modificare:
1. Confirmă-mi în maximum 20 de rânduri că ai înțeles: ce este Valet, care sunt cei 7 invarianți de siguranță, ce înseamnă „o fază se termină” și ce nu ai voie să faci fără mine.
2. Verifică din terminal că hook-urile merg: rulează o comandă inofensivă (`cargo --version`) și apoi încearcă intenționat `git push --force` ca să confirmi că e blocată. Raportează rezultatul. Dacă hook-urile nu rulează (PowerShell, execution policy), spune-mi exact ce trebuie să fac eu.
3. Rulează `claude plugin validate .claude/agents` dacă comanda există, sau verifică manual că toate cele 7 fișiere de agent au `name` și `description`.

Apoi rulează `/phase 0`.

Context pe care nu-l găsești în cod:
- README.md actual descrie altă aplicație și DEVELOPMENT_COMPLETE.md exagerează; nu te baza pe ele.
- Doar Windows acum. macOS mai târziu: nu scrie cod specific Mac, dar nu băga cod Windows în valet-core.
- Fără AI în cod până la Faza 6. Fără funcții noi în Faza 0: doar adevăr, curățenie, structură, teste de caracterizare, CI verde.
- Textele de interfață în română; codul în engleză. Fără vocabular de marketing.
- Commit direct pe main. Fără branch-uri, fără PR-uri.
- Când Faza 0 e gata, scrie docs/reports/phase-0.md după șablonul din skill-ul `phase`, fă commit și oprește-te. Nu începe Faza 1. Eu rulez checklist-ul manual și îți spun „Faza 0 aprobată” sau ce e de corectat.
