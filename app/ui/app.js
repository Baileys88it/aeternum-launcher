// Aeternum launcher UI. Everything privileged happens in Rust (src-tauri); this file only calls its commands.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

let state = null;      // snapshot from get_state
let client = null;     // client_status: { state: missing|ok, ... } local install only
let bnetStarted = false;
let draft = null;      // settings being edited in the drawer
let toastTimer = 0;
let launcherUpdate = null;  // mandatory launcher update from the manifest (Play stays off until installed)
let guard = { tool: null, gameRunning: false };  // watchdog: forbidden tool / Forever game already running
let blockerText = '';

function toast(text) {
  const t = $('toast');
  t.textContent = text;
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (t.hidden = true), 4500);
}

function log(line) {
  const pre = $('log');
  const time = new Date().toLocaleTimeString(lang);
  pre.textContent += `[${time}] ${line}\n`;
  pre.scrollTop = pre.scrollHeight;
}

function setStep(text, kind = '') {
  const s = $('step');
  s.textContent = text;
  s.className = 'step' + (kind ? ' ' + kind : '');
}

// ------------------------------------------------------------------------------------------------ state
// '' follows the system language; Rust gets the same language for its messages
function setLanguage(setting) {
  const code = resolveLanguage(setting);
  if (code === lang && document.documentElement.lang === code) return;
  applyLanguage(code);
  invoke('set_language', { language: code });
}

async function refresh() {
  state = await invoke('get_state');
  setLanguage(state.settings.language);
  const realmName = (state.settings.server.name || '').trim();
  $('realm-field').textContent = realmName ? t('realm', { name: realmName }) : t('realmUnconfigured');
  $('version').textContent = `v${state.version} · ${{ windows: 'Windows', macos: 'macOS', linux: 'Linux' }[state.platform]}`;
  const website = siteUrl();
  const signup = $('signup');
  if (signup) signup.hidden = !website;
  const siteCta = $('site-cta');
  if (siteCta) siteCta.hidden = !website;
  const logged = !!state.email;
  $('view-login').hidden = logged;
  $('view-play').hidden = !logged;
  $('account').hidden = !logged;
  const crest = document.querySelector('.home-crest');
  if (crest) {
    crest.hidden = !logged;
    crest.setAttribute('aria-hidden', logged ? 'false' : 'true');
  }
  $('game-field').textContent = (client?.installedVersion || state.gameVersion || '').trim();
  if (logged) {
    $('account-email').textContent = state.email;
    $('avatar').textContent = state.email[0].toUpperCase();
  }
  updatePlayButton();
}

function updatePlayButton() {
  const play = $('play');
  // Play needs a local install. A website is not asked whether the build is too old or too recent.
  // Also off for a mandatory launcher update, a forbidden tool, or a game already started outside this launcher.
  const blocker = launcherUpdate ? t('updateRequired')
    : guard.tool ? t('toolBlocked', { name: guard.tool })
    : guard.gameRunning && !state.playing ? t('gameRunning') : '';
  play.disabled = state.playing || !state.gameFound || client?.state !== 'ok' || !!blocker;
  play.classList.toggle('running', state.playing);
  play.textContent = state.playing ? t('inGame') : t('play');
  if (blocker) {
    if ($('step').textContent !== blocker) setStep(blocker, 'err');
    blockerText = blocker;
  } else if (blockerText) {
    if ($('step').textContent === blockerText) setStep('');
    blockerText = '';
  }
  if (!state.gameFound && !state.playing) setStep(t('chooseGameDir'), 'err');
}

async function refreshGuard() {
  try {
    const g = await invoke('game_running');
    const changed = g.tool !== guard.tool || g.gameRunning !== guard.gameRunning;
    guard = g;
    if (changed && state) updatePlayButton();
  } catch {}
}

// ------------------------------------------------------------------------------------------------ local client folder
// missing: 'Download client' · ok: Play. No remote too-old / too-recent gate.
let clientTimer = 0;

async function refreshClient() {
  try {
    client = await invoke('client_status');
  } catch (e) {
    client = { state: state?.gameFound ? 'ok' : 'missing', command: '' };
  }
  if (client.state === 'ok') bnetStarted = false;
  if (client.installedVersion) state.gameVersion = client.installedVersion;
  // the folder may have been detected after a Battle.net install
  if (client.state !== 'missing' && !state.gameFound) await refresh();
  showClient();
  clearTimeout(clientTimer);
  // re-check on its own: often while something is wrong, rarely when all is fine
  clientTimer = setTimeout(refreshClient, client.state === 'ok' ? 300000 : bnetStarted ? 15000 : 30000);
}

