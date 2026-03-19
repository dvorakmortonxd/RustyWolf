//webview.rs
use crate::cli::Cli;
use anyhow::{Result, anyhow};
use base64::Engine;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use tao::window::{Icon, Window, WindowBuilder};
use wry::{PageLoadEvent, Rect, WebView, WebViewBuilder};

const DEFAULT_HOME: &str = "https://duckduckgo.com";
const NEW_TAB_URL: &str = "https://duckduckgo.com";
const SEARCH_URL_PREFIX: &str = "https://duckduckgo.com/?q=";
const CHROME_HEIGHT_BASE: f64 = 84.0;
const CHROME_HEIGHT_DOWNLOADS_EXTRA: f64 = 104.0;
const CHROME_HEIGHT_POPUP_PROMPT_EXTRA: f64 = 44.0;
const MAX_HISTORY_ENTRIES: usize = 250;
const MAX_DOWNLOAD_ENTRIES: usize = 200;
const PROPERTIES_MAX_ROWS: usize = 120;

// ---------- Event types -----------------------------------------------

#[derive(Debug)]
enum UserEvent {
    ChromeIpc(String),
    TitleChanged { tab_id: u32, title: String },
    PageLoaded { tab_id: u32, url: String },
    DownloadStarted { url: String, file_path: String },
    DownloadTotal { file_path: String, total_bytes: Option<u64> },
    DownloadTick,
    DownloadCompleted { url: String, file_path: Option<String>, success: bool },
    OpenInNewTab { url: String },
    Quit,
}

// ---------- State -------------------------------------------------------

struct Tab {
    id: u32,
    title: String,
    url: String,
    kir_enabled: bool,
    webview: Option<WebView>,
}

struct BrowserState {
    chrome: WebView,
    host_mode: WebViewHostMode,
    tabs: Vec<Tab>,
    history: Vec<HistoryEntry>,
    downloads: Vec<DownloadEntry>,
    adblock_enabled: bool,
    popup_allow_hosts: HashSet<String>,
    pending_popup: Option<PendingPopup>,
    downloads_panel_open: bool,
    active: usize,
    next_id: u32,
    private: bool,
}

struct PendingPopup {
    url: String,
    host: String,
}

struct HistoryEntry {
    title: String,
    url: String,
    visited_at: String,
}

struct DownloadEntry {
    url: String,
    file_path: String,
    status: String,
    in_progress: bool,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    updated_at: String,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WebViewHostMode {
    Child,
    Window,
}

impl BrowserState {
    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    fn find_index(&self, tab_id: u32) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == tab_id)
    }
}

// ---------- Chrome HTML (static, separate webview) --------------------

const CHROME_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
html, body { height: 100%; overflow: hidden; background: #101012;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  -webkit-user-select: none; user-select: none; }
#bar { display: flex; flex-direction: column; height: 100%;
  padding: 6px 8px; gap: 6px;
  border-bottom: 1px solid rgba(255,255,255,0.15); }
#tabs-top { display: flex; align-items: center; gap: 6px; height: 30px; }
#tabs-row { flex: 1; display: flex; align-items: center; gap: 4px;
  height: 30px; overflow-x: auto; overflow-y: hidden; }
#nav-row { display: flex; align-items: center; gap: 6px; height: 36px; }
#downloads-row { display: none; flex-direction: column; align-items: stretch; gap: 6px; overflow-y: auto;
  border: 1px solid rgba(255,255,255,0.18); border-radius: 0; padding: 8px;
  background: rgba(255,255,255,0.07); min-height: 92px; }
