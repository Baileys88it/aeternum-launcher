# Operator guide

English guide for building this launcher against your own TrinityCore Forever-family `bnetserver` and `worldserver`. The Italian version is [OPERATORE.md](OPERATORE.md).

This repository does not contain the game client or the TrinityCore sources. Paths such as `src/server/bnetserver/CMakeLists.txt` are in **your** server tree, not here.

Do not put real public or LAN addresses, passwords, tokens, or `DatabaseInfo` lines in git, issues, or chat. Examples below use the placeholders `SET_SERVER_ADDRESS` and `<portal-host>`.

## What the launcher actually does

On login and on Play it opens **HTTPS** to LoginREST:

```text
https://<portal-host>:<rest-port>/bnetserver/...
```

That URL is built in `app/src-tauri/src/api.rs`. The launcher refuses clear HTTP. TLS trusts **only** the CA certificate embedded at build time (`tls_built_in_root_certs(false)` plus that one CA). On Windows the HTTP stack is the system TLS library (Schannel). On Linux and macOS it is rustls.

The portal hostname in that request is pinned to the address you compiled in (`FOREVER_SERVER_ADDRESS`, optional `FOREVER_SERVER_LAN_ADDRESS`). The launcher does not need the hosts file for its own login call. The **game** does: `Wow.exe` resolves the portal with ordinary DNS.

After a successful launcher login the Play button:

1. Exchanges the saved launcher token for a login ticket (`/bnetserver/launcher/login/`).
2. Writes a hosts line for the portal if it is missing (administrator prompt). On Windows it also installs the embedded CA into the machine root store.
3. Writes `WTF/AeternumLauncher.wtf` with `SET portal "<portal-host>"` (hostname only, no port).
4. Writes the Battle.net launch options the client reads with `-launcherlogin`: `CONNECTION_STRING` is `<portal-host>:<bnet-port>`, plus the ticket and the game account. On Windows that is `HKCU\Software\Blizzard Entertainment\Battle.net\Launch Options\WoW`. On macOS it is the `net.battle` preferences (see [MACOS.md](MACOS.md)).
5. Starts `Wow.exe` or `WowB.exe` with `-launcherlogin -config AeternumLauncher.wtf`.

`world_port` is only the TCP port the status light probes. The launcher does not write it into the client. After login the client uses the realm address from `bnetserver` (the `realmlist` row on the server).

The in-memory key patch uses the stock TrinityCore Ed25519 public key and region group 8 (`STOCK_PUBLIC_KEY` and `region_group` in `app/src-tauri/src/config.rs`). There is no build-time switch for those. They match a stock Forever-family server. A server with a different key will not accept the client until you change those constants.

Self-update is off unless `AETERNUM_UPDATE_URL` is an `https://` URL at build time. This tree has no GitHub Actions workflow.

## Why `CN=*.*` breaks login

Stock Trinity files `bnetserver.cert.pem` and `bnetserver.key.pem` are a development wildcard. The certificate is `CN=*.*`, issued by `TrinityCore Battle.net Aurora CA`.

When `bnetserver` sees that dev wildcard it sets `UsesDevWildcardCertificate()` and `LoginHttpSession` serves **HTTP**, not TLS. This launcher only speaks HTTPS, so the call waits until it times out. The UI then shows **server unreachable** / **server non raggiungibile** (`unreachable` in `app/src-tauri/src/i18n.rs`). A TLS failure that is not a connect or a timeout is shown as a network error instead.

A rebuild of `bnetserver` can copy those default PEMs back over your certificates. In the TrinityCore tree, `src/server/bnetserver/CMakeLists.txt` has a `POST_BUILD` step that runs `cmake -E copy` of the stock PEMs into the build output. Keep your signed files **outside** that output directory, or change the copy so it runs only when the destination is missing. After every server build, confirm the certificate next to the running `bnetserver` is not `CN=*.*`:

```sh
openssl x509 -in bnetserver.cert.pem -noout -subject -issuer
```

You want your portal hostname as CN/SAN, and your private CA as the issuer.

## Certificates

Both scripts create the same files:

| File | Who gets it |
|---|---|
| `certs/forever.cert.pem` | Leaf plus CA chain. Copy to the server as `bnetserver.cert.pem`. |
| `certs/forever.key.pem` | Server private key. Copy to the server as `bnetserver.key.pem`. Never share it. |
| `certs/ca/forever_root_ca.crt` | Public CA. Players and the launcher. Safe to distribute. |
| `certs/ca/forever_root_ca.key` | CA private key. Never share it, never commit it. |

`certs/*`, `*.pem`, and `*.key` are gitignored. The public file `app/src-tauri/resources/forever_root_ca.crt` is **not** ignored. That file in a fresh clone is a placeholder CA with subject `CN=Aeternum Launcher Placeholder CA` and a name constraint of `example.invalid`. Login will not trust **your** server until you replace it.

The CA these scripts create is `CN=Forever Private Root CA` with `nameConstraints` permitting only `<portal-domain>` and names under it. The example server name is `<portal-host>` (CN and SAN, plus `DNS:*.<portal-domain>`). It is an example, not a realm baked into the launcher. To allow a different parent domain, edit **both** `nameConstraints` lines in the script **before** `forever_root_ca.key` exists. An existing CA is reused and keeps its old constraints.

