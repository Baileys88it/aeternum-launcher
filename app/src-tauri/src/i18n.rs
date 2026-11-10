//! Messages shown to the player (steps, errors, journal) in French, English, Spanish, Portuguese (Latin America) and Italian.
//! The UI sends the language it uses (`set_language`); English is the fallback.

use std::fmt::Display;
use std::sync::atomic::{AtomicUsize, Ordering};

static LANG: AtomicUsize = AtomicUsize::new(1);

/// fr, en, es, pt, it (anything else: en).
pub fn set(lang: &str) {
    let i = match lang.get(..2).unwrap_or("").to_ascii_lowercase().as_str() {
        "fr" => 0,
        "es" => 2,
        "pt" => 3,
        "it" => 4,
        _ => 1,
    };
    LANG.store(i, Ordering::Relaxed);
}

/// The message in the current language.
pub fn t(key: &str) -> &'static str {
    let texts = MESSAGES.iter().find(|(k, _)| *k == key).map(|(_, v)| v).unwrap_or(&[""; 5]);
    texts[LANG.load(Ordering::Relaxed)]
}

/// The message with each `{}` replaced by the next argument.
pub fn tf(key: &str, args: &[&dyn Display]) -> String {
    let mut out = String::new();
    let mut args = args.iter();
    let mut parts = t(key).split("{}").peekable();
    while let Some(part) = parts.next() {
        out.push_str(part);
        if parts.peek().is_some() {
            if let Some(a) = args.next() {
                out.push_str(&a.to_string());
            }
        }
    }
    out
}