.dl-empty { font-size: 12px; opacity: .75; }
.dl-item { display: flex; flex-direction: column; gap: 4px; border: 1px solid rgba(255,255,255,0.16);
  border-radius: 0; padding: 6px 8px; background: rgba(0,0,0,0.2); color: #e8e8ed; }
.dl-title { font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.dl-meta { font-size: 11px; opacity: .82; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.dl-track { width: 100%; height: 6px; background: rgba(255,255,255,0.18); border-radius: 0; overflow: hidden; }
.dl-fill { height: 100%; width: 0%; border-radius: 0; background: linear-gradient(90deg, #4f8bff, #72d8ff);
  transform-origin: left center; }
.dl-fill.indeterminate { width: 35%; animation: glide 1.05s ease-in-out infinite; }
@keyframes glide { 0% { transform: translateX(-120%); } 100% { transform: translateX(320%); } }
button { flex-shrink: 0; height: 30px; min-width: 30px; padding: 0 8px;
  border: 1px solid rgba(255,255,255,0.2); border-radius: 0;
  background: rgba(255,255,255,0.1); color: #e8e8ed;
  font-size: 14px; cursor: pointer; outline: none; }
button:active { background: rgba(255,255,255,0.2); }
#url-input { flex: 1; height: 30px; border: 1px solid rgba(255,255,255,0.2);
  border-radius: 0; padding: 4px 10px; font-size: 13px; outline: none;
  color: #f5f5f7; background: rgba(255,255,255,0.07);
  -webkit-user-select: text; user-select: text; }
#url-input:focus { border-color: rgba(90,140,255,0.7); background: rgba(255,255,255,0.1); }
.tab-btn { position: relative; display: inline-flex; align-items: center;
  max-width: 170px; height: 26px; font-size: 12px;
  padding-right: 42px; }
.tab-btn.unloaded:not(.active) { opacity: .72; }
.tab-title { display: block; width: 100%;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.tab-btn.active { background: rgba(90,140,255,0.75); border-color: rgba(90,140,255,0.5); }
.kir-btn { position: absolute; right: 22px; top: 2px;
  width: 18px; height: 18px; min-width: 18px; border-radius: 0;
  border: 1px solid rgba(255,255,255,0.2); background: rgba(0,0,0,0.25);
  color: #fff; font-size: 10px; line-height: 16px; padding: 0; }
.kir-btn.active { background: rgba(90,140,255,0.75); border-color: rgba(90,140,255,0.5); }
.close-btn { position: absolute; right: 3px; top: 2px;
  width: 18px; height: 18px; min-width: 18px; border-radius: 0;
  border: 1px solid rgba(255,255,255,0.2); background: rgba(0,0,0,0.25);
  color: #fff; font-size: 11px; line-height: 16px; padding: 0; }
#add-btn { height: 26px; min-width: 28px; font-size: 16px; }
#adblock-btn { height: 26px; min-width: 40px; font-size: 12px; font-weight: 700; }
#adblock-btn.active { background: rgba(90,140,255,0.75); border-color: rgba(90,140,255,0.5); }
#props-btn { height: 26px; min-width: 88px; font-size: 12px; }
#downloads-btn { height: 26px; min-width: 28px; font-size: 12px; font-weight: 700; }
#downloads-btn.active { background: rgba(90,140,255,0.75); border-color: rgba(90,140,255,0.5); }
#popup-row { display: none; align-items: center; justify-content: space-between; gap: 8px;
  min-height: 34px; padding: 6px 8px; border: 1px solid rgba(255,255,255,0.18);
  background: rgba(255,255,255,0.06); color: #e8e8ed; font-size: 12px; }
#popup-message { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
#popup-actions { display: inline-flex; align-items: center; gap: 6px; }
#popup-yes, #popup-no { height: 24px; min-width: 44px; font-size: 12px; }
</style>
</head>
<body>
<div id="bar">
  <div id="tabs-top">
    <div id="tabs-row"></div>
    <button id="adblock-btn" title="Toggle adblock">ADS</button>
    <button id="downloads-btn" title="Downloads">D</button>
    <button id="props-btn" title="History and downloads">Properties</button>
  </div>
  <div id="nav-row">
    <button id="back" title="Back">←</button>
    <button id="forward" title="Forward">→</button>
    <button id="reload" title="Reload">⟳</button>
    <input id="url-input" type="text" placeholder="Search or enter address"
           autocomplete="off" autocapitalize="off" spellcheck="false">
  </div>
  <div id="downloads-row"></div>
  <div id="popup-row">
    <div id="popup-message"></div>
    <div id="popup-actions">
      <button id="popup-yes">Yes</button>
      <button id="popup-no">No</button>
    </div>
  </div>
</div>
<script>
const post = (msg) => window.ipc && window.ipc.postMessage(msg);

const normalize = (raw) => {
  const value = (raw || '').trim();
  if (!value) return '';
  if (/^[a-zA-Z][a-zA-Z\d+.-]*:/.test(value)) return value;
  const looksLikeAddress =
    value.includes('.') || value.includes(':') ||
    value.startsWith('localhost') || value.startsWith('127.') ||
    value.startsWith('[') || value.startsWith('/');
  return looksLikeAddress ? 'https://' + value
    : 'https://duckduckgo.com/?q=' + encodeURIComponent(value);
};

const input = document.getElementById('url-input');

const isEditableTarget = (target) => {
  if (!target || !(target instanceof Element)) return false;
  if (target === input) return true;
  const tag = (target.tagName || '').toLowerCase();
  return tag === 'input' || tag === 'textarea' || target.isContentEditable;
};

document.getElementById('back').addEventListener('click', () => post('back'));
document.getElementById('forward').addEventListener('click', () => post('forward'));
document.getElementById('reload').addEventListener('click', () => post('reload'));
document.getElementById('props-btn').addEventListener('click', () => post('open_properties'));
document.getElementById('adblock-btn').addEventListener('click', () => post('toggle_adblock'));
document.getElementById('downloads-btn').addEventListener('click', () => post('toggle_downloads'));
document.getElementById('popup-yes').addEventListener('click', () => post('popup_allow_yes'));
document.getElementById('popup-no').addEventListener('click', () => post('popup_allow_no'));

input.addEventListener('keydown', (ev) => {
  if (ev.key === 'Enter') {
    ev.preventDefault();
    const next = normalize(input.value);
    if (next) post('navigate:' + next);
  }
});

const shortTitle = (tab, i) => {
  if (tab.title && tab.title.trim()) return tab.title.trim().slice(0, 22);
  try { return new URL(tab.url).hostname || 'Tab ' + (i + 1); }
  catch (_) { return 'Tab ' + (i + 1); }
};

window.__rustywolfSetTabs = (state) => {
  const row = document.getElementById('tabs-row');
  row.innerHTML = '';
  state.tabs.forEach((tab, i) => {
    const btn = document.createElement('button');
    btn.className =
      'tab-btn' +
      (tab.id === state.activeId ? ' active' : '') +
      (tab.loaded ? '' : ' unloaded');
    btn.title = tab.url;
    btn.addEventListener('click', () => post('switch_tab:' + tab.id));

    const title = document.createElement('span');
    title.className = 'tab-title';
    title.textContent = shortTitle(tab, i) + (tab.loaded ? '' : ' [S]');
    btn.appendChild(title);

    const kir = document.createElement('button');
    kir.className = 'kir-btn' + (tab.kirEnabled ? ' active' : '');
    kir.textContent = 'K';
    kir.title = tab.kirEnabled ? 'KIR On' : 'KIR Off';
    kir.addEventListener('click', (ev) => {
      ev.stopPropagation();
      ev.preventDefault();
      post('toggle_kir:' + tab.id);
    });
    btn.appendChild(kir);

    const x = document.createElement('button');
    x.className = 'close-btn';
    x.textContent = '-'; x.title = 'Close tab';
    x.addEventListener('click', (ev) => {
      ev.stopPropagation();
      ev.preventDefault();
      post('close_tab:' + tab.id);
    });
    btn.appendChild(x);
    row.appendChild(btn);
  });

  const add = document.createElement('button');
  add.id = 'add-btn'; add.textContent = '+'; add.title = 'New tab';
  add.addEventListener('click', () => post('new_tab'));
  row.appendChild(add);

  const active = state.tabs.find(t => t.id === state.activeId);
  if (active) input.value = (active.url && active.url !== 'about:blank') ? active.url : '';
};

const shortPath = (value) => {
  if (!value) return 'Unknown destination';
  if (value.length <= 42) return value;
  return '...' + value.slice(-39);
};

window.__rustywolfSetDownloads = (state) => {
  const row = document.getElementById('downloads-row');
  const button = document.getElementById('downloads-btn');
  const items = (state && Array.isArray(state.downloads)) ? state.downloads : [];
  const panelOpen = !!(state && state.panelOpen);
  row.innerHTML = '';
  if (!panelOpen) {
    row.style.display = 'none';
    button.classList.remove('active');
    return;
  }
  button.classList.add('active');
  row.style.display = 'flex';

  if (items.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'dl-empty';
    empty.textContent = 'No active downloads';
    row.appendChild(empty);
    return;
  }

  const formatBytes = (value) => {
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let num = Number(value) || 0;
    let unit = 0;
    while (num >= 1024 && unit < units.length - 1) {
      num /= 1024;
      unit += 1;
    }
    return (unit === 0 ? Math.round(num).toString() : num.toFixed(1)) + ' ' + units[unit];
  };

  items.slice(0, 6).forEach((entry) => {
    const card = document.createElement('div');
    card.className = 'dl-item';

    const title = document.createElement('div');
    title.className = 'dl-title';
    title.textContent = shortPath(entry.filePath);

    const meta = document.createElement('div');
    meta.className = 'dl-meta';
    let progressText = formatBytes(entry.downloadedBytes || 0) + ' downloaded';
    const total = Number(entry.totalBytes || 0);
    const done = Number(entry.downloadedBytes || 0);
    if (total > 0) {
      const pct = Math.max(0, Math.min(100, Math.floor((done / total) * 100)));
      progressText = formatBytes(done) + ' / ' + formatBytes(total) + ' (' + pct + '%)';
    }
    meta.textContent = entry.status + ' - ' + progressText;

    const track = document.createElement('div');
    track.className = 'dl-track';
    const fill = document.createElement('div');
    fill.className = 'dl-fill';
    if (entry.inProgress) {
      const totalBytes = Number(entry.totalBytes || 0);
      const downloadedBytes = Number(entry.downloadedBytes || 0);
      if (totalBytes > 0) {
        const pct = Math.max(0, Math.min(100, (downloadedBytes / totalBytes) * 100));
        fill.style.width = pct.toFixed(1) + '%';
      } else {
        fill.style.width = '35%';
        fill.classList.add('indeterminate');
      }
    } else if (entry.status === 'Completed') {
      fill.style.width = '100%';
      fill.style.background = 'linear-gradient(90deg, #36c977, #52e39a)';
    } else {
      fill.style.width = '100%';
      fill.style.background = 'linear-gradient(90deg, #b85b5b, #e47979)';
    }

    const src = document.createElement('div');
    src.className = 'dl-meta';
    src.textContent = entry.url || '';

    track.appendChild(fill);
    card.appendChild(title);
    card.appendChild(meta);
    card.appendChild(track);
    card.appendChild(src);
    row.appendChild(card);
  });
};

window.__rustywolfSetAdblock = (enabled) => {
  const button = document.getElementById('adblock-btn');
  if (!button) return;
  if (enabled) {
    button.classList.add('active');
    button.title = 'Adblock: On';
  } else {
    button.classList.remove('active');
    button.title = 'Adblock: Off';
  }
};

window.__rustywolfSetPopupPrompt = (state) => {
  const row = document.getElementById('popup-row');
  const msg = document.getElementById('popup-message');
  if (!row || !msg) return;
  const visible = !!(state && state.visible);
  if (!visible) {
    row.style.display = 'none';
    msg.textContent = '';
    return;
  }
  row.style.display = 'flex';
  const host = (state && state.host) ? String(state.host) : 'this website';
  msg.textContent = 'Allow ' + host + ' to open a new tab?';
};

document.addEventListener('keydown', (ev) => {
  const key = ev.key.toLowerCase();
  const wantsReload =
    key === 'f5' ||
    ((ev.metaKey || ev.ctrlKey) && key === 'r');
  if (!ev.defaultPrevented && wantsReload) {
    ev.preventDefault();
    post('reload');
    return;
  }

  const mod = ev.metaKey || ev.ctrlKey;
  if (!mod || ev.altKey || ev.defaultPrevented) return;

  if (key === 'l') {
    ev.preventDefault();
    input.focus();
    input.select();
    return;
  }

  if (isEditableTarget(ev.target)) return;

  if (key === 't') {
    ev.preventDefault();
    post('new_tab');
    return;
  }

  if (key === 'w') {
    ev.preventDefault();
    post('close_active_tab');
  }
});
</script>
</body>
</html>"#;

// ---------- Privacy-only init script for content webviews ------------

const PRIVACY_JS: &str = r#"
Object.defineProperty(navigator, 'doNotTrack', { get: () => '1' });
Object.defineProperty(window, 'openDatabase', { value: undefined });
if ('BatteryManager' in window) window.BatteryManager = undefined;
if ('RTCPeerConnection' in window) window.RTCPeerConnection = undefined;

// Keep reload shortcuts working when focus is inside the page webview.
window.addEventListener('keydown', (ev) => {
  if (ev.defaultPrevented) return;
  const key = (ev.key || '').toLowerCase();
  const wantsReload =
    key === 'f5' ||
    ((ev.metaKey || ev.ctrlKey) && key === 'r');
  if (wantsReload) {
    ev.preventDefault();
    location.reload();
  }
}, true);
"#;

const ADBLOCK_JS: &str = r#"
(() => {
  if (window.__rustywolfAdblockBooted) return;
  window.__rustywolfAdblockBooted = true;

  const storageKey = '__rustywolf_adblock_enabled';
  const readPersistedEnabled = () => {
    try {
      const raw = window.localStorage && window.localStorage.getItem(storageKey);
      if (raw === '0') return false;
      if (raw === '1') return true;
    } catch (_) {}
    return false;
  };
  const persistEnabled = (enabled) => {
    try {
      if (window.localStorage) {
        window.localStorage.setItem(storageKey, enabled ? '1' : '0');
      }
    } catch (_) {}
  };

  window.__rustywolfAdblockEnabled = readPersistedEnabled();

  const blockedHosts = [
    'doubleclick.net',
    'googlesyndication.com',
    'adservice.google.com',
    'google-analytics.com',
    'googletagmanager.com',
    'facebook.net',
    'ads.yahoo.com',
    'taboola.com',
    'outbrain.com'
  ];

  const cosmeticSelectors = [
    '[id^="ad-"]',
    '[id^="ads-"]',
    '[class^="ad-"]',
    '[class^="ads-"]',
    '[class*=" ad-"]',
    '[class*=" ads-"]',
    '[class*="advert"]',
    '[id*="sponsored"]',
    '[class*="sponsored"]',
    'iframe[src*="doubleclick"]',
    'iframe[src*="adservice"]'
  ];

  const isEnabled = () => window.__rustywolfAdblockEnabled !== false;

  const matchesBlockedHost = (host) => {
    const lower = (host || '').toLowerCase();
    return blockedHosts.some((blocked) => lower === blocked || lower.endsWith('.' + blocked));
  };

  const shouldBlockUrl = (rawUrl) => {
    if (!isEnabled() || !rawUrl) return false;
    try {
      const parsed = new URL(rawUrl, location.href);
      if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return false;
      if (matchesBlockedHost(parsed.hostname)) return true;
      const full = parsed.href.toLowerCase();
      return full.includes('/ads') || full.includes('adserver') || full.includes('tracking');
    } catch (_) {
      return false;
    }
  };

  const removeAds = () => {
    if (!isEnabled()) return;
    for (const selector of cosmeticSelectors) {
      document.querySelectorAll(selector).forEach((el) => {
        if (!el.hasAttribute('data-rw-adblock-hidden')) {
          el.setAttribute('data-rw-adblock-hidden', '1');
          el.setAttribute('data-rw-prev-display', el.style.getPropertyValue('display') || '');
          el.setAttribute('data-rw-prev-visibility', el.style.getPropertyValue('visibility') || '');
        }
        el.style.setProperty('display', 'none', 'important');
        el.style.setProperty('visibility', 'hidden', 'important');
        el.setAttribute('aria-hidden', 'true');
      });
    }
  };

  const restoreAds = () => {
    document.querySelectorAll('[data-rw-adblock-hidden="1"]').forEach((el) => {
      const prevDisplay = el.getAttribute('data-rw-prev-display') || '';
      const prevVisibility = el.getAttribute('data-rw-prev-visibility') || '';
      if (prevDisplay) {
        el.style.setProperty('display', prevDisplay);
      } else {
        el.style.removeProperty('display');
      }
      if (prevVisibility) {
        el.style.setProperty('visibility', prevVisibility);
      } else {
        el.style.removeProperty('visibility');
      }
      el.removeAttribute('data-rw-adblock-hidden');
      el.removeAttribute('data-rw-prev-display');
      el.removeAttribute('data-rw-prev-visibility');
      if (el.getAttribute('aria-hidden') === 'true') {
        el.removeAttribute('aria-hidden');
      }
    });
  };

  let adObserver = null;
  let scanQueued = false;

  const scheduleAdScan = () => {
    if (!isEnabled() || scanQueued) return;
    scanQueued = true;
    const run = () => {
      scanQueued = false;
      removeAds();
    };
    if (typeof requestAnimationFrame === 'function') {
      requestAnimationFrame(run);
    } else {
      setTimeout(run, 16);
    }
  };

  const stopObserver = () => {
    if (!adObserver) return;
    adObserver.disconnect();
    adObserver = null;
  };

  const startObserver = () => {
    stopObserver();
    if (!isEnabled()) return;
    const root = document.body || document.documentElement;
    if (!root) return;
    adObserver = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.type === 'childList' && mutation.addedNodes && mutation.addedNodes.length > 0) {
          scheduleAdScan();
          return;
        }
      }
    });
    adObserver.observe(root, { childList: true, subtree: true });
  };

  window.__rustywolfSetAdblockEnabled = (enabled) => {
    window.__rustywolfAdblockEnabled = !!enabled;
    persistEnabled(window.__rustywolfAdblockEnabled);
    if (window.__rustywolfAdblockEnabled) {
      startObserver();
      scheduleAdScan();
    } else {
      stopObserver();
      restoreAds();
    }
  };

  const originalFetch = window.fetch;
  if (typeof originalFetch === 'function') {
    window.fetch = function patchedFetch(input, init) {
      const target = typeof input === 'string' ? input : (input && input.url) || '';
      if (shouldBlockUrl(target)) {
        return Promise.resolve(new Response('', { status: 204, statusText: 'Blocked by RustyWolf' }));
      }
      return originalFetch.call(this, input, init);
    };
  }

  const originalXhrOpen = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function patchedOpen(method, url) {
    this.__rustywolfUrl = url;
    return originalXhrOpen.apply(this, arguments);
  };

  const originalXhrSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.send = function patchedSend() {
    if (shouldBlockUrl(this.__rustywolfUrl)) {
      this.abort();
      return;
    }
    return originalXhrSend.apply(this, arguments);
  };

  const patchSrcSetter = (ctor) => {
    if (!ctor || !ctor.prototype) return;
    const desc = Object.getOwnPropertyDescriptor(ctor.prototype, 'src');
    if (!desc || typeof desc.set !== 'function' || typeof desc.get !== 'function') return;
    Object.defineProperty(ctor.prototype, 'src', {
      configurable: true,
      enumerable: desc.enumerable,
      get: desc.get,
      set(value) {
        if (shouldBlockUrl(value)) {
          return desc.set.call(this, 'about:blank');
        }
        return desc.set.call(this, value);
      }
    });
  };

  patchSrcSetter(window.HTMLImageElement);
  patchSrcSetter(window.HTMLScriptElement);
  patchSrcSetter(window.HTMLIFrameElement);

  startObserver();
  scheduleAdScan();
})();
"#;

// ---------- Entry point ------------------------------------------------

pub fn launch_webkit(args: &Cli) -> Result<()> {
    apply_linux_runtime_overrides(args);
    let mut host_mode = resolve_webview_host_mode(args);

    let start_url = args
        .url
        .as_deref()
        .map(normalize_url)
        .unwrap_or_else(|| DEFAULT_HOME.to_string());
    let title = args.title.clone().unwrap_or_else(|| "RustyWolf".to_string());

    if args.dry_run {
        println!(
            "engine=webkit url={start_url} private={} linux_backend={:?} linux_disable_dmabuf={}",
            args.private, args.linux_backend, args.linux_disable_dmabuf
        );
        return Ok(());
    }

    let event_loop: EventLoop<UserEvent> = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title(title)
        .with_window_icon(load_window_icon())
        .with_inner_size(LogicalSize::new(1000.0_f64, 600.0_f64))
        .build(&event_loop)
        .map_err(|err| anyhow!("Failed to create window: {err}"))?;

    let (chrome, effective_host_mode) = build_chrome(&window, &proxy, host_mode)?;
    host_mode = effective_host_mode;

    let mut state = BrowserState {
        chrome,
        host_mode,
        tabs: Vec::new(),
        history: Vec::new(),
        downloads: Vec::new(),
        adblock_enabled: false,
        popup_allow_hosts: HashSet::new(),
        pending_popup: None,
        downloads_panel_open: false,
        active: 0,
        next_id: 1,
        private: args.private,
    };

    open_tab(&mut state, &window, &proxy, start_url)?;

    let tick_proxy = proxy.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(300));
        if tick_proxy.send_event(UserEvent::DownloadTick).is_err() {
            break;
        }
    });

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(ev) => {
                if matches!(ev, UserEvent::Quit) {
                    *control_flow = ControlFlow::Exit;
                } else if let Err(e) = handle_event(ev, &window, &proxy, &mut state) {
                    eprintln!("event error: {e:#}");
                }
            }
            Event::WindowEvent { event: WindowEvent::Resized(_size), .. } => {
                reflow_layout(&window, &state);
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

fn apply_linux_runtime_overrides(_args: &Cli) {
    #[cfg(target_os = "linux")]
    {
        match _args.linux_backend {
            crate::cli::LinuxBackend::Auto => {}
            crate::cli::LinuxBackend::X11 => {
                unsafe {
                    std::env::set_var("WINIT_UNIX_BACKEND", "x11");
                    std::env::set_var("GDK_BACKEND", "x11");
                }
            }
            crate::cli::LinuxBackend::Wayland => {
                unsafe {
                    std::env::set_var("WINIT_UNIX_BACKEND", "wayland");
                    std::env::set_var("GDK_BACKEND", "wayland");
                }
            }
        }

        if _args.linux_disable_dmabuf {
            // WebKitGTK + NVIDIA can fail with dmabuf/gbm allocation on some systems.
            unsafe {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            }
        }
    }
}

fn resolve_webview_host_mode(args: &Cli) -> WebViewHostMode {
    #[cfg(target_os = "linux")]
    {
        return match args.linux_backend {
            crate::cli::LinuxBackend::X11 => WebViewHostMode::Child,
            crate::cli::LinuxBackend::Wayland => WebViewHostMode::Window,
            crate::cli::LinuxBackend::Auto => {
                let forced = std::env::var("WINIT_UNIX_BACKEND").ok();
                if forced.as_deref() == Some("x11") {
                    return WebViewHostMode::Child;
                }
                if forced.as_deref() == Some("wayland") {
                    return WebViewHostMode::Window;
                }

                let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
                let has_wayland_display = std::env::var_os("WAYLAND_DISPLAY").is_some();
                if session_type.eq_ignore_ascii_case("wayland") || has_wayland_display {
                    WebViewHostMode::Window
                } else {
                    WebViewHostMode::Child
                }
            }
        };
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = args;
        WebViewHostMode::Child
    }
}

// ---------- Event handling ---------------------------------------------

fn handle_event(
    ev: UserEvent,
    window: &Window,
    proxy: &EventLoopProxy<UserEvent>,
    state: &mut BrowserState,
) -> Result<()> {
    match ev {
        UserEvent::ChromeIpc(msg) => handle_chrome_ipc(&msg, window, proxy, state)?,
        UserEvent::TitleChanged { tab_id, title } => {
            if let Some(i) = state.find_index(tab_id) {
                if !title.trim().is_empty() {
                    state.tabs[i].title = title;
                    push_tabs(state);
                }
            }
        }
        UserEvent::PageLoaded { tab_id, url } => {
            if let Some(i) = state.find_index(tab_id) {
                if !url.is_empty() && url != "about:blank" {
                    state.tabs[i].url = url;
                    if !is_internal_page(&state.tabs[i].url) {
                        append_history(
                            state,
                            state.tabs[i].title.clone(),
                            state.tabs[i].url.clone(),
                        );
                    }
                    let enabled = if state.adblock_enabled { "true" } else { "false" };
                    if let Some(webview) = state.tabs[i].webview.as_ref() {
                        let _ = webview.evaluate_script(&format!(
                            "window.__rustywolfSetAdblockEnabled && window.__rustywolfSetAdblockEnabled({enabled});"
                        ));
                    }
                    push_tabs(state);
                }
            }
        }
        UserEvent::DownloadStarted { url, file_path } => {
            state.downloads.push(DownloadEntry {
                url,
                file_path,
                status: "In progress".to_string(),
                in_progress: true,
                downloaded_bytes: 0,
                total_bytes: None,
                updated_at: now_stamp(),
            });
            trim_downloads(state);
            push_downloads(state);
        }
        UserEvent::DownloadTotal { file_path, total_bytes } => {
            if let Some(entry) = state
                .downloads
                .iter_mut()
                .rev()
                .find(|d| d.file_path == file_path && d.in_progress)
            {
                entry.total_bytes = total_bytes;
                push_downloads(state);
            }
        }
        UserEvent::DownloadTick => {
            let mut changed = false;
            for entry in state.downloads.iter_mut().filter(|d| d.in_progress) {
                if let Ok(meta) = fs::metadata(&entry.file_path) {
                    let bytes = meta.len();
                    if bytes != entry.downloaded_bytes {
                        entry.downloaded_bytes = bytes;
                        changed = true;
                    }
                }
            }
            if changed {
                push_downloads(state);
            }
        }
        UserEvent::DownloadCompleted { url, file_path, success } => {
            let path = file_path.unwrap_or_else(|| "Unknown destination".to_string());
            if let Some(entry) = state
                .downloads
                .iter_mut()
                .rev()
                .find(|d| d.url == url && d.in_progress)
            {
                entry.file_path = path;
                entry.status = if success {
                    "Completed".to_string()
                } else {
                    "Failed".to_string()
                };
                entry.in_progress = false;
                if let Ok(meta) = fs::metadata(&entry.file_path) {
                    entry.downloaded_bytes = meta.len();
                }
                entry.updated_at = now_stamp();
            } else {
                state.downloads.push(DownloadEntry {
                    url,
                    file_path: path,
                    status: if success {
                        "Completed".to_string()
                    } else {
                        "Failed".to_string()
                    },
                    in_progress: false,
                    downloaded_bytes: 0,
                    total_bytes: None,
                    updated_at: now_stamp(),
                });
                trim_downloads(state);
            }
            push_downloads(state);
        }
        UserEvent::OpenInNewTab { url } => {
            if should_open_new_tab_url(&url) {
                let trimmed = url.trim();
                let host = url::Url::parse(trimmed)
                    .ok()
                    .and_then(|u| u.host_str().map(ToString::to_string))
                    .unwrap_or_default();

                if !host.is_empty() && state.popup_allow_hosts.contains(&host) {
                    open_tab(state, window, proxy, trimmed.to_string())?;
                } else if state.pending_popup.is_none() {
                    state.pending_popup = Some(PendingPopup {
                        url: trimmed.to_string(),
                        host,
                    });
                    push_popup_prompt(state);
                    reflow_layout(window, state);
                }
            }
        }
        UserEvent::Quit => {}
    }
    Ok(())
}

fn handle_chrome_ipc(
    message: &str,
    window: &Window,
    proxy: &EventLoopProxy<UserEvent>,
    state: &mut BrowserState,
) -> Result<()> {
    if let Some(raw_url) = message.strip_prefix("navigate:") {
        ensure_active_tab_loaded(state, window, proxy)?;
        if let Some(tab) = active_tab_mut(state) {
            let next = normalize_url(raw_url);
            tab.url = next.clone();
            tab.title = "Loading…".to_string();
            if let Some(webview) = tab.webview.as_ref() {
                let _ = webview.load_url(&next);
            }
            push_tabs(state);
        }
        return Ok(());
    }

    if message == "back" {
        run_script_on_active_tab(state, "history.back();");
        return Ok(());
    }

    if message == "forward" {
        run_script_on_active_tab(state, "history.forward();");
        return Ok(());
    }

    if message == "reload" {
        run_script_on_active_tab(state, "location.reload();");
        return Ok(());
    }

    if message == "new_tab" {
        open_tab(state, window, proxy, NEW_TAB_URL.to_string())?;
        return Ok(());
    }

    if message == "open_properties" {
        let properties_url = properties_data_url(state);
        open_tab(state, window, proxy, properties_url)?;
        if let Some(tab) = state.tabs.last_mut() {
            tab.title = "Properties".to_string();
        }
        push_tabs(state);
        return Ok(());
    }

    if message == "toggle_downloads" {
        state.downloads_panel_open = !state.downloads_panel_open;
        reflow_layout(window, state);
        push_downloads(state);
        return Ok(());
    }

    if message == "popup_allow_yes" {
        if let Some(pending) = state.pending_popup.take() {
            if !pending.host.is_empty() {
                state.popup_allow_hosts.insert(pending.host);
            }
            push_popup_prompt(state);
            reflow_layout(window, state);
            open_tab(state, window, proxy, pending.url)?;
        }
        return Ok(());
    }

    if message == "popup_allow_no" {
        state.pending_popup = None;
        push_popup_prompt(state);
        reflow_layout(window, state);
        return Ok(());
    }

    if message == "toggle_adblock" {
        state.adblock_enabled = !state.adblock_enabled;
        apply_adblock_setting_to_tabs(state);
        push_adblock(state);
        return Ok(());
    }

    if message == "close_active_tab" {
        if !state.tabs.is_empty() {
            let tab_id = state.tabs[state.active].id;
            if close_tab(state, window, proxy, tab_id)? {
                let _ = proxy.send_event(UserEvent::Quit);
            }
        }
        return Ok(());
    }

    if let Some(raw) = message.strip_prefix("toggle_kir:") {
        if let Ok(target_id) = raw.parse::<u32>() {
            if let Some(i) = state.find_index(target_id) {
                state.tabs[i].kir_enabled = !state.tabs[i].kir_enabled;
                if i != state.active && !state.tabs[i].kir_enabled {
                    state.tabs[i].webview = None;
                }
                apply_visibility(state);
                push_tabs(state);
            }
        }
        return Ok(());
    }

    if let Some(raw) = message.strip_prefix("switch_tab:") {
        if let Ok(target_id) = raw.parse::<u32>() {
            if let Some(i) = state.find_index(target_id) {
                state.active = i;
                ensure_active_tab_loaded(state, window, proxy)?;
                suspend_background_tabs(state);
                apply_visibility(state);
                push_tabs(state);
            }
        }
        return Ok(());
    }

    if let Some(raw) = message.strip_prefix("close_tab:") {
        if let Ok(target_id) = raw.parse::<u32>() {
            if close_tab(state, window, proxy, target_id)? {
                let _ = proxy.send_event(UserEvent::Quit);
            }
        }
        return Ok(());
    }

    Ok(())
}

// ---------- Webview builders -------------------------------------------

fn build_chrome(
    window: &Window,
    proxy: &EventLoopProxy<UserEvent>,
    host_mode: WebViewHostMode,
) -> Result<(WebView, WebViewHostMode)> {
    let (lw, _lh) = logical_size(window);
    let builder = || {
        let chrome_proxy = proxy.clone();
        WebViewBuilder::new()
            .with_html(CHROME_HTML)
            .with_bounds(chrome_bounds(lw, CHROME_HEIGHT_BASE))
            .with_ipc_handler(move |req| {
                let _ = chrome_proxy.send_event(UserEvent::ChromeIpc(req.body().to_string()));
            })
    };

    match host_mode {
        WebViewHostMode::Window => builder()
            .build(window)
            .map(|webview| (webview, WebViewHostMode::Window))
            .map_err(|err| anyhow!("Failed to create chrome webview: {err}")),
        WebViewHostMode::Child => match builder().build_as_child(window) {
            Ok(webview) => Ok((webview, WebViewHostMode::Child)),
            Err(err) => {
                let err_text = err.to_string();
                if is_child_webview_unsupported_error(&err_text) {
                    eprintln!(
                        "child chrome webview unsupported on this backend; falling back to window-hosted chrome"
                    );
                    return builder()
                        .build(window)
                        .map(|webview| (webview, WebViewHostMode::Window))
                        .map_err(|fallback_err| {
                            anyhow!(
                                "Failed to create chrome webview: child failed ({err_text}); window fallback failed ({fallback_err})"
                            )
                        });
                }
                Err(anyhow!("Failed to create chrome webview: {err_text}"))
            }
        },
    }
}

fn is_child_webview_unsupported_error(err_text: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        let lower = err_text.to_ascii_lowercase();
        lower.contains("window handle kind is not supported")
            || (lower.contains("child") && lower.contains("not supported"))
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = err_text;
        false
    }
}

