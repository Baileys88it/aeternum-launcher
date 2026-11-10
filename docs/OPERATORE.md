# Guida per chi gestisce il reame

Guida in italiano per compilare questo launcher contro il proprio `bnetserver` e `worldserver` della famiglia TrinityCore Forever. La versione inglese è [OPERATOR.md](OPERATOR.md).

Questo repository non contiene il client di gioco né i sorgenti di TrinityCore. Percorsi come `src/server/bnetserver/CMakeLists.txt` stanno nell'albero del **vostro** server, non qui.

Non mettere indirizzi pubblici o di LAN reali, password, token o righe `DatabaseInfo` in git, nelle issue o in chat. Gli esempi usano i segnaposto `SET_SERVER_ADDRESS` e `<portal-host>`.

## Cosa fa il launcher

Al login e su Play apre **HTTPS** verso LoginREST:

```text
https://<portal-host>:<rest-port>/bnetserver/...
```

L'URL è costruito in `app/src-tauri/src/api.rs`. Il launcher non accetta HTTP in chiaro. Il TLS si fida **solo** della CA incorporata in compilazione (`tls_built_in_root_certs(false)` più quella CA). Su Windows lo stack HTTP è la TLS di sistema (Schannel). Su Linux e macOS è rustls.

Il nome del portale in quella richiesta è fissato sull'indirizzo compilato (`FOREVER_SERVER_ADDRESS`, opzionale `FOREVER_SERVER_LAN_ADDRESS`). Il launcher non ha bisogno del file hosts per la propria chiamata di login. Il **gioco** sì: `Wow.exe` risolve il portale con il DNS normale.

Dopo un login riuscito, Play:

1. Scambia il token del launcher con un ticket (`/bnetserver/launcher/login/`).
2. Scrive una riga hosts per il portale se manca (richiesta di amministratore). Su Windows installa anche la CA incorporata nell'archivio radice del computer.
3. Scrive `WTF/AeternumLauncher.wtf` con `SET portal "<portal-host>"` (solo il nome host, senza porta).
4. Scrive le opzioni di avvio di Battle.net che il client legge con `-launcherlogin`: `CONNECTION_STRING` è `<portal-host>:<bnet-port>`, più il ticket e l'account di gioco. Su Windows è `HKCU\Software\Blizzard Entertainment\Battle.net\Launch Options\WoW`. Su macOS sono le preferenze `net.battle` (vedi [MACOS.md](MACOS.md)).
5. Avvia `Wow.exe` o `WowB.exe` con `-launcherlogin -config AeternumLauncher.wtf`.

`world_port` è solo la porta TCP che la spia di stato prova. Il launcher non la scrive nel client. Dopo il login il client usa l'indirizzo del reame che arriva da `bnetserver` (la riga `realmlist` sul server).

La patch della chiave in memoria usa la chiave pubblica Ed25519 di serie di TrinityCore e il region group 8 (`STOCK_PUBLIC_KEY` e `region_group` in `app/src-tauri/src/config.rs`). Non c'è una variabile di compilazione per cambiarle. Vanno bene per un server Forever di serie. Un server con un'altra chiave non accetta il client finché non modificate quelle costanti.

L'auto-aggiornamento è spento, salvo che `AETERNUM_UPDATE_URL` sia un URL `https://` al momento della compilazione. In questo albero non c'è un workflow di GitHub Actions.

## Perché `CN=*.*` blocca il login

I file di serie `bnetserver.cert.pem` e `bnetserver.key.pem` di Trinity sono un wildcard di sviluppo. Il certificato è `CN=*.*`, emesso da `TrinityCore Battle.net Aurora CA`.

Quando `bnetserver` vede quel wildcard di sviluppo imposta `UsesDevWildcardCertificate()` e `LoginHttpSession` serve **HTTP**, non TLS. Questo launcher parla solo HTTPS, quindi la chiamata aspetta fino al timeout. L'interfaccia mostra allora **server non raggiungibile** / **server unreachable** (chiave `unreachable` in `app/src-tauri/src/i18n.rs`). Un errore TLS che non è una connessione fallita né un timeout compare invece come errore di rete.

Una ricompilazione di `bnetserver` può copiare di nuovo quei PEM predefiniti sopra i vostri certificati. Nell'albero TrinityCore, `src/server/bnetserver/CMakeLists.txt` ha un passo `POST_BUILD` con `cmake -E copy` dei PEM di serie nell'output di build. Tenete i file firmati **fuori** da quella cartella, oppure fate eseguire la copia solo se la destinazione manca. Dopo ogni build del server, controllate che il certificato accanto al `bnetserver` in esecuzione non sia `CN=*.*`:

