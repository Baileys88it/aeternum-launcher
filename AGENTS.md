# Instructions for AI coding assistants

This repository is a **neutral launcher base** for operators of TrinityCore Forever-family realms. It is derived from the open-source Forever launcher. Read `README.md` and `docs/OPERATOR.md` first.

The launcher does not ship a realm. `FOREVER_PORTAL`, `FOREVER_BNET_PORT`, `FOREVER_REST_PORT`, `FOREVER_WORLD_PORT`, `FOREVER_SERVER_ADDRESS`, and `FOREVER_WEBSITE` are optional build-time hooks and default to unset. Do not put back a production hostname, a public site, or fixed ports as compiled defaults. An empty website must hide every site button and must not fall back to a URL. Do not query a website to decide whether the client is too old or too recent. Remote version gating against a website API is outside this base.

## Layout

| Path | What it is |
|---|---|
| `app/` | Launcher (Tauri 2: Rust in `app/src-tauri/src`, UI in `app/ui`). Languages: FR, EN, ES, PT, IT (`i18n.rs`, `ui/i18n.js`). |
| `scripts/gen-certs.sh`, `scripts/generate-certs.bat` | Private CA (name-constrained to `*.<portal-domain>`) and a server certificate. Never commit the `.key` or `*.pem`. |
| `docs/OPERATOR.md`, `docs/OPERATORE.md` | How to connect the launcher to a Forever-family bnetserver (HTTPS LoginREST, certs, ports, hosts). |
| `Dockerfile`, `docker/`, `docker-compose.yml`, `web/` | Older Forever server packaging. Not required to build the launcher. `docs/DOKPLOY.md` is that packaging's deploy note. |
| `docs/MACOS.md` | How the macOS launcher copies and re-signs the game. |

## Rules

- **Never** commit, upload, or paste `certs/ca/forever_root_ca.key`, `certs/forever.key.pem`, `.env`, passwords, tokens, `DatabaseInfo` lines, or real public/LAN addresses. `forever_root_ca.crt` is public and is what the launcher embeds.
- Do not add Blizzard files: no game client, game data, artwork, logos, or download links to the client.
- Do not suggest ways to obtain the game client.
- The stock Trinity dev certificate `CN=*.*` makes bnetserver serve LoginREST as HTTP. This launcher speaks HTTPS only. Do not document that wildcard as a working setup.
- Keep the five UI languages in sync when changing texts (FR, EN, ES, PT, IT). `cargo test --lib` checks the Rust messages.
- If you touch the optional Docker worldserver import, do not restart worldserver during its first database import.

## Checks

```sh
cd app/src-tauri && cargo test --lib
```

The resource CA `app/src-tauri/resources/forever_root_ca.crt` is enough for that test. `FOREVER_CA_CERT` replaces it at build time when an operator has generated their own CA.