fn build_tab_webview(
    window: &Window,
    proxy: &EventLoopProxy<UserEvent>,
    host_mode: WebViewHostMode,
    private: bool,
    tab_id: u32,
    url: &str,
    bounds: Rect,
) -> Result<(WebView, WebViewHostMode)> {
    let init_script = format!("{PRIVACY_JS}\n{ADBLOCK_JS}");

    let builder = || {
        let title_proxy = proxy.clone();
        let page_proxy = proxy.clone();
        let download_started_proxy = proxy.clone();
        let download_completed_proxy = proxy.clone();
        let download_total_proxy = proxy.clone();
        let new_window_proxy = proxy.clone();

        WebViewBuilder::new()
            .with_url(url)
            .with_incognito(private)
            .with_initialization_script(&init_script)
            .with_bounds(bounds)
            .with_new_window_req_handler(move |target| {
                let _ = new_window_proxy.send_event(UserEvent::OpenInNewTab { url: target });
                false
            })
            .with_document_title_changed_handler(move |title| {
                let _ = title_proxy.send_event(UserEvent::TitleChanged { tab_id, title });
            })
            .with_on_page_load_handler(move |event, url| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = page_proxy.send_event(UserEvent::PageLoaded { tab_id, url });
                }
            })
            .with_download_started_handler(move |url, destination| {
                let path = ensure_unique_download_path(default_download_path_for(&url));
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                *destination = path.clone();
                let path_string = path_to_string(&path);
                fetch_download_total(url.clone(), path_string.clone(), download_total_proxy.clone());
                let _ = download_started_proxy.send_event(UserEvent::DownloadStarted {
                    url,
                    file_path: path_string,
                });
                true
            })
            .with_download_completed_handler(move |url, path, success| {
                let _ = download_completed_proxy.send_event(UserEvent::DownloadCompleted {
                    url,
                    file_path: path.map(|p| path_to_string(&p)),
                    success,
                });
            })
    };

    match host_mode {
        WebViewHostMode::Window => builder()
            .build(window)
            .map(|webview| (webview, WebViewHostMode::Window))
            .map_err(|err| anyhow!("Failed to create tab webview: {err}")),
        WebViewHostMode::Child => match builder().build_as_child(window) {
            Ok(webview) => Ok((webview, WebViewHostMode::Child)),
            Err(err) => {
                let err_text = err.to_string();
                if is_child_webview_unsupported_error(&err_text) {
                    eprintln!(
                        "child tab webview unsupported on this backend; falling back to window-hosted tabs"
                    );
                    return builder()
                        .build(window)
                        .map(|webview| (webview, WebViewHostMode::Window))
                        .map_err(|fallback_err| {
                            anyhow!(
                                "Failed to create tab webview: child failed ({err_text}); window fallback failed ({fallback_err})"
                            )
                        });
                }
                Err(anyhow!("Failed to create tab webview: {err_text}"))
            }
        },
    }
}