function showClient() {
  const bar = $('client-bar');
  const st = client?.state || 'checking';
  bar.dataset.state = st;
  const action = $('client-action');
  let title = '';
  let text = '';
  action.hidden = true;
  $('client-folder').hidden = true;
  if (st === 'checking') {
    title = t('clientChecking');
  } else if (st === 'missing') {
    title = t('clientMissingTitle');
    text = bnetStarted ? t('bnetStarted') : t('clientMissing');
    action.hidden = false;
    action.textContent = t('clientDownload');
    $('client-folder').hidden = false;
  }
  $('client-title').textContent = title;
  $('client-text').textContent = text;
  bar.hidden = st === 'ok';
  if (state) {
    $('game-field').textContent = (client?.installedVersion || state.gameVersion || '').trim();
    updatePlayButton();
  }
}

async function installClient() {
  try {
    const r = await invoke('install_client');
    if (r === 'started') {
      bnetStarted = true;
      showClient();
      toast(t('bnetStarted'));
      clearTimeout(clientTimer);
      clientTimer = setTimeout(refreshClient, 15000);
      return;
    }
    // Battle.net not installed: instructions + the official download page
    showModal({
      title: t('bnetMissingTitle'),
      text: t('bnetMissing'),
      code: client?.command || 'Battle.net.exe --game=wowf --install',
      actions: [
        { label: t('ok'), cls: 'ghost' },
        { label: t('bnetOpenDownload'), cls: 'primary', run: () => invoke('battlenet_download').catch((e) => toast(String(e))) },
      ],
    });
  } catch (e) {
    toast(t('failed', { e }));
  }
}

$('client-action').addEventListener('click', installClient);
$('client-recheck').addEventListener('click', async (ev) => {
  const btn = ev.currentTarget;
  btn.disabled = true;
  client = null;
  showClient();
  await refreshClient();
  btn.disabled = false;
});
$('client-folder').addEventListener('click', openSettings);

// ------------------------------------------------------------------------------------------------ dialogs
// locked: no Escape (mandatory choices)
function showModal({ title, text, code = '', actions = [{ label: t('ok'), cls: 'primary' }], locked = false }) {
  $('modal').dataset.locked = locked ? '1' : '';
  $('modal-title').textContent = title;
  $('modal-text').textContent = text;
  $('modal-code').hidden = !code;
  $('modal-code').textContent = code;
  const box = $('modal-actions');
  box.replaceChildren();
  for (const a of actions) {
    const b = document.createElement('button');
    b.className = 'btn ' + (a.cls || '');
    b.textContent = a.label;
    b.addEventListener('click', () => {
      if (!a.keep) $('modal').hidden = true;
      a.run?.();
      // another dialog covered the mandatory update one: bring it back
      if (!a.keep && !a.run && !locked && launcherUpdate && $('updating').hidden) askUpdate(launcherUpdate);
    });
    box.append(b);
  }
  $('modal').hidden = false;
  box.lastChild?.focus();
}

// ------------------------------------------------------------------------------------------------ launcher self-update
async function checkLauncherUpdate() {
  let info;
  try {
    info = await invoke('update_check');
  } catch {
    return;   // no manifest / offline: keep going with this version
  }
  const m = info.available;
  launcherUpdate = m || null;
  if (state) updatePlayButton();
  const [kind, text] = info.note || [];
  if (kind === 'updated') toast(t('updateDone', { version: text }));
  if (kind === 'error') {
    // the failed update was rolled back: Play stays blocked while the manifest still offers it
    if (m) return updateBlocked(text);
    return showModal({ title: t('updateFailedTitle'), text: t('updateRolledBack', { e: text }) });
  }
  if (m) askUpdate(m);
}

// mandatory update: 'Update' or close the launcher, nothing else
function askUpdate(m) {
  showModal({
    title: t('updateTitle'),
    text: t('updateMandatory', { version: m.version }),
    locked: true,
    actions: [
      { label: t('quitLauncher'), cls: 'ghost', keep: true, run: () => invoke('quit_launcher') },
      { label: t('updateButton'), cls: 'primary', run: () => runLauncherUpdate(m) },
    ],
  });
}

