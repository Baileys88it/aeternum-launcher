# Aeternum launcher fork notes

The window title, wordmark, config folder, and WTF name still say Aeternum. Login host, ports, server address, and website are not compiled in. Set them with the `FOREVER_*` variables at build time (see `docs/OPERATOR.md`). The updater stays off unless `AETERNUM_UPDATE_URL` is an https URL. The launcher does not call a website to decide if the installed client is too old or too recent.
