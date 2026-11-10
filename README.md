<p align="center"><img src="docs/assets/aeternum-logo.png" alt="Aeternum" width="220"></p>

# Launcher base for TrinityCore Forever-family realms

**Server:** [Aeternum Engine](../../../aeternum-core) is the companion server engine for this launcher.

A [Tauri](https://tauri.app) 2 launcher (Rust and a small HTML/CSS/JS UI) that logs a player into a private Battle.net portal (`bnetserver`) and starts the World of Warcraft client with a login ticket. You build it once for your realm.

It is derived from the open-source Forever launcher. The window title still says Aeternum. The realm does not: host, ports, address, and website are empty until you set them at build time.

## What this is not

- No game client, no game data, and no link to download either.
- No baked realm. Nothing in the source defaults to a production hostname, to a public site, or to a fixed set of ports.
- No required website. Site buttons stay hidden unless you set one.
- No remote client-version gate. The launcher does not ask a website whether the installed client is too old or too recent. That check is outside this base: an operator who wants it adds it for their own site.

## Game cache

On every Play click the launcher empties the client's `Cache` folder (WDB/ADB) inside the game folder before it starts the game, so the client does not keep stale creature, quest, or item data from an earlier server state. Only that folder is touched; locked files are skipped and a failure never stops the launch. Turn it off in Settings: "Clear cache on launch".

## Quest tag addon

On every Play click the launcher installs or updates the client addon **AeternumQuestTag** in `Interface\AddOns\AeternumQuestTag` inside the game folder (only when its embedded version differs). The addon writes a yellow **QUEST** above the nameplate of the creatures your active quests still need: kill targets and creatures that drop an item an unfinished objective needs. `/aqt` toggles it in game (saved per character). It is on by default; turn it off in Settings: "Quest tag addon" (the launcher then removes the folder it installed). Other addons are never touched. The quest data comes from the world database: regenerate it with `scripts/gen-questtag-data.ps1` (see the script header) and rebuild the launcher. Addon tests: `addon/tests/run.lua` (Lua 5.1, from `addon/`).

## Third-party addons

This repository does not include third-party addons. The only addon in the tree is **AeternumQuestTag**, described above.

## Build-time configuration

Everything that identifies a realm is set when you build. Nothing has a production default.

| Variable | Default | Purpose |
|---|---|---|
| `AETERNUM_PORTAL` / `FOREVER_PORTAL` | empty | Login hostname (LoginREST, certificate CN/SAN) |
| `AETERNUM_SERVER_ADDRESS`, `AETERNUM_SERVER_LAN_ADDRESS` | empty | Where that hostname really is; encrypted into the exe (`build.rs`, `secret.rs`) |
| `AETERNUM_BNET_PORT`, `AETERNUM_REST_PORT`, `AETERNUM_WORLD_PORT` | 0 (unset) | Ports |
| `AETERNUM_SERVER_NAME` | empty | Realm name shown in the UI |
| `AETERNUM_WEBSITE` | empty | Site for news and site buttons; empty hides them |
| `AETERNUM_UPDATE_URL` | empty | HTTPS URL of the self-update manifest; empty disables the updater |
| `FOREVER_CA_CERT` | `app/src-tauri/resources/forever_root_ca.crt` (placeholder) | PEM text of your server CA |

The `FOREVER_*` names are accepted as aliases. See [docs/OPERATOR.md](docs/OPERATOR.md).

```sh
cd app/src-tauri
cargo check            # or: cargo test --lib
cargo tauri build      # release build (needs the Tauri CLI)
```

## Documentation

| | |
|---|---|
| Connect path, TLS, certificates, configuration, build | [docs/OPERATOR.md](docs/OPERATOR.md) (English) |
| The same guide in Italian | [docs/OPERATORE.md](docs/OPERATORE.md), [README.it.md](README.it.md) |
| Contributing, security | [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md) |
| macOS copy of the game | [docs/MACOS.md](docs/MACOS.md) |

Certificate scripts: [`scripts/gen-certs.sh`](scripts/gen-certs.sh) and [`scripts/generate-certs.bat`](scripts/generate-certs.bat). Both are described in the operator guide. Do not commit the private keys they write.

`docker-compose.yml`, `web/`, and [docs/DOKPLOY.md](docs/DOKPLOY.md) are older server packaging inherited with the tree. The launcher does not need them.

## License

GNU GPL v2.0 or later, the same license as TrinityCore and the Forever launcher this program is derived from. See [LICENSE](LICENSE).

Not affiliated with or endorsed by Blizzard Entertainment. World of Warcraft and Battle.net are trademarks of Blizzard Entertainment, Inc.