function updateBlocked(e) {
  showModal({
    title: t('updateFailedTitle'),
    text: t('updateFailedBlocked', { e }),
    locked: true,
    actions: [
      { label: t('quitLauncher'), cls: 'ghost', keep: true, run: () => invoke('quit_launcher') },
      { label: t('retry'), cls: 'primary', run: () => runLauncherUpdate(launcherUpdate) },
    ],
  });
}

function updateScreen(stage, done = 0, total = 0) {
  const label = { download: 'updateDownload', verify: 'updateVerify', extract: 'updateVerify', install: 'updateInstall' }[stage];
  $('update-status').textContent = t(label || 'updateDownload');
  const bar = $('update-bar');
  const prog = bar.parentElement;
  // download: 0-80 %, verify/extract: 80-95 %, install: 100 %
  let pct = 0;
  if (stage === 'download') pct = total ? (done / total) * 80 : 0;
  else if (stage === 'verify' || stage === 'extract') pct = 80 + (total ? (done / total) * 15 : 0);
  else if (stage === 'install') pct = 100;
  prog.classList.toggle('indeterminate', stage === 'download' && !total);
  bar.style.width = `${Math.round(pct)}%`;
}

async function runLauncherUpdate(m) {
  $('update-version').textContent = m.version;
  updateScreen('download');
  $('updating').hidden = false;
  try {
    await invoke('update_install');
    updateScreen('install');
    // the launcher closes itself; the new version reopens on its own
  } catch (e) {
    $('updating').hidden = true;
    updateBlocked(e);
  }
}

listen('update', ({ payload }) => updateScreen(payload.stage, payload.done, payload.total));

async function refreshStatus() {
  const pill = $('status');
  try {
    const s = await invoke('server_status');
    pill.dataset.state = s.online && s.login ? 'online' : s.online || s.login ? 'partial' : 'offline';
    $('status-text').textContent = t(s.online && s.login ? 'online' : s.online || s.login ? 'starting' : 'offline');
  } catch {
    pill.dataset.state = 'offline';
    $('status-text').textContent = t('unreachable');
  }
}

async function refreshChecks() {
  try {
    const c = await invoke('checks');
    showCheck('check-hosts', c.hosts, c.hostsDetail);
    showCheck('check-cert', c.certificate, c.certificateDetail);
    // Banner disabled: hosts/CA can still be fixed from Settings.
    $('setup-notice').hidden = true;
    $('setup-detail').textContent = '';
    return c;
  } catch (e) {
    log(t('checks', { e }));
  }
}

function showCheck(id, ok, detail) {
  const el = $(id);
  el.dataset.ok = String(ok);
  el.querySelector('p').textContent = detail;
}

async function repair(btn) {
  btn.disabled = true;
  const label = btn.textContent;
  btn.textContent = t('working');
  try {
    const c = await invoke('repair');
    showCheck('check-hosts', c.hosts, c.hostsDetail);
    showCheck('check-cert', c.certificate, c.certificateDetail);
    toast(c.hosts ? t('pcReady') : t('partialRepair'));
  } catch (e) {
    toast(t('failed', { e }));
  } finally {
    btn.disabled = false;
    btn.textContent = label;
    refreshChecks();
  }
}

// ------------------------------------------------------------------------------------------------ login
$('login-form').addEventListener('submit', async (ev) => {
  ev.preventDefault();
  const btn = $('login-btn');
  const err = $('login-error');
  err.hidden = true;
  btn.disabled = true;
  btn.textContent = t('loggingIn');
  try {
    await invoke('login', { email: $('email').value, password: $('password').value });
    $('password').value = '';
    await refresh();
    refreshChecks();
  } catch (e) {
    err.textContent = String(e);
    err.hidden = false;
  } finally {
    btn.disabled = false;
    btn.textContent = t('loginButton');
  }
});

$('logout').addEventListener('click', async () => {
  await invoke('logout');
  await refresh();
});

$('signup-link').addEventListener('click', (ev) => {
  ev.preventDefault();
  openSite('/register');
});

// ------------------------------------------------------------------------------------------------ play
$('play').addEventListener('click', async () => {
  $('log').textContent = '';
  state.playing = true;
  updatePlayButton();
  setStep(t('preparing'));
  try {
    await invoke('play');
  } catch (e) {
    state.playing = false;
    updatePlayButton();
    setStep(String(e), 'err');
    log(t('error', { e }));
    refresh();   // an expired session sends back to the login view
  }
});

