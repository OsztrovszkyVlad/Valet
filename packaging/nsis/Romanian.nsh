; Romanian strings for the messages Tauri adds to its NSIS installer template.
; The Tauri bundler ships no Romanian file, so tauri.conf.json points
; bundle > windows > nsis > customLanguageFiles > Romanian here.
; Keys follow tauri-bundler's languages/English.nsh. Both `choowHowToInstall`
; (bundler <= 2.6) and `chooseHowToInstall` (later) are defined so a CLI
; upgrade does not leave an untranslated string.
LangString addOrReinstall ${LANG_ROMANIAN} "Adaugă/Reinstalează componentele"
LangString alreadyInstalled ${LANG_ROMANIAN} "Deja instalat"
LangString alreadyInstalledLong ${LANG_ROMANIAN} "${PRODUCTNAME} ${VERSION} este deja instalat. Alege operația dorită și apasă Înainte pentru a continua."
LangString appRunning ${LANG_ROMANIAN} "{{product_name}} rulează. Închide aplicația și încearcă din nou."
LangString appRunningOkKill ${LANG_ROMANIAN} "{{product_name}} rulează.$\nApasă OK pentru a o închide forțat"
LangString chooseMaintenanceOption ${LANG_ROMANIAN} "Alege operația de întreținere."
LangString choowHowToInstall ${LANG_ROMANIAN} "Alege cum vrei să instalezi ${PRODUCTNAME}."
LangString chooseHowToInstall ${LANG_ROMANIAN} "Alege cum vrei să instalezi ${PRODUCTNAME}."
LangString createDesktop ${LANG_ROMANIAN} "Creează o scurtătură pe desktop"
LangString dontUninstall ${LANG_ROMANIAN} "Nu dezinstala"
LangString dontUninstallDowngrade ${LANG_ROMANIAN} "Nu dezinstala (instalarea unei versiuni mai vechi fără dezinstalare este dezactivată pentru acest instalator)"
LangString failedToKillApp ${LANG_ROMANIAN} "{{product_name}} nu a putut fi închisă. Închide aplicația și încearcă din nou"
LangString installingWebview2 ${LANG_ROMANIAN} "Se instalează WebView2..."
LangString newerVersionInstalled ${LANG_ROMANIAN} "O versiune mai nouă de ${PRODUCTNAME} este deja instalată. Nu este recomandat să instalezi o versiune mai veche. Dacă vrei totuși versiunea mai veche, dezinstalează mai întâi versiunea curentă. Alege operația dorită și apasă Înainte pentru a continua."
LangString older ${LANG_ROMANIAN} "mai veche"
LangString olderOrUnknownVersionInstalled ${LANG_ROMANIAN} "Pe sistem este instalată o versiune $R4 de ${PRODUCTNAME}. Este recomandat să dezinstalezi versiunea curentă înainte de instalare. Alege operația dorită și apasă Înainte pentru a continua."
LangString silentDowngrades ${LANG_ROMANIAN} "Instalarea unei versiuni mai vechi este dezactivată pentru acest instalator; instalarea silențioasă nu poate continua. Folosește instalatorul cu interfață grafică.$\n"
LangString unableToUninstall ${LANG_ROMANIAN} "Dezinstalarea a eșuat."
LangString uninstallApp ${LANG_ROMANIAN} "Dezinstalează ${PRODUCTNAME}"
LangString uninstallBeforeInstalling ${LANG_ROMANIAN} "Dezinstalează înainte de instalare"
LangString unknown ${LANG_ROMANIAN} "necunoscută"
LangString webview2AbortError ${LANG_ROMANIAN} "Instalarea WebView2 a eșuat. Aplicația nu poate rula fără WebView2. Repornește instalatorul."
LangString webview2DownloadError ${LANG_ROMANIAN} "Eroare: descărcarea WebView2 a eșuat - $0"
LangString webview2DownloadSuccess ${LANG_ROMANIAN} "Programul de instalare WebView2 a fost descărcat"
LangString webview2Downloading ${LANG_ROMANIAN} "Se descarcă programul de instalare WebView2..."
LangString webview2InstallError ${LANG_ROMANIAN} "Eroare: instalarea WebView2 a eșuat cu codul $1"
LangString webview2InstallSuccess ${LANG_ROMANIAN} "WebView2 a fost instalat"
LangString deleteAppData ${LANG_ROMANIAN} "Șterge datele aplicației"