Linux / macOS (OpenSSL on `PATH`):

```sh
./scripts/gen-certs.sh
./scripts/gen-certs.sh <portal-host> 825
./scripts/gen-certs.sh 825
```

The first argument is the hostname when it is not all digits. A single number is the validity in days (default 825). The CA itself lasts 3650 days.

Windows (`cmd.exe`), with `openssl` on `PATH` or under Git for Windows (`Program Files\Git\usr\bin\openssl.exe`) or OpenSSL-Win64 (`Program Files\OpenSSL-Win64\bin\openssl.exe`):

```bat
scripts\generate-certs.bat
scripts\generate-certs.bat <portal-host> 825
scripts\generate-certs.bat 825
```

`generate-certs.bat` sets `MSYS2_ARG_CONV_EXCL=*` so Git's OpenSSL does not rewrite `/CN=...` into a path.

### Install on bnetserver

Copy the chain and the server key onto the **runtime** directory next to `bnetserver` (the folder you start, not a CMake output that `POST_BUILD` will overwrite):

```text
forever.cert.pem  ->  bnetserver.cert.pem
forever.key.pem   ->  bnetserver.key.pem
```

Give players only `forever_root_ca.crt`. They import it as a trusted root, or they let the launcher install the copy you embedded. Never give them `forever_root_ca.key`.

This repo's optional Docker entrypoint does not use those two filenames. `docker/entrypoint.sh` sets `CertificatesFile` and `PrivateKeyFile` to `forever.cert.pem` and `forever.key.pem` inside the container certs directory. Native Windows servers that leave those keys unset use `bnetserver.cert.pem` and `bnetserver.key.pem` beside the executable. Either way, the leaf the process loads must be your certificate, not `CN=*.*`.

### Teach the launcher the same CA

`app/src-tauri/build.rs` embeds the CA in this order:

1. Environment variable `FOREVER_CA_CERT` — the PEM **text** of the certificate, if it is non-empty.
2. Otherwise `app/src-tauri/resources/forever_root_ca.crt`.

The PEM is written to the build output and compiled in as `CA_PEM` (`app/src-tauri/src/config.rs`). Replace the resource file with `certs/ca/forever_root_ca.crt` after you generate it, **or** pass the text:

```sh
export FOREVER_CA_CERT="$(cat certs/ca/forever_root_ca.crt)"
```

Do not put `forever_root_ca.key` in `FOREVER_CA_CERT`. Check the subject before you build:

```sh
openssl x509 -in app/src-tauri/resources/forever_root_ca.crt -noout -subject
```

It must be your CA, not `Aeternum Launcher Placeholder CA`, if the server certificate was signed by the script above.

## bnetserver.conf

`LoginREST.ExternalAddress` and `LoginREST.LocalAddress` must be the portal **hostname**, the same string as the certificate CN/SAN. They must not be `127.0.0.1`. `bnetserver` also has to resolve that hostname itself (the Docker compose file adds an `extra_hosts` entry for this). Use a hosts line or DNS on the server machine too. Still use a placeholder in anything you commit:

```text
SET_SERVER_ADDRESS <portal-host>
```

Set `LoginREST.Port` and `BattlenetPort` to the ports you will compile into the launcher. If you omit `BattlenetPort`, Trinity's usual dist value is 1119, which is also the port the Wow client uses when the portal string has no port. A realm that listens on any other Battle.net port **must** be written `host:port` for the client. This launcher always writes `CONNECTION_STRING` as `<FOREVER_PORTAL>:<FOREVER_BNET_PORT>`.

Match the three launcher ports to the server:

| Build variable | Server |
|---|---|
| `FOREVER_PORTAL` | Hostname only. No `:port`. Example: `<portal-host>`. |
| `FOREVER_BNET_PORT` | `BattlenetPort` (Battle.net login). |
| `FOREVER_REST_PORT` | `LoginREST.Port`. |
| `FOREVER_WORLD_PORT` | Worldserver game port the status light probes. The realm list must advertise a world address players can reach. |
| `FOREVER_SERVER_ADDRESS` | IPv4 or DNS name the launcher connects to and writes into the players' hosts file. Example value in docs only: `SET_SERVER_ADDRESS`. |
| `FOREVER_SERVER_LAN_ADDRESS` | Optional second address, tried when the public one does not accept TCP on the REST port. |
| `FOREVER_SERVER_NAME` | Optional label in the launcher. Empty shows "Realm not configured". |
| `FOREVER_WEBSITE` | Optional. Empty hides every site button. Not a client-version authority. |

`AETERNUM_PORTAL`, `AETERNUM_BNET_PORT`, `AETERNUM_REST_PORT`, `AETERNUM_WORLD_PORT`, `AETERNUM_SERVER_ADDRESS`, `AETERNUM_SERVER_LAN_ADDRESS`, `AETERNUM_SERVER_NAME`, and `AETERNUM_WEBSITE` are aliases, checked first. Leave them unset unless an old build script still sets them.