$('setup-fix').addEventListener('click', (ev) => repair(ev.currentTarget));
$('toggle-log').addEventListener('click', (ev) => {
  ev.preventDefault();
  $('log-box').hidden = !$('log-box').hidden;
});
$('close-log').addEventListener('click', () => ($('log-box').hidden = true));
$('repair').addEventListener('click', (ev) => repair(ev.currentTarget));

listen('game', ({ payload }) => {
  const { kind, message } = payload;
  if (kind === 'log') return log(message);
  if (kind === 'blocked' || kind === 'unblocked') {
    if (message) log(message);
    return refreshGuard();
  }
  log(message);
  if (kind === 'step' || kind === 'running') setStep(message);
  if (kind === 'ready') setStep(message, 'ok');
  if (kind === 'error') setStep(message, 'err');
  if (kind === 'closed' || kind === 'error') {
    state.playing = false;
    updatePlayButton();
    if (kind === 'closed') setStep('');
  }
});

// ------------------------------------------------------------------------------------------------ settings
function option(select, value, label) {
  const o = document.createElement('option');
  o.value = value;
  o.textContent = label;
  select.append(o);
}

function fillSettings() {
  draft = structuredClone(state.settings);
  $('s-game-dir').value = draft.gameDir;
  $('s-bnet').value = draft.battlenetPath || '';
  $('s-clear-cache').checked = draft.clearCacheOnLaunch !== false;
  $('s-quest-tag').checked = draft.questTagAddon !== false;
  $('bnet-field').hidden = state.platform !== 'windows';
  const loc = $('s-locale');
  loc.replaceChildren();
  state.locales.forEach((l) => option(loc, l, l));
  loc.value = draft.locale;
  $('linux-settings').hidden = state.platform !== 'linux';
  if (state.platform === 'linux') {
    const r = $('s-runner');
    r.replaceChildren();
    state.runners.forEach(([name, path]) => option(r, path, name));
    if (![...r.options].some((o) => o.value === draft.linux.runner) && draft.linux.runner) option(r, draft.linux.runner, draft.linux.runner);
    r.value = draft.linux.runner;
    $('s-prefix').value = draft.linux.prefix;
    $('s-gamescope').checked = draft.linux.gamescope;
    $('s-gamescope-args').value = draft.linux.gamescopeArgs;
    $('s-keyboard').value = draft.linux.keyboard;
  }
  const language = $('s-language');
  language.replaceChildren();
  option(language, '', t('systemLanguage'));
  Object.entries(LANGUAGES).forEach(([code, name]) => option(language, code, name));
  language.value = draft.language || '';
  gameDirState();
}

function gameDirState() {
  const el = $('game-dir-state');
  const dir = $('s-game-dir').value.trim();
  const same = dir === state.settings.gameDir;
  const game = state.platform === 'macos' ? t('gameApp') : 'Wow.exe';
  el.textContent = !dir ? t('noFolder') : same && state.gameFound ? t('gameFound', { game }) : same ? t('gameMissing', { game }) : '';
  el.className = 'small ' + (same && state.gameFound ? 'ok' : dir && same ? 'ko' : '');
}

function openSettings() {
  fillSettings();
  $('settings').hidden = false;
  $('scrim').hidden = false;
  refreshChecks();
}

function closeSettings() {
  $('settings').hidden = true;
  $('scrim').hidden = true;
}

async function pick(input, title) {
  const dir = await window.__TAURI__.dialog.open({ directory: true, title, defaultPath: input.value || undefined });
  if (dir) {
    input.value = dir;
    if (input.id === 's-game-dir') {
      const loc = await invoke('detect_locale', { gameDir: dir });
      if (loc) $('s-locale').value = loc;
    }
  }
}

$('open-settings').addEventListener('click', openSettings);
$('play-settings').addEventListener('click', openSettings);
$('close-settings').addEventListener('click', closeSettings);
$('cancel-settings').addEventListener('click', closeSettings);
$('scrim').addEventListener('click', closeSettings);
$('pick-game').addEventListener('click', () => pick($('s-game-dir'), t('pickGameDir')));
$('pick-prefix').addEventListener('click', () => pick($('s-prefix'), t('pickPrefix')));
$('pick-bnet').addEventListener('click', async () => {
  const f = await window.__TAURI__.dialog.open({ directory: false, title: 'Battle.net.exe', filters: [{ name: 'Battle.net', extensions: ['exe'] }] });
  if (f) $('s-bnet').value = f;
});
$('s-game-dir').addEventListener('input', gameDirState);
document.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape') return;
  if (!$('modal').hidden) {
    if (!$('modal').dataset.locked) {
      $('modal').hidden = true;
      if (launcherUpdate && $('updating').hidden) askUpdate(launcherUpdate);
    }
  }
  else if (!$('settings').hidden) closeSettings();
});

