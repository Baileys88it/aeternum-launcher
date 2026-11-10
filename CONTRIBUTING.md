# Contributing

Thanks for helping. This repository is a **neutral launcher base**: it must work for any operator of a TrinityCore
Forever-family realm and must not identify a particular server.

## Workflow

1. Fork the repository and create a branch from `main`.
2. Keep each pull request to one topic. Describe what changed and how you tested it.
3. `main` is protected: changes land only through pull requests; direct pushes and force-pushes are blocked.

## Checks before a pull request

```sh
cd app/src-tauri
cargo fmt --check
cargo test --lib
```

UI texts exist in five languages (FR, EN, ES, PT, IT): keep `app/src-tauri/src/i18n.rs` and `app/ui/i18n.js` in sync.

## Neutrality guard

Do not commit anything that identifies a particular server: hostnames, realm names, IP addresses, local drive or
user-profile paths, personal names or e-mail addresses, API keys. Use placeholders such as `portal.example.invalid`.
Realm values are build-time variables (see the README) and must default to empty.

`contrib/neutrality/check_neutrality.py` enforces this in CI (`.github/workflows/neutrality.yml`) on every push to
`main` and every pull request. Enable the same check locally once per clone:

```sh
git config core.hooksPath .githooks
```

The `pre-commit` hook checks staged files and `pre-push` checks the whole tree. Files that must stay byte-identical
to an upstream source are listed in `.github/neutrality-allow.txt`.

## Third-party content

Do not add game client files, Blizzard artwork, or third-party addons to the repository. This tree ships only
AeternumQuestTag.

## License

By contributing you agree that your contribution is licensed under the GNU GPL v2.0 or later, like the rest of the
project (see `LICENSE`).