fn open_tab(
    state: &mut BrowserState,
    window: &Window,
    proxy: &EventLoopProxy<UserEvent>,
    url: String,
) -> Result<()> {
    let tab_id = state.alloc_id();
    let (lw, lh) = logical_size(window);
    let chrome_h = chrome_height(state);
    let (webview, effective_host_mode) = build_tab_webview(
        window,
        proxy,
        state.host_mode,
        state.private,
        tab_id,
        &url,
        content_bounds(lw, lh, chrome_h),
    )?;
    state.host_mode = effective_host_mode;

    state.tabs.push(Tab {
        id: tab_id,
        title: "New Tab".into(),
        url,
        kir_enabled: false,
        webview: Some(webview),
    });
    state.active = state.tabs.len() - 1;

    suspend_background_tabs(state);

    apply_visibility(state);
    push_tabs(state);
    push_downloads(state);
    push_adblock(state);
    push_popup_prompt(state);
    if state.adblock_enabled {
        if let Some(webview) = state.tabs[state.active].webview.as_ref() {
            let _ = webview.evaluate_script(
                "window.__rustywolfSetAdblockEnabled && window.__rustywolfSetAdblockEnabled(true);",
            );
        }
    }
    Ok(())
}

fn close_tab(
    state: &mut BrowserState,
    window: &Window,
    proxy: &EventLoopProxy<UserEvent>,
    tab_id: u32,
) -> Result<bool> {
    let Some(index) = state.find_index(tab_id) else { return Ok(false); };
    state.tabs.remove(index);

    if state.tabs.is_empty() {
        return Ok(true);
    }
    if state.active >= state.tabs.len() {
        state.active = state.tabs.len() - 1;
    } else if index < state.active {
        state.active -= 1;
    }
    ensure_active_tab_loaded(state, window, proxy)?;
    suspend_background_tabs(state);
    apply_visibility(state);
    push_tabs(state);
    Ok(false)
}