```sh
openssl x509 -in bnetserver.cert.pem -noout -subject -issuer
```

CN e SAN devono essere il nome host del portale, e l'emittente deve essere la vostra CA privata.

## Certificati

I due script creano gli stessi file:

| File | Chi lo riceve |
|---|---|
| `certs/forever.cert.pem` | Catena foglia + CA. Sul server diventa `bnetserver.cert.pem`. |
| `certs/forever.key.pem` | Chiave privata del server. Sul server diventa `bnetserver.key.pem`. Non condividetela. |
| `certs/ca/forever_root_ca.crt` | CA pubblica. Giocatori e launcher. Si può distribuire. |
| `certs/ca/forever_root_ca.key` | Chiave privata della CA. Non condividetela e non committatela. |

`certs/*`, `*.pem` e `*.key` sono in gitignore. Il file pubblico `app/src-tauri/resources/forever_root_ca.crt` **non** è ignorato. In un clone nuovo è una CA segnaposto con subject `CN=Aeternum Launcher Placeholder CA` e vincolo di nome `example.invalid`. Il login non si fiderà del **vostro** server finché non la sostituite.

La CA creata dagli script è `CN=Forever Private Root CA` con `nameConstraints` che permettono solo `<portal-domain>` e i nomi sotto di esso. Il nome server di esempio è `<portal-host>` (CN e SAN, più `DNS:*.<portal-domain>`). È un esempio, non un reame compilato nel launcher. Per un altro dominio padre, modificate **entrambe** le righe `nameConstraints` nello script **prima** che esista `forever_root_ca.key`. Una CA già creata viene riusata e tiene i vincoli vecchi.

Linux / macOS (OpenSSL nel `PATH`):

```sh
./scripts/gen-certs.sh
./scripts/gen-certs.sh <portal-host> 825
./scripts/gen-certs.sh 825
```

Il primo argomento è il nome host quando non è fatto solo di cifre. Un numero da solo è la validità in giorni (predefinito 825). La CA dura 3650 giorni.

Windows (`cmd.exe`), con `openssl` nel `PATH` oppure con Git for Windows (`Program Files\Git\usr\bin\openssl.exe`) o OpenSSL-Win64 (`Program Files\OpenSSL-Win64\bin\openssl.exe`):

```bat
scripts\generate-certs.bat
scripts\generate-certs.bat <portal-host> 825
scripts\generate-certs.bat 825
```

`generate-certs.bat` imposta `MSYS2_ARG_CONV_EXCL=*` così l'OpenSSL di Git non trasforma `/CN=...` in un percorso.

### Installazione su bnetserver

Copiate catena e chiave del server nella cartella di **esecuzione** accanto a `bnetserver` (quella da cui lo avviate, non un output di CMake che `POST_BUILD` sovrascrive):

```text
forever.cert.pem  ->  bnetserver.cert.pem
forever.key.pem   ->  bnetserver.key.pem
```

Ai giocatori date solo `forever_root_ca.crt`. La importano come radice attendibile, oppure lasciano che il launcher installi la copia che avete incorporato. Non date mai `forever_root_ca.key`.

L'entrypoint Docker opzionale di questo repo non usa quei due nomi. `docker/entrypoint.sh` imposta `CertificatesFile` e `PrivateKeyFile` su `forever.cert.pem` e `forever.key.pem` nella cartella certificati del container. Un server Windows nativo che lascia quelle chiavi vuote usa `bnetserver.cert.pem` e `bnetserver.key.pem` accanto all'eseguibile. In entrambi i casi il certificato che il processo carica deve essere il vostro, non `CN=*.*`.

### Far fidare il launcher della stessa CA

`app/src-tauri/build.rs` incorpora la CA in quest'ordine:

1. Variabile d'ambiente `FOREVER_CA_CERT`: il **testo** PEM del certificato, se non è vuoto.
2. Altrimenti `app/src-tauri/resources/forever_root_ca.crt`.

Il PEM finisce nell'output di build e viene compilato come `CA_PEM` (`app/src-tauri/src/config.rs`). Dopo la generazione, sostituite il file risorsa con `certs/ca/forever_root_ca.crt`, **oppure** passate il testo:

```sh
export FOREVER_CA_CERT="$(cat certs/ca/forever_root_ca.crt)"
```

Non mettete `forever_root_ca.key` in `FOREVER_CA_CERT`. Controllate il subject prima di compilare:

```sh
openssl x509 -in app/src-tauri/resources/forever_root_ca.crt -noout -subject
```

Deve essere la vostra CA, non `Aeternum Launcher Placeholder CA`, se il certificato del server è stato firmato dallo script.

## bnetserver.conf

`LoginREST.ExternalAddress` e `LoginREST.LocalAddress` devono essere il **nome host** del portale, la stessa stringa di CN/SAN del certificato. Non devono essere `127.0.0.1`. Anche `bnetserver` deve risolvere quel nome (il compose Docker aggiunge una riga `extra_hosts` per questo). Mettete una riga hosts o un DNS anche sulla macchina del server. In ciò che committate usate comunque un segnaposto:

```text
SET_SERVER_ADDRESS <portal-host>
```

Impostate `LoginREST.Port` e `BattlenetPort` sulle porte che compilerete nel launcher. Se omettete `BattlenetPort`, il valore solito delle dist di Trinity è 1119, che è anche la porta usata dal client Wow quando la stringa del portale non ha una porta. Un reame in ascolto su un'altra porta Battle.net **deve** essere scritto `host:port` per il client. Questo launcher scrive sempre `CONNECTION_STRING` come `<FOREVER_PORTAL>:<FOREVER_BNET_PORT>`.

Fate coincidere le tre porte del launcher con il server:

| Variabile di build | Server |
|---|---|
| `FOREVER_PORTAL` | Solo il nome host. Senza `:port`. Esempio: `<portal-host>`. |
| `FOREVER_BNET_PORT` | `BattlenetPort` (login Battle.net). |
| `FOREVER_REST_PORT` | `LoginREST.Port`. |
| `FOREVER_WORLD_PORT` | Porta di gioco del worldserver, quella che la spia di stato prova. La realm list deve annunciare un indirizzo world raggiungibile dai giocatori. |
| `FOREVER_SERVER_ADDRESS` | IPv4 o nome DNS a cui il launcher si collega e che scrive nel file hosts dei giocatori. Negli esempi: `SET_SERVER_ADDRESS`. |
| `FOREVER_SERVER_LAN_ADDRESS` | Secondo indirizzo opzionale, provato se quello pubblico non accetta TCP sulla porta REST. |
| `FOREVER_SERVER_NAME` | Etichetta opzionale nel launcher. Vuota mostra «Reame non configurato». |
| `FOREVER_WEBSITE` | Opzionale. Vuoto nasconde ogni pulsante del sito. Non è un’autorità sulla versione del client. |

`AETERNUM_PORTAL`, `AETERNUM_BNET_PORT`, `AETERNUM_REST_PORT`, `AETERNUM_WORLD_PORT`, `AETERNUM_SERVER_ADDRESS`, `AETERNUM_SERVER_LAN_ADDRESS`, `AETERNUM_SERVER_NAME` e `AETERNUM_WEBSITE` sono alias, controllati per primi. Lasciateli vuoti se uno script vecchio non li imposta già.

Non impostato vuol dire non impostato. Porta `0`, portale vuoto, indirizzo vuoto o sito vuoto non vengono sostituiti con un host di produzione, con le vecchie porte Aeternum o con un sito pubblico. Il login fallisce allora con **server non configurato: imposta portale, porte e indirizzo in compilazione**.

Salvate `bnetserver.conf` e `worldserver.conf` in **UTF-8 senza BOM**. Un BOM (tipico quando il Blocco note dice «UTF-8») sta davanti alla prima chiave e il server legge male il file. Non incollate `LoginDatabaseInfo`, `WorldDatabaseInfo` o altre righe `DatabaseInfo` in git o in chat. Contengono la password del database.

## Hosts e DNS per i giocatori

Il nome host del portale deve risolversi nell'indirizzo raggiungibile del server su ogni PC dei giocatori. Due modi:

- un DNS che controllate, oppure
- una riga hosts: `SET_SERVER_ADDRESS <portal-host>`

Il launcher può scrivere quella riga (Impostazioni, Ripara, oppure in automatico prima di Play). Usa l'indirizzo compilato, mai un indirizzo digitato nell'interfaccia. Non c'è una pagina Impostazioni per il reame. `settings.json` non può sostituirlo.

Non committate un indirizzo reale. `SET_SERVER_ADDRESS` in questo documento non è un nome da pubblicare: sostituitelo solo sulle macchine che ne hanno bisogno.

## Launcher e Wow.exe