type Texts = [&'static str; 5];

#[rustfmt::skip]
const MESSAGES: &[(&str, Texts)] = &[
    // launch steps
    ("step_connecting", ["Connexion au serveurâ€¦", "Connecting to the serverâ€¦", "Conectando con el servidorâ€¦", "Conectando ao servidorâ€¦", "Connessione al serverâ€¦"]),
    ("step_preparing", ["PrÃ©paration du jeuâ€¦", "Preparing the gameâ€¦", "Preparando el juegoâ€¦", "Preparando o jogoâ€¦", "Preparazione del giocoâ€¦"]),
    ("step_launching", ["Lancement de World of Warcraftâ€¦", "Starting World of Warcraftâ€¦", "Iniciando World of Warcraftâ€¦", "Iniciando World of Warcraftâ€¦", "Avvio di World of Warcraftâ€¦"]),
    ("running", ["Jeu lancÃ© : connectez-vous au royaume", "Game started: log in to the realm", "Juego iniciado: entra al reino", "Jogo iniciado: entre no reino", "Gioco avviato: entra nel reame"]),
    ("ready", ["ClÃ© du serveur en place : entrez dans le royaume", "Server key in place: enter the realm", "Clave del servidor lista: entra al reino", "Chave do servidor pronta: entre no reino", "Chiave del server pronta: entra nel reame"]),
    ("closed", ["Le jeu est fermÃ©", "The game is closed", "El juego se cerrÃ³", "O jogo foi fechado", "Il gioco Ã¨ stato chiuso"]),
    ("not_started", ["Le jeu n'a pas dÃ©marrÃ© (aucun Wow.exe trouvÃ© en 3 minutes)", "The game did not start (no Wow.exe found within 3 minutes)", "El juego no arrancÃ³ (no se encontrÃ³ Wow.exe en 3 minutos)", "O jogo nÃ£o iniciou (nenhum Wow.exe encontrado em 3 minutos)", "Il gioco non si Ã¨ avviato (nessun Wow.exe trovato in 3 minuti)"]),
    ("mac_copying", ["Copie du jeu pour Forever (premiÃ¨re fois ou aprÃ¨s une mise Ã  jour)â€¦", "Copying the game for Forever (first time or after an update)â€¦", "Copiando el juego para Forever (primera vez o tras una actualizaciÃ³n)â€¦", "Copiando o jogo para o Forever (primeira vez ou apÃ³s uma atualizaÃ§Ã£o)â€¦", "Copia del gioco per Forever (prima volta o dopo un aggiornamento)â€¦"]),
    ("mac_signing", ["Signature de la copie du jeuâ€¦", "Signing the game copyâ€¦", "Firmando la copia del juegoâ€¦", "Assinando a cÃ³pia do jogoâ€¦", "Firma della copia del giocoâ€¦"]),
    // launch errors
    ("already_running", ["le jeu est dÃ©jÃ  lancÃ©", "the game is already running", "el juego ya estÃ¡ abierto", "o jogo jÃ¡ estÃ¡ aberto", "il gioco Ã¨ giÃ  avviato"]),
    ("update_required", ["une mise Ã  jour du launcher est obligatoire avant de jouer", "a launcher update is required before playing", "es obligatorio actualizar el launcher antes de jugar", "Ã© obrigatÃ³rio atualizar o launcher antes de jogar", "Ã¨ obbligatorio aggiornare il launcher prima di giocare"]),
    ("game_already_running", ["le jeu est dÃ©jÃ  lancÃ©", "the game is already running", "el juego ya estÃ¡ abierto", "o jogo jÃ¡ estÃ¡ aberto", "il gioco Ã¨ giÃ  avviato"]),
    ("tool_blocked", ["programme non autorisÃ© dÃ©tectÃ© : {}, fermez-le pour jouer", "forbidden program detected: {}, close it to play", "programa no permitido detectado: {}, ciÃ©rralo para jugar", "programa nÃ£o permitido detectado: {}, feche-o para jogar", "programma non consentito rilevato: {}, chiudilo per giocare"]),
    ("game_not_found", ["jeu introuvable : choisissez le dossier _forever_ dans les paramÃ¨tres", "game not found: choose the _forever_ folder in the settings", "juego no encontrado: elige la carpeta _forever_ en los ajustes", "jogo nÃ£o encontrado: escolha a pasta _forever_ nas configuraÃ§Ãµes", "gioco non trovato: scegli la cartella _forever_ nelle impostazioni"]),
    ("login_first", ["connectez-vous d'abord", "log in first", "inicia sesiÃ³n primero", "entre na sua conta primeiro", "accedi prima"]),
    ("session_expired", ["session expirÃ©e : reconnectez-vous", "session expired: log in again", "sesiÃ³n caducada: vuelve a iniciar sesiÃ³n", "sessÃ£o expirada: entre novamente", "sessione scaduta: accedi di nuovo"]),
    ("session_save", ["enregistrement de la session : {}", "saving the session: {}", "guardando la sesiÃ³n: {}", "salvando a sessÃ£o: {}", "salvataggio della sessione: {}"]),
    ("launch_failed", ["lancement de {} : {}", "starting {}: {}", "iniciando {}: {}", "iniciando {}: {}", "avvio di {}: {}"]),
    ("wtf_folder", ["dossier WTF : {}", "WTF folder: {}", "carpeta WTF: {}", "pasta WTF: {}", "cartella WTF: {}"]),
    ("registry", ["registre : {}", "registry: {}", "registro: {}", "registro: {}", "registro: {}"]),
    ("mac_failed", ["{} a Ã©chouÃ© : {}", "{} failed: {}", "{} fallÃ³: {}", "{} falhou: {}", "{} non riuscito: {}"]),
    ("mac_not_started", ["le jeu n'a pas pu dÃ©marrer (open : {})", "the game could not start (open: {})", "el juego no pudo arrancar (open: {})", "o jogo nÃ£o pÃ´de iniciar (open: {})", "il gioco non Ã¨ riuscito ad avviarsi (open: {})"]),
    ("mac_module", ["le module Forever ne s'est pas chargÃ© dans le jeu", "the Forever module was not loaded into the game", "el mÃ³dulo Forever no se cargÃ³ en el juego", "o mÃ³dulo Forever nÃ£o foi carregado no jogo", "il modulo Forever non Ã¨ stato caricato nel gioco"]),
    ("mac_key", ["dÃ©rivation de la clÃ© du ticket impossible", "could not derive the ticket key", "no se pudo derivar la clave del ticket", "nÃ£o foi possÃ­vel derivar a chave do ticket", "impossibile derivare la chiave del ticket"]),
    ("mac_encrypt", ["chiffrement du ticket impossible ({})", "could not encrypt the ticket ({})", "no se pudo cifrar el ticket ({})", "nÃ£o foi possÃ­vel criptografar o ticket ({})", "impossibile cifrare il ticket ({})"]),
    ("mac_user", ["nom d'utilisateur inconnu", "unknown user name", "nombre de usuario desconocido", "nome de usuÃ¡rio desconhecido", "nome utente sconosciuto"]),
    // server API
    ("address_not_found", ["serveur introuvable", "server not found", "servidor no encontrado", "servidor nÃ£o encontrado", "server non trovato"]),
    ("unexpected_answer", ["rÃ©ponse inattendue du serveur", "unexpected answer from the server", "respuesta inesperada del servidor", "resposta inesperada do servidor", "risposta inattesa dal server"]),
    ("authenticator", ["ce compte utilise un authentificateur, non gÃ©rÃ© par le launcher", "this account uses an authenticator, which the launcher does not support", "esta cuenta usa un autenticador, que el launcher no admite", "esta conta usa um autenticador, que o launcher nÃ£o suporta", "questo account usa un autenticatore, non supportato dal launcher"]),
    ("bad_login", ["e-mail ou mot de passe incorrect", "wrong e-mail or password", "correo o contraseÃ±a incorrectos", "e-mail ou senha incorretos", "e-mail o password errati"]),
    ("remember_refused", ["le serveur refuse la connexion automatique (HTTP {})", "the server refuses automatic login (HTTP {})", "el servidor rechaza el inicio de sesiÃ³n automÃ¡tico (HTTP {})", "o servidor recusa o login automÃ¡tico (HTTP {})", "il server rifiuta l'accesso automatico (HTTP {})"]),
    ("banned", ["compte banni", "account banned", "cuenta suspendida", "conta banida", "account bannato"]),
    ("server_error", ["erreur du serveur (HTTP {})", "server error (HTTP {})", "error del servidor (HTTP {})", "erro do servidor (HTTP {})", "errore del server (HTTP {})"]),
    ("unreachable", ["serveur injoignable", "server unreachable", "servidor inaccesible", "servidor inacessÃ­vel", "server non raggiungibile"]),
    ("network_error", ["erreur rÃ©seau : {}", "network error: {}", "error de red: {}", "erro de rede: {}", "errore di rete: {}"]),
    ("no_news", ["pas de nouveautÃ©s (HTTP {})", "no news (HTTP {})", "sin novedades (HTTP {})", "sem novidades (HTTP {})", "nessuna novitÃ  (HTTP {})"]),
    ("unexpected_site", ["rÃ©ponse inattendue du site", "unexpected answer from the website", "respuesta inesperada del sitio", "resposta inesperada do site", "risposta inattesa dal sito"]),
    // PC checks and setup
    ("wine_system", ["Wine (systÃ¨me)", "Wine (system)", "Wine (sistema)", "Wine (sistema)", "Wine (sistema)"]),
    ("no_resolve", ["le serveur ne se rÃ©sout pas", "the server does not resolve", "el servidor no se resuelve", "o servidor nÃ£o Ã© resolvido", "il nome del server non viene risolto"]),
    ("hosts_wrong", ["{} : mauvaise adresse (cliquez sur RÃ©parer)", "{}: wrong address (click Fix)", "{}: direcciÃ³n incorrecta (pulse Reparar)", "{}: endereÃ§o incorreto (clique em Reparar)", "{}: indirizzo errato (clicca su Ripara)"]),
    ("hosts_missing", ["{} n'est pas dans le fichier hosts", "{} is not in the hosts file", "{} no estÃ¡ en el archivo hosts", "{} nÃ£o estÃ¡ no arquivo hosts", "{} non Ã¨ nel file hosts"]),
    ("cert_store_unreadable", ["magasin de certificats illisible", "certificate store unreadable", "almacÃ©n de certificados ilegible", "repositÃ³rio de certificados ilegÃ­vel", "archivio certificati illeggibile"]),
    ("cert_ok_windows", ["certificat du serveur reconnu par Windows", "server certificate trusted by Windows", "certificado del servidor reconocido por Windows", "certificado do servidor reconhecido pelo Windows", "certificato del server riconosciuto da Windows"]),
    ("cert_missing", ["certificat du serveur non installÃ©", "server certificate not installed", "certificado del servidor no instalado", "certificado do servidor nÃ£o instalado", "certificato del server non installato"]),
    ("cert_ok_macos", ["certificat du serveur approuvÃ© par macOS", "server certificate trusted by macOS", "certificado del servidor aprobado por macOS", "certificado do servidor aprovado pelo macOS", "certificato del server approvato da macOS"]),
    ("cert_not_trusted", ["certificat du serveur non approuvÃ©", "server certificate not trusted", "certificado del servidor no aprobado", "certificado do servidor nÃ£o aprovado", "certificato del server non approvato"]),
    ("cert_ok_prefix", ["certificat installÃ© dans le prÃ©fixe Wine", "certificate installed in the Wine prefix", "certificado instalado en el prefijo de Wine", "certificado instalado no prefixo do Wine", "certificato installato nel prefisso Wine"]),
    ("prefix_missing", ["prÃ©fixe Wine pas encore crÃ©Ã©", "Wine prefix not created yet", "prefijo de Wine aÃºn no creado", "prefixo do Wine ainda nÃ£o criado", "prefisso Wine non ancora creato"]),
    ("cert_not_in_prefix", ["certificat absent du prÃ©fixe Wine", "certificate missing from the Wine prefix", "certificado ausente del prefijo de Wine", "certificado ausente do prefixo do Wine", "certificato assente dal prefisso Wine"]),
    ("not_configured", ["serveur non configuré : définissez le portail, les ports et l'adresse à la compilation", "server not configured: set the portal, ports, and address at build time", "servidor no configurado: define el portal, los puertos y la dirección al compilar", "servidor não configurado: defina o portal, as portas e o endereço na compilação", "server non configurato: imposta portale, porte e indirizzo in compilazione"]),
    ("bad_portal", ["portail invalide", "invalid portal", "portal no vÃ¡lido", "portal invÃ¡lido", "portale non valido"]),
    ("admin_refused", ["autorisation administrateur refusÃ©e", "administrator permission refused", "permiso de administrador denegado", "permissÃ£o de administrador recusada", "autorizzazione di amministratore negata"]),
    ("install_wait", ["attente de l'installation impossible", "could not wait for the installation", "no se pudo esperar a la instalaciÃ³n", "nÃ£o foi possÃ­vel aguardar a instalaÃ§Ã£o", "impossibile attendere l'installazione"]),
    ("install_failed", ["Ã©chec de l'installation (code {})", "installation failed (code {})", "la instalaciÃ³n fallÃ³ (cÃ³digo {})", "a instalaÃ§Ã£o falhou (cÃ³digo {})", "installazione non riuscita (codice {})"]),
    ("hosts_prompt", ["Aeternum launcher : ajout du serveur dans le fichier hosts.", "Aeternum launcher: adding the server to the hosts file.", "Aeternum launcher: aÃ±adiendo el servidor al archivo hosts.", "Aeternum launcher: adicionando o servidor ao arquivo hosts.", "Aeternum launcher: aggiunta del server al file hosts."]),
    ("hosts_refused", ["autorisation refusÃ©e pour le fichier hosts", "permission refused for the hosts file", "permiso denegado para el archivo hosts", "permissÃ£o recusada para o arquivo hosts", "autorizzazione negata per il file hosts"]),
    ("hosts_write_failed", ["Ã©criture du fichier hosts impossible : {}", "could not write the hosts file: {}", "no se pudo escribir el archivo hosts: {}", "nÃ£o foi possÃ­vel gravar o arquivo hosts: {}", "impossibile scrivere il file hosts: {}"]),
    ("hosts_write_failed_pkexec", ["Ã©criture du fichier hosts impossible (pkexec)", "could not write the hosts file (pkexec)", "no se pudo escribir el archivo hosts (pkexec)", "nÃ£o foi possÃ­vel gravar o arquivo hosts (pkexec)", "impossibile scrivere il file hosts (pkexec)"]),
    ("cert_trust_failed", ["certificat non approuvÃ© : {}", "certificate not trusted: {}", "certificado no aprobado: {}", "certificado nÃ£o aprovado: {}", "certificato non approvato: {}"]),
    ("prefix_error", ["prÃ©fixe {} : {}", "prefix {}: {}", "prefijo {}: {}", "prefixo {}: {}", "prefisso {}: {}"]),
    ("wine_not_found", ["wine est introuvable", "wine not found", "no se encontrÃ³ wine", "wine nÃ£o encontrado", "wine non trovato"]),
    ("umu_not_found", ["umu-run est introuvable (paquet umu-launcher)", "umu-run not found (umu-launcher package)", "no se encontrÃ³ umu-run (paquete umu-launcher)", "umu-run nÃ£o encontrado (pacote umu-launcher)", "umu-run non trovato (pacchetto umu-launcher)"]),
    ("proton_not_found", ["Proton introuvable : {}", "Proton not found: {}", "Proton no encontrado: {}", "Proton nÃ£o encontrado: {}", "Proton non trovato: {}"]),
    ("command_failed", ["{} a Ã©chouÃ© : {}", "{} failed: {}", "{} fallÃ³: {}", "{} falhou: {}", "{} non riuscito: {}"]),
    // key patcher (journal)
    ("bad_public_key", ["clÃ© publique du serveur invalide", "invalid server public key", "clave pÃºblica del servidor no vÃ¡lida", "chave pÃºblica do servidor invÃ¡lida", "chiave pubblica del server non valida"]),
    ("bad_region_group", ["groupe de rÃ©gion invalide (2-12, sauf 9)", "invalid region group (2-12, except 9)", "grupo de regiÃ³n no vÃ¡lido (2-12, excepto 9)", "grupo de regiÃ£o invÃ¡lido (2-12, exceto 9)", "gruppo di regione non valido (2-12, tranne 9)"]),
    ("write_refused", ["Ã©criture refusÃ©e", "write refused", "escritura rechazada", "gravaÃ§Ã£o recusada", "scrittura rifiutata"]),
    ("verify_failed", ["vÃ©rification Ã©chouÃ©e", "check failed", "verificaciÃ³n fallida", "verificaÃ§Ã£o falhou", "verifica non riuscita"]),
    ("memory_changed", ["la mÃ©moire a changÃ© pendant l'Ã©criture", "memory changed while writing", "la memoria cambiÃ³ durante la escritura", "a memÃ³ria mudou durante a gravaÃ§Ã£o", "la memoria Ã¨ cambiata durante la scrittura"]),
    ("key_restored", ["clÃ© du serveur remise en place", "server key put back", "clave del servidor restablecida", "chave do servidor restaurada", "chiave del server ripristinata"]),
    ("key_write_error", ["Ã©criture de la clÃ© : {}", "writing the key: {}", "escribiendo la clave: {}", "gravando a chave: {}", "scrittura della chiave: {}"]),
    ("key_store_freed", ["magasin de clÃ©s libÃ©rÃ©, nouvelle recherche Ã  la prochaine connexion", "key store freed, searching again at the next login", "almacÃ©n de claves liberado, nueva bÃºsqueda en el prÃ³ximo inicio de sesiÃ³n", "repositÃ³rio de chaves liberado, nova busca no prÃ³ximo login", "archivio chiavi liberato, nuova ricerca al prossimo accesso"]),
    ("game_process", ["processus du jeu : PID {}", "game process: PID {}", "proceso del juego: PID {}", "processo do jogo: PID {}", "processo del gioco: PID {}"]),
    ("key_written", ["clÃ© du serveur Ã©crite (groupe {})", "server key written (group {})", "clave del servidor escrita (grupo {})", "chave do servidor gravada (grupo {})", "chiave del server scritta (gruppo {})"]),
    ("key_write_retry", ["Ã©criture de la clÃ© : {}, nouvel essai", "writing the key: {}, retrying", "escribiendo la clave: {}, reintentando", "gravando a chave: {}, tentando de novo", "scrittura della chiave: {}, nuovo tentativo"]),
    ("waiting_realm", ["en attente de l'entrÃ©e dans le royaumeâ€¦", "waiting for the realm loginâ€¦", "esperando la entrada al reinoâ€¦", "aguardando a entrada no reinoâ€¦", "in attesa dell'ingresso nel reameâ€¦"]),
    ("many_key_stores", ["{} magasins de clÃ©s Ã  la fois : attente", "{} key stores at once: waiting", "{} almacenes de claves a la vez: esperando", "{} repositÃ³rios de chaves ao mesmo tempo: aguardando", "{} archivi di chiavi contemporaneamente: attesa"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_has_five_texts_with_the_same_placeholders() {
        for (key, texts) in MESSAGES {
            let n = texts[0].matches("{}").count();
            for text in texts {
                assert!(!text.is_empty(), "{key}");
                assert_eq!(text.matches("{}").count(), n, "{key}: {text}");
            }
        }
    }

    #[test]
    fn formats_in_the_chosen_language() {
        set("es-419");
        assert_eq!(tf("server_error", &[&503]), "error del servidor (HTTP 503)");
        set("pt-BR");
        assert_eq!(t("banned"), "conta banida");
        set("it-IT");
        assert_eq!(t("banned"), "account bannato");
        set("de");
        assert_eq!(t("banned"), "account banned");
        set("fr");
    }
}