// ---------- Helpers ----------------------------------------------------

fn active_tab_mut(state: &mut BrowserState) -> Option<&mut Tab> {
    state.tabs.get_mut(state.active)
}

fn ensure_active_tab_loaded(
    state: &mut BrowserState,
    window: &Window,
    proxy: &EventLoopProxy<UserEvent>,
) -> Result<()> {
    if state.tabs.is_empty() {
        return Ok(());
    }
    ensure_tab_loaded(state, window, proxy, state.active)
}

fn ensure_tab_loaded(
    state: &mut BrowserState,
    window: &Window,
    proxy: &EventLoopProxy<UserEvent>,
    index: usize,
) -> Result<()> {
    if index >= state.tabs.len() || state.tabs[index].webview.is_some() {
        return Ok(());
    }
    let (lw, lh) = logical_size(window);
    let chrome_h = chrome_height(state);
    let tab_id = state.tabs[index].id;
    let tab_url = state.tabs[index].url.clone();
    let (webview, effective_host_mode) = build_tab_webview(
        window,
        proxy,
        state.host_mode,
        state.private,
        tab_id,
        &tab_url,
        content_bounds(lw, lh, chrome_h),
    )?;
    state.host_mode = effective_host_mode;
    state.tabs[index].webview = Some(webview);
    Ok(())
}