Avviare `Wow.exe` da solo parla con Battle.net su HTTPS porta 443. Non apre le vostre porte private di bnet o world, e non ha il ticket del launcher. Se la stringa del portale non ha una porta esplicita, il client usa **1119**.

I giocatori avviano il gioco da questo launcher, così può emettere il ticket e impostare `CONNECTION_STRING` su `host:port`.

## Compilazione

Da `app/src-tauri`, con le variabili nella stessa shell:

```sh
export FOREVER_PORTAL=<portal-host>
export FOREVER_BNET_PORT=1119
export FOREVER_REST_PORT=8081
export FOREVER_WORLD_PORT=8085
export FOREVER_SERVER_ADDRESS=SET_SERVER_ADDRESS
export FOREVER_SERVER_NAME="Reame di esempio"
export FOREVER_CA_CERT="$(cat ../../certs/ca/forever_root_ca.crt)"
cargo build --release
```

I numeri di porta qui sopra sono **esempi** delle porte solite delle dist Trinity, non i valori predefiniti di questo launcher. Usate le porte del vostro `bnetserver.conf` e del worldserver.

`cargo build --release` incorpora `app/ui` e produce il binario. Un installer NSIS (in `tauri.conf.json` il target è `nsis`) richiede la CLI Tauri 2: `cargo tauri build` in `app/src-tauri`. Una build GUI su Linux richiede i pacchetti WebKitGTK chiesti da Tauri 2 (di solito `libwebkit2gtk-4.1-dev`). In questo albero non ci sono `package.json` né un workflow launcher in `.github/workflows`.

Cambiare una variabile `FOREVER_*` richiede una ricompilazione. I giocatori non possono puntare un exe già fatto verso un altro reame.

Test della libreria, senza un reame configurato:

```sh
cd app/src-tauri && cargo test --lib
```

## Sito web

`FOREVER_WEBSITE` è vuoto se non lo impostate. Serve solo da link per i pulsanti del sito. Non è un’autorità sulla versione del client, e il launcher non gli chiede un elenco di build.

- Vuoto: il pulsante «Apri il sito» e il link «Crea un account» restano nascosti. Non si apre nulla.
- Impostato su un’origine: compaiono i due controlli. «Apri il sito» apre quell’origine. «Crea un account» apre `<website>/register` (`app/ui/app.js`).

Play non aspetta un sito per decidere se il client installato è troppo vecchio o troppo recente. Un controllo di versione remoto contro l’API di un sito è fuori da questo launcher di base. Chi gestisce un reame e lo vuole lo aggiunge nella propria build, contro il proprio sito. Il reame può comunque rifiutare un client al login.

L’app FastAPI in `web/` è il vecchio pacchetto server. Il modulo di iscrizione è su `/` e accetta **POST** `/register`. Questo launcher non ne ha bisogno.

## Altri file in questo albero

`docker-compose.yml`, `web/` e `docs/DOKPLOY.md` sono il vecchio pacchetto server Forever. Non servono per compilare il launcher, e il launcher non li legge.

Il compose pubblica TCP **1119**, **8081** e **8085**. `docker/entrypoint.sh` imposta `LoginREST.ExternalAddress` e `LoginREST.LocalAddress` da `LOGIN_HOSTNAME`, il cui valore predefinito è `<portal-host>`. Non imposta `BattlenetPort` né `LoginREST.Port`. Punta il certificato a `forever.cert.pem` / `forever.key.pem`. In quel compose MySQL e SOAP non sono sulle porte pubbliche; non pubblicateli.

Creazione account dalla console del worldserver, se la usate:

```text
bnetaccount create mail@example.com change-me
```

Non committate quella password, e non committate `.env`.

## Controllo finale

1. Generate una CA e un certificato server il cui CN e SAN sono il nome host di LoginREST.
2. Installate catena e chiave dove il `bnetserver` in esecuzione le carica, e impedite che `CN=*.*` venga ricopiato.
3. Impostate `LoginREST.ExternalAddress` e `LoginREST.LocalAddress` su quel nome host. Salvate il conf in UTF-8 senza BOM.
4. Fate risolvere il nome host in `SET_SERVER_ADDRESS` sul server e sui PC dei giocatori.
5. Ricompilate il launcher con `FOREVER_PORTAL`, le tre porte, `FOREVER_SERVER_ADDRESS` e `FOREVER_CA_CERT` (oppure la CA risorsa sostituita). Lasciate `FOREVER_WEBSITE` vuoto se non volete i pulsanti del sito. Non controlla la build del client.
6. I giocatori entrano e premono Play nel launcher. Non avviano `Wow.exe` da solo.
