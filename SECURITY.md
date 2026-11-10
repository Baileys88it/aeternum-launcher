# Security policy

## Reporting a vulnerability

Please report security issues **privately** through GitHub's "Report a vulnerability" button (Security tab, private
vulnerability reporting) of this repository. Do not open a public issue for a vulnerability.

Include the affected version or commit, the platform, and steps to reproduce. You should get a first answer within
a few days.

## Scope

- the launcher (Rust code in `app/src-tauri`, UI in `app/ui`): login flow, TLS / CA handling, self-update download verification, hosts-file and certificate changes, the client patcher;
- the certificate scripts in `scripts/`.

Server software (bnetserver / worldserver) and the optional Docker / web packaging are best-effort.

## Never share

Private keys (`certs/ca/forever_root_ca.key`, `certs/forever.key.pem`), `.env` files, passwords, tokens, real server
addresses, or API keys. If you find one committed anywhere in this repository, report it privately.