fn suspend_background_tabs(state: &mut BrowserState) {
    for (i, tab) in state.tabs.iter_mut().enumerate() {
        if i != state.active && !tab.kir_enabled {
            tab.webview = None;
        }
    }
}

fn run_script_on_active_tab(state: &BrowserState, script: &str) {
    if let Some(tab) = state.tabs.get(state.active) {
        if let Some(webview) = tab.webview.as_ref() {
            let _ = webview.evaluate_script(script);
        }
    }
}

fn apply_visibility(state: &BrowserState) {
    for (i, tab) in state.tabs.iter().enumerate() {
        if let Some(webview) = tab.webview.as_ref() {
            let _ = webview.set_visible(i == state.active);
        }
    }
}

fn push_tabs(state: &BrowserState) {
    if state.tabs.is_empty() { return; }
    let payload = json!({
        "tabs": state.tabs.iter().map(|t| json!({
            "id": t.id,
            "title": t.title,
            "url": t.url,
            "kirEnabled": t.kir_enabled,
            "loaded": t.webview.is_some(),
        })).collect::<Vec<_>>(),
        "activeId": state.tabs[state.active].id,
    })
    .to_string();
    let script = format!("window.__rustywolfSetTabs && window.__rustywolfSetTabs({payload});");
    let _ = state.chrome.evaluate_script(&script);
}