Unset means unset. Port `0`, a blank portal, a blank address, or a blank website are not replaced with a production host, the old Aeternum ports, or a public site. Login then fails with **server not configured: set the portal, ports, and address at build time**.

Save `bnetserver.conf` and `worldserver.conf` as **UTF-8 without a BOM**. A BOM (common when Notepad says "UTF-8") sits in front of the first key and the server misreads the file. Do not paste `LoginDatabaseInfo`, `WorldDatabaseInfo`, or any other `DatabaseInfo` line into git or chat. Those lines contain the database password.

## Hosts and DNS for players

The portal hostname must resolve to the reachable server address on every player PC. Two ways:

- DNS that you control, or
- a hosts line: `SET_SERVER_ADDRESS <portal-host>`

The launcher can write that line (Settings, Fix, or automatically before Play). It uses the compiled address, never an address typed into the UI. There is no Settings page for the realm. `settings.json` cannot override it.

Do not commit a real address. `SET_SERVER_ADDRESS` in this document is not a hostname you should publish; replace it only on the machines that need it.

## Launcher versus Wow.exe

Starting `Wow.exe` by itself talks to Battle.net on HTTPS port 443. It does not open your private bnet or world ports, and it does not have the launcher ticket. If the portal string has no explicit port, the client uses **1119**.

Players start the game from this launcher so it can issue the ticket and set `CONNECTION_STRING` to `host:port`.

## Build

From `app/src-tauri`, with the variables set in the same shell:

```sh
export FOREVER_PORTAL=<portal-host>
export FOREVER_BNET_PORT=1119
export FOREVER_REST_PORT=8081
export FOREVER_WORLD_PORT=8085
export FOREVER_SERVER_ADDRESS=SET_SERVER_ADDRESS
export FOREVER_SERVER_NAME="Example Realm"
export FOREVER_CA_CERT="$(cat ../../certs/ca/forever_root_ca.crt)"
cargo build --release
```

The port numbers above are **examples** of Trinity's usual dist ports, not defaults of this launcher. Use the ports in your `bnetserver.conf` and worldserver.

`cargo build --release` embeds `app/ui` and produces the binary. An NSIS installer (`tauri.conf.json` targets `nsis`) needs the Tauri 2 CLI: `cargo tauri build` in `app/src-tauri`. A Linux GUI build needs the WebKitGTK packages Tauri 2 asks for (commonly `libwebkit2gtk-4.1-dev`). This tree has no `package.json` and no `.github/workflows` launcher workflow.

Changing a `FOREVER_*` variable requires a rebuild. Players cannot point a finished exe at another realm.

Library tests, with no realm configured:

```sh
cd app/src-tauri && cargo test --lib
```

## Website

`FOREVER_WEBSITE` defaults to empty. It is only a link for the site buttons. It is not a client-version authority, and the launcher does not request a client-build list from it.

- Empty: the "Open website" button and the "Create one" link stay hidden. Nothing is opened.
- Set to an origin: those two controls are shown. "Open website" opens that origin. "Create one" opens `<website>/register` (`app/ui/app.js`).

Play does not wait on a website to decide whether the installed client is too old or too recent. Remote version gating against a website API is outside this base launcher. An operator who wants that check adds it in their own build, against their own site. The realm can still reject a client at login.

The FastAPI app in `web/` is older server packaging. Its sign-up form is on `/` and it accepts **POST** `/register`. This launcher does not need that app.

## Other files in this tree

`docker-compose.yml`, `web/`, and `docs/DOKPLOY.md` are the older Forever server packaging. They are not required to build the launcher, and the launcher does not read them.

The compose file publishes TCP **1119**, **8081**, and **8085**. `docker/entrypoint.sh` sets `LoginREST.ExternalAddress` and `LoginREST.LocalAddress` from `LOGIN_HOSTNAME`, which defaults to `<portal-host>`. It does not set `BattlenetPort` or `LoginREST.Port`. It points the certificate at `forever.cert.pem` / `forever.key.pem`. MySQL and SOAP stay off the public ports in that compose file; do not publish them.

Account creation on the worldserver console, if you use it:

```text
bnetaccount create mail@example.com change-me
```

Do not commit that password, and do not commit `.env`.

## Checklist

1. Generate a CA and a server certificate whose CN and SAN are the LoginREST hostname.
2. Install the chain and key where the running `bnetserver` loads them, and keep `CN=*.*` from being copied back.
3. Set `LoginREST.ExternalAddress` and `LoginREST.LocalAddress` to that hostname. Save the conf as UTF-8 without BOM.
4. Make the hostname resolve to `SET_SERVER_ADDRESS` on the server and on player PCs.
5. Rebuild the launcher with `FOREVER_PORTAL`, the three ports, `FOREVER_SERVER_ADDRESS`, and `FOREVER_CA_CERT` (or a replaced resource CA). Leave `FOREVER_WEBSITE` unset unless you want the site buttons. It does not gate the client build.
6. Players log in and press Play in the launcher. They do not start `Wow.exe` on its own.
