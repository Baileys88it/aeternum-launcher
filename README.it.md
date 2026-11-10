<p align="center"><img src="docs/assets/aeternum-logo.png" alt="Aeternum" width="220"></p>

# Base del launcher per reami TrinityCore della famiglia Forever

**Server:** [Aeternum Engine](../../../aeternum-core) è il motore server abbinato a questo launcher.

Launcher [Tauri](https://tauri.app) 2 (Rust e una UI HTML/CSS/JS) che fa entrare un giocatore in un portale Battle.net privato (`bnetserver`) e avvia il client di World of Warcraft con un ticket di login. Si compila una volta per il proprio reame.

Deriva dal launcher open source Forever. Il titolo della finestra dice ancora Aeternum. Il reame no: host, porte, indirizzo e sito restano vuoti finché non li impostate in compilazione.

## Cosa non è

- Non contiene il client di gioco, i dati di gioco, né un link per scaricarli.
- Non ha un reame già configurato. Nel sorgente non ci sono un hostname di produzione, un sito pubblico o un set fisso di porte.
- Non richiede un sito. I pulsanti del sito restano nascosti se non ne impostate uno.
- Nessun controllo remoto della versione del client. Il launcher non chiede a un sito se il client installato è troppo vecchio o troppo recente. Quel controllo è fuori da questa base: chi gestisce un reame lo aggiunge per il proprio sito.

## Cache del gioco

A ogni clic su Gioca il launcher svuota la cartella `Cache` del client (WDB/ADB) dentro la cartella del gioco prima di avviarlo, così il client non tiene dati vecchi di creature, missioni od oggetti. Tocca solo quella cartella; i file bloccati vengono saltati e un errore non blocca mai l'avvio. Si disattiva nelle Impostazioni: "Svuota cache all'avvio".

## Addon segnalino missioni

A ogni clic su Gioca il launcher installa o aggiorna l'addon del client **AeternumQuestTag** in `Interface\AddOns\AeternumQuestTag` dentro la cartella del gioco (solo quando la versione incorporata cambia). L'addon scrive **QUEST** in giallo sopra la targhetta delle creature che servono alle tue missioni attive: quelle da uccidere e quelle che droppano un oggetto richiesto da un obiettivo non completato. In gioco `/aqt` lo attiva o disattiva (salvato per personaggio). È attivo di default; si disattiva nelle Impostazioni: "Addon segnalino missioni" (il launcher toglie la cartella che aveva installato). Gli altri addon non vengono mai toccati. I dati delle missioni vengono dal database world: si rigenerano con `scripts/gen-questtag-data.ps1` (vedi l'intestazione dello script) e poi si ricompila il launcher. Test dell'addon: `addon/tests/run.lua` (Lua 5.1, da `addon/`).

## Addon di terzi

Questo repository non include addon di terzi. L'unico addon nell'albero è **AeternumQuestTag**, descritto sopra.

## Documentazione

| | |
|---|---|
| Percorso di connessione, TLS, certificati, configurazione, build | [docs/OPERATOR.md](docs/OPERATOR.md) (inglese) |
| La stessa guida in italiano | [docs/OPERATORE.md](docs/OPERATORE.md) |
| Copia del gioco su macOS | [docs/MACOS.md](docs/MACOS.md) |

Script dei certificati: [`scripts/gen-certs.sh`](scripts/gen-certs.sh) e [`scripts/generate-certs.bat`](scripts/generate-certs.bat). Sono descritti nella guida. Non committate le chiavi private che scrivono.

`docker-compose.yml`, `web/` e [docs/DOKPLOY.md](docs/DOKPLOY.md) sono il vecchio pacchetto server ereditato con l'albero. Il launcher non ne ha bisogno.

## Licenza

GNU GPL v2.0 o successiva, la stessa licenza di TrinityCore e del launcher Forever da cui questo programma deriva. Vedi [LICENSE](LICENSE).

Non è affiliato né approvato da Blizzard Entertainment. World of Warcraft e Battle.net sono marchi di Blizzard Entertainment, Inc.