fn push_downloads(state: &BrowserState) {
    let payload = json!({
        "panelOpen": state.downloads_panel_open,
        "downloads": state
            .downloads
            .iter()
            .rev()
            .filter(|d| d.in_progress)
            .take(12)
            .map(|d| json!({
                "url": d.url,
                "filePath": d.file_path,
                "status": d.status,
                "inProgress": d.in_progress,
                "downloadedBytes": d.downloaded_bytes,
                "totalBytes": d.total_bytes,
            }))
            .collect::<Vec<_>>(),
    })
    .to_string();
    let script = format!(
        "window.__rustywolfSetDownloads && window.__rustywolfSetDownloads({payload});"
    );
    let _ = state.chrome.evaluate_script(&script);
}

fn push_adblock(state: &BrowserState) {
    let enabled = if state.adblock_enabled { "true" } else { "false" };
    let script = format!("window.__rustywolfSetAdblock && window.__rustywolfSetAdblock({enabled});");
    let _ = state.chrome.evaluate_script(&script);
}

fn push_popup_prompt(state: &BrowserState) {
    let payload = match &state.pending_popup {
        Some(pending) => json!({
            "visible": true,
            "host": if pending.host.is_empty() { "this website" } else { &pending.host },
        }),
        None => json!({ "visible": false }),
    }
    .to_string();
    let script = format!(
        "window.__rustywolfSetPopupPrompt && window.__rustywolfSetPopupPrompt({payload});"
    );
    let _ = state.chrome.evaluate_script(&script);
}

fn apply_adblock_setting_to_tabs(state: &BrowserState) {
    let enabled = if state.adblock_enabled { "true" } else { "false" };
    let script = format!(
        "window.__rustywolfSetAdblockEnabled && window.__rustywolfSetAdblockEnabled({enabled});"
    );
    for tab in &state.tabs {
        if let Some(webview) = tab.webview.as_ref() {
            let _ = webview.evaluate_script(&script);
        }
    }
}

fn logical_size(window: &Window) -> (f64, f64) {
    let scale = window.scale_factor();
    let phys = window.inner_size();
    (phys.width as f64 / scale, phys.height as f64 / scale)
}

fn chrome_bounds(logical_w: f64, chrome_h: f64) -> Rect {
    Rect {
        position: wry::dpi::LogicalPosition::new(0.0, 0.0).into(),
        size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(logical_w, chrome_h)),
    }
}

fn content_bounds(logical_w: f64, logical_h: f64, chrome_h: f64) -> Rect {
    Rect {
        position: wry::dpi::LogicalPosition::new(0.0, chrome_h).into(),
        size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(
            logical_w,
            (logical_h - chrome_h).max(0.0),
        )),
    }
}

fn reflow_layout(window: &Window, state: &BrowserState) {
    let (lw, lh) = logical_size(window);
    let chrome_h = chrome_height(state);
    let _ = state.chrome.set_bounds(chrome_bounds(lw, chrome_h));
    let cb = content_bounds(lw, lh, chrome_h);
    for tab in &state.tabs {
        if let Some(webview) = tab.webview.as_ref() {
            let _ = webview.set_bounds(cb);
        }
    }
}

fn load_window_icon() -> Option<Icon> {
    let image = image::load_from_memory_with_format(
        include_bytes!("rustywolf.ico"),
        image::ImageFormat::Ico,
    )
    .ok()?
    .into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).ok()
}

fn chrome_height(state: &BrowserState) -> f64 {
    let mut height = CHROME_HEIGHT_BASE;
    if state.downloads_panel_open {
        height += CHROME_HEIGHT_DOWNLOADS_EXTRA;
    }
    if state.pending_popup.is_some() {
        height += CHROME_HEIGHT_POPUP_PROMPT_EXTRA;
    }
    height
}

fn normalize_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed == "about:blank" { return trimmed.to_string(); }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }
    if looks_like_address(trimmed) {
        return format!("https://{trimmed}");
    }
    let encoded: String = url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect();
    format!("{SEARCH_URL_PREFIX}{encoded}")
}

fn should_open_new_tab_url(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("about:blank") {
        return false;
    }
    if trimmed.to_ascii_lowercase().starts_with("javascript:") {
        return false;
    }

    let Ok(parsed) = url::Url::parse(trimmed) else {
        return false;
    };

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return false;
    }

    let host = parsed.host_str().unwrap_or_default();
    let path = parsed.path().to_ascii_lowercase();
    if (host == "duckduckgo.com" || host.ends_with(".duckduckgo.com"))
        && (path == "/post3.html" || path == "/page3" || path == "/page3.html")
    {
        return false;
    }

    true
}

fn properties_data_url(state: &BrowserState) -> String {
    let html = render_properties_html(state);
    let encoded = base64::engine::general_purpose::STANDARD.encode(html.as_bytes());
    format!("data:text/html;base64,{encoded}")
}