$('save-settings').addEventListener('click', async () => {
  draft.gameDir = $('s-game-dir').value.trim();
  draft.battlenetPath = $('s-bnet').value.trim();
  draft.clearCacheOnLaunch = $('s-clear-cache').checked;
  draft.questTagAddon = $('s-quest-tag').checked;
  draft.locale = $('s-locale').value;
  draft.language = $('s-language').value;
  if (state.platform === 'linux') {
    draft.linux.runner = $('s-runner').value;
    draft.linux.prefix = $('s-prefix').value.trim();
    draft.linux.gamescope = $('s-gamescope').checked;
    draft.linux.gamescopeArgs = $('s-gamescope-args').value.trim();
    draft.linux.keyboard = $('s-keyboard').value.trim();
  }
  try {
    await invoke('save_settings', { settings: draft });
    closeSettings();
    await refresh();
    refreshStatus();
    refreshChecks();
    toast(t('saved'));
    refreshClient();
  } catch (e) {
    toast(t('saveFailed', { e }));
  }
});

// ------------------------------------------------------------------------------------------------ website
// No public fallback. An empty compiled website hides every site button.
function siteUrl() {
  return (state?.settings?.server?.website || '').trim();
}

async function openLink(url) {
  if (!url) return;
  try {
    const open = window.__TAURI__?.opener?.openUrl;
    if (open) await open(url);
    else window.open(url, '_blank', 'noopener');
  } catch (e) {
    console.error('openLink', e);
    toast(String(e));
  }
}

function openSite(path = '') {
  const base = siteUrl().replace(/\/$/, '');
  if (!base) return;
  const suffix = path ? (path.startsWith('/') ? path : `/${path}`) : '';
  openLink(`${base}${suffix}`);
}

$('open-website')?.addEventListener('click', () => openSite());

// ------------------------------------------------------------------------------------------------ storm particles (Myrkgard-like blue ambience)
function startStormFx() {
  const canvas = $('storm-fx');
  if (!canvas || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  let w = 0;
  let h = 0;
  const flakes = [];
  const resize = () => {
    w = canvas.width = canvas.clientWidth * devicePixelRatio;
    h = canvas.height = canvas.clientHeight * devicePixelRatio;
    ctx.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);
    flakes.length = 0;
    const n = Math.floor((canvas.clientWidth * canvas.clientHeight) / 18000);
    for (let i = 0; i < Math.max(28, Math.min(70, n)); i++) {
      flakes.push({
        x: Math.random() * canvas.clientWidth,
        y: Math.random() * canvas.clientHeight,
        r: 0.6 + Math.random() * 1.8,
        vx: -0.15 + Math.random() * 0.3,
        vy: 0.12 + Math.random() * 0.35,
        a: 0.15 + Math.random() * 0.45,
      });
    }
  };
  resize();
  window.addEventListener('resize', resize);
  const tick = () => {
    ctx.clearRect(0, 0, canvas.clientWidth, canvas.clientHeight);
    for (const p of flakes) {
      p.x += p.vx;
      p.y += p.vy;
      if (p.y > canvas.clientHeight + 4) { p.y = -4; p.x = Math.random() * canvas.clientWidth; }
      if (p.x < -4) p.x = canvas.clientWidth + 4;
      if (p.x > canvas.clientWidth + 4) p.x = -4;
      ctx.beginPath();
      ctx.fillStyle = `rgba(190, 220, 255, ${p.a})`;
      ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
      ctx.fill();
    }
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

// ------------------------------------------------------------------------------------------------ start
startStormFx();
(async () => {
  await refresh();
  // launcher self-update first (no-op when offline or up to date), then the local client folder check
  checkLauncherUpdate();
  refreshGuard();
  setInterval(refreshGuard, 5000);
  showClient();
  refreshClient();
  refreshStatus();
  if (state.email) refreshChecks();
  setInterval(refreshStatus, 60000);
})();