fn render_properties_html(state: &BrowserState) -> String {
    let history_is_truncated = state.history.len() > PROPERTIES_MAX_ROWS;
    let downloads_are_truncated = state.downloads.len() > PROPERTIES_MAX_ROWS;
    let history_rows = if state.history.is_empty() {
        "<tr><td colspan=\"3\">No history yet.</td></tr>".to_string()
    } else {
        state
            .history
            .iter()
            .rev()
            .take(PROPERTIES_MAX_ROWS)
            .map(|entry| {
                format!(
                    "<tr><td>{}</td><td><a href=\"{}\">{}</a></td><td>{}</td></tr>",
                    escape_html(&entry.visited_at),
                    escape_html(&entry.url),
                    escape_html(&entry.title),
                    escape_html(&entry.url),
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    let download_rows = if state.downloads.is_empty() {
        "<tr><td colspan=\"4\">No downloads yet.</td></tr>".to_string()
    } else {
        state
            .downloads
            .iter()
            .rev()
            .take(PROPERTIES_MAX_ROWS)
            .map(|entry| {
                let file_url = path_string_to_file_url(&entry.file_path);
                format!(
                    "<tr><td>{}</td><td>{}</td><td><a href=\"{}\">{}</a></td><td>{}</td></tr>",
                    escape_html(&entry.updated_at),
                    escape_html(&entry.status),
                    escape_html(&file_url),
                    escape_html(&entry.file_path),
                    escape_html(&entry.url),
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>RustyWolf Properties</title>\
        <style>body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;background:#141418;color:#e9e9ee;margin:0;padding:20px}}\
        h1{{font-size:24px;margin:0 0 14px}} h2{{font-size:17px;margin:20px 0 8px}}\
        table{{width:100%;border-collapse:collapse;background:#1c1c22;border:1px solid rgba(255,255,255,.12);border-radius:0;overflow:hidden}}\
        th,td{{padding:8px 10px;border-bottom:1px solid rgba(255,255,255,.08);font-size:13px;text-align:left;vertical-align:top}}\
        th{{background:#23232b;color:#f4f4f8}} tr:last-child td{{border-bottom:none}}\
        a{{color:#8ab4ff;text-decoration:none}} a:hover{{text-decoration:underline}} .muted{{opacity:.75;font-size:12px}}</style></head>\
        <body><h1>Browser Properties</h1><p class=\"muted\">Session history and download activity.</p>\
        <h2>Browsing History</h2><p class=\"muted\">{}</p><table><thead><tr><th>Visited</th><th>Title</th><th>URL</th></tr></thead><tbody>{history_rows}</tbody></table>\
        <h2>Download History</h2><p class=\"muted\">{}</p><table><thead><tr><th>Updated</th><th>Status</th><th>Saved To</th><th>Source URL</th></tr></thead><tbody>{download_rows}</tbody></table>\
        </body></html>"
        ,
        if history_is_truncated {
            format!("Showing latest {PROPERTIES_MAX_ROWS} entries.")
        } else {
            "Showing all entries.".to_string()
        },
        if downloads_are_truncated {
            format!("Showing latest {PROPERTIES_MAX_ROWS} entries.")
        } else {
            "Showing all entries.".to_string()
        }
    )
}

fn append_history(state: &mut BrowserState, title: String, url: String) {
    if url.is_empty() {
        return;
    }
    state.history.push(HistoryEntry {
        title: if title.trim().is_empty() {
            url.clone()
        } else {
            title
        },
        url,
        visited_at: now_stamp(),
    });
    if state.history.len() > MAX_HISTORY_ENTRIES {
        let remove_count = state.history.len() - MAX_HISTORY_ENTRIES;
        state.history.drain(0..remove_count);
    }
}

fn trim_downloads(state: &mut BrowserState) {
    if state.downloads.len() > MAX_DOWNLOAD_ENTRIES {
        let remove_count = state.downloads.len() - MAX_DOWNLOAD_ENTRIES;
        state.downloads.drain(0..remove_count);
    }
}

fn default_download_path_for(url: &str) -> PathBuf {
    let mut dir = dirs::download_dir()
        .or_else(dirs::home_dir)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(std::env::temp_dir);
    dir.push(filename_from_url(url));
    dir
}

fn ensure_unique_download_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }

    let parent = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("download")
        .to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
        .map(ToString::to_string);

    for idx in 1..=9999 {
        let file_name = match &ext {
            Some(ext) => format!("{stem} ({idx}).{ext}"),
            None => format!("{stem} ({idx})"),
        };
        let candidate = parent.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    parent.join(format!("{stem}-{}", now_stamp()))
}

fn filename_from_url(url: &str) -> String {
    let raw = url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.path_segments().and_then(|mut parts| {
                parts
                    .next_back()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(ToString::to_string)
            })
        })
        .unwrap_or_else(|| "download.bin".to_string());

    sanitize_filename(&raw)
}

fn path_to_string(path: &PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

fn now_stamp() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "0".to_string(),
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn is_internal_page(url: &str) -> bool {
    url.starts_with("data:text/html;base64,")
}

fn path_string_to_file_url(path: &str) -> String {
    url::Url::from_file_path(Path::new(path))
        .map(|u| u.to_string())
        .unwrap_or_else(|_| format!("file://{path}"))
}

fn sanitize_filename(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let invalid = matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            || ch.is_control();
        out.push(if invalid { '_' } else { ch });
    }
    let trimmed = out.trim_matches('.').trim();
    if trimmed.is_empty() {
        "download.bin".to_string()
    } else {
        trimmed.to_string()
    }
}

fn fetch_download_total(url: String, file_path: String, proxy: EventLoopProxy<UserEvent>) {
    thread::spawn(move || {
        let total_bytes = ureq::head(&url)
            .call()
            .ok()
            .and_then(|resp| resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok()));
        let _ = proxy.send_event(UserEvent::DownloadTotal {
            file_path,
            total_bytes,
        });
    });
}

fn looks_like_address(value: &str) -> bool {
    value.contains('.')
        || value.contains(':')
        || value.starts_with("localhost")
        || value.starts_with("127.")
        || value.starts_with('[')
        || value.starts_with('/')
}

// ---------- Tests -------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{normalize_url, sanitize_filename, should_open_new_tab_url};

    #[test]
    fn keeps_existing_scheme() {
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
    }

    #[test]
    fn adds_https_for_domains() {
        assert_eq!(normalize_url("example.com"), "https://example.com");
    }

    #[test]
    fn keeps_about_blank() {
        assert_eq!(normalize_url("about:blank"), "about:blank");
    }

    #[test]
    fn converts_search_terms_to_duckduckgo_query() {
        assert_eq!(normalize_url("privacy browser"), "https://duckduckgo.com/?q=privacy+browser");
    }

    #[test]
    fn converts_single_token_to_duckduckgo_query() {
        assert_eq!(normalize_url("rust"), "https://duckduckgo.com/?q=rust");
    }

    #[test]
    fn trims_before_search() {
        assert_eq!(normalize_url("   rust webview   "), "https://duckduckgo.com/?q=rust+webview");
    }

    #[test]
    fn blocks_blank_and_about_blank_new_tabs() {
        assert!(!should_open_new_tab_url(""));
        assert!(!should_open_new_tab_url(" about:blank "));
    }

    #[test]
    fn blocks_duckduckgo_internal_popup_paths() {
        assert!(!should_open_new_tab_url("https://duckduckgo.com/post3.html"));
        assert!(!should_open_new_tab_url("https://duckduckgo.com/page3"));
        assert!(!should_open_new_tab_url("https://duckduckgo.com/page3.html"));
    }

    #[test]
    fn allows_regular_https_new_tab_targets() {
        assert!(should_open_new_tab_url("https://example.com/"));
        assert!(should_open_new_tab_url("https://duckduckgo.com/?q=rust"));
    }

    #[test]
    fn sanitizes_windows_unsafe_filename_chars() {
        assert_eq!(sanitize_filename("report:2026?.pdf"), "report_2026_.pdf");
    }

    #[test]
    fn falls_back_when_filename_becomes_empty() {
        assert_eq!(sanitize_filename("..."), "download.bin");
    }
}
