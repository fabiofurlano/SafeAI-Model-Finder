// ── SafeAI Model Finder — Frontend Application ─────────────────
// Presentation-only. All data comes from the local backend;
// nothing leaves this computer. UI is fully localised (en/it)
// through embedded locale files served by the same local server.

const API = '/api';
const LANGS = ['en', 'it'];
let currentMode = 'easy';
let currentUseCase = null;
let currentView = 'overview';
let pollInterval = null;
let downloadActive = false; // one download at a time: buttons grey out while busy
let browseTimer = null;
let guideTimer = null;
let sessionToken = new URLSearchParams(window.location.search).get('token') || '';

// ── Icons (one family: 24×24, stroke 1.7–1.9, round caps) ─────
const ICONS = {
    download: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v12M7 10l5 5 5-5M4 21h16"/></svg>',
    check: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12l5 5 9-10"/></svg>',
    retry: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-2.6-6.4"/><path d="M21 3v6h-6"/></svg>',
    compare: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3v18M16 3v18M3 8h5M3 16h5M16 8h5M16 16h5"/></svg>',
    trash: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6.5 7l.8 12a2 2 0 0 0 2 1.9h5.4a2 2 0 0 0 2-1.9l.8-12M10 11v6M14 11v6"/></svg>',
    close: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>',
};

// Guide topic icons — same family as the rest of the UI.
const GUIDE_ICONS = {
    size: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2l8 4.5v9L12 20l-8-4.5v-9z"/><path d="M12 11L4 6.5M12 11l8-4.5M12 11v9"/></svg>',
    quant: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2l9 4.5-9 4.5-9-4.5z"/><path d="M3 12l9 4.5 9-4.5"/><path d="M3 16.5L12 21l9-4.5"/></svg>',
    ram: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="4" width="16" height="12" rx="2"/><path d="M8 8h8M8 12h8M9 16v4M15 16v4"/></svg>',
    speed: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 14l4-4"/><path d="M3.3 16a9 9 0 1 1 17.4 0"/></svg>',
    context: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 6h16M4 12h16M4 18h10"/></svg>',
    caps: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l2 4.5 5 .5-3.7 3.4L16.5 17 12 14.5 7.5 17l1.2-5.6L5 8l5-.5z"/></svg>',
    moe: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="5" cy="6" r="2.2"/><circle cx="19" cy="6" r="2.2"/><circle cx="12" cy="18" r="2.2"/><path d="M6.5 7.6L10.8 16M17.5 7.6L13.2 16M7.2 6h9.6"/></svg>',
    tag: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12V4a1 1 0 0 1 1-1h8l9 9-8 8z"/><circle cx="8" cy="8" r="1.5"/></svg>',
    license: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z"/><path d="M9 12l2 2 4-4"/></svg>',
    measured: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12h4l2-7 4 14 2-7h6"/></svg>',
};

// ── Internationalisation ──────────────────────────────────────
const I18N = {
    lang: 'en',
    data: null,
    fallback: null,
    locales: {}, // { en: {...}, it: {...} } — both embedded, loaded once

    async init() {
        let lang = 'en';
        try {
            const saved = localStorage.getItem('smf-lang');
            if (saved && LANGS.includes(saved)) {
                lang = saved;
            } else {
                const nav = (navigator.language || 'en').toLowerCase();
                lang = nav.startsWith('it') ? 'it' : 'en';
            }
        } catch (e) { /* private mode */ }
        this.lang = lang;
        try {
            const [en, it] = await Promise.all([
                fetch('/assets/locales/en.json').then(r => r.json()),
                fetch('/assets/locales/it.json').then(r => r.json()),
            ]);
            this.locales = { en, it };
            this.fallback = en;
            this.data = this.locales[lang] || en;
        } catch (e) {
            console.error('Could not load locale files', e);
            this.data = {};
            this.fallback = {};
        }
    },

    t(key, vars) {
        let v = key.split('.').reduce((o, k) => (o || {})[k], this.data);
        if (v === undefined) {
            v = key.split('.').reduce((o, k) => (o || {})[k], this.fallback);
            if (v === undefined) {
                console.warn('missing i18n key', key);
                return key;
            }
        }
        if (typeof v === 'string' && vars) {
            for (const [k, val] of Object.entries(vars)) {
                v = v.split(`{${k}}`).join(String(val));
            }
        }
        return v;
    },

    setLang(lang) {
        if (!LANGS.includes(lang)) lang = 'en';
        this.lang = lang;
        this.data = this.locales[lang] || this.fallback;
        try { localStorage.setItem('smf-lang', lang); } catch (e) { /* private mode */ }
        document.documentElement.lang = lang;
        document.documentElement.setAttribute('data-lang', lang);
        applyStaticTranslations();
        document.querySelectorAll('.seg-lang-btn').forEach(b => {
            b.setAttribute('aria-pressed', String(b.dataset.lang === lang));
        });
        // Re-render everything dynamic.
        renderHardwareFromCache();
        loadOllamaStatus();
        if (currentUseCase) fetchRecommendations();
        if (currentView === 'browse') { if (lastBrowseData) renderBrowse(lastBrowseData, lastBrowseQuery); renderCompareBar(); renderComparePanel(); updateModeVisibility(); }
        if (currentView === 'installed') loadInstalled();
        if (currentView === 'guide') renderGuide();
        if (currentView === 'performance') renderBenchHistory();
        if (currentView === 'planner' && lastPlanData) renderPlan(lastPlanData);
        if (currentView === 'planner' && plannerPicked) renderSelectedModelCard(plannerPicked);
    },
};

const t = (key, vars) => I18N.t(key, vars);

function plural(n, key1, keyN, extra) {
    return t(n === 1 ? key1 : keyN, Object.assign({ n }, extra || {}));
}

// ── Human-language helpers (localised) ────────────────────────
function humanCpu(name, cores) {
    let n = (name || '').replace(/\([^)]*\)/g, '').replace(/@\s*[\d.]+GHz/i, '').trim();
    const i = n.match(/\bi\d\b/i);
    const coreHint = plural(cores, 'overview.cpuCores1', 'overview.cpuCoresN');
    if (i) return { value: `Intel Core ${i[0].toLowerCase()}`, hint: coreHint };
    const apple = n.match(/\bM\d\b/i);
    if (apple) return { value: `Apple ${apple[0].toUpperCase()}`, hint: coreHint };
    const ryzen = n.match(/\bRyzen[\w\s-]*/i);
    if (ryzen) return { value: ryzen[0].trim(), hint: coreHint };
    const words = n.split(/\s+/).filter(Boolean).slice(0, 3).join(' ');
    return { value: words || 'Processor', hint: coreHint };
}

function memoryMeaning(gb) {
    if (gb >= 32) return t('overview.memory32');
    if (gb >= 16) return t('overview.memory16');
    if (gb >= 8) return t('overview.memory8');
    return t('overview.memorySmall');
}

function humanGpu(specs) {
    if (!specs.has_gpu) return { value: t('overview.gpuNone'), hint: t('overview.gpuNoneHint') };
    const raw = specs.gpu_name || 'Graphics';
    let name = raw.replace(/\([^)]*\)/g, '').trim();
    const br = name.match(/\[([^\]]+)\]/);
    if (br) name = br[1];
    name = name.replace(/\s*Graphics.*$/i, '').trim();
    if (!name) name = raw.split(' ').slice(0, 2).join(' ');
    let hint = 'Graphics';
    if (/integrated|iris|tiger|uhd|radeon.*integrated/i.test(raw)) hint = t('overview.gpuIntegrated');
    else if (specs.gpu_vram_gb && specs.gpu_vram_gb >= 8) hint = t('overview.gpuDedicated', { n: Math.round(specs.gpu_vram_gb) });
    else if (specs.gpu_vram_gb) hint = t('overview.gpuPlain', { n: Math.round(specs.gpu_vram_gb) });
    return { value: name, hint };
}

function humanSpeed(tps) {
    if (tps >= 100) return t('speed.veryFast');
    if (tps >= 30) return t('speed.fast');
    if (tps >= 10) return t('speed.moderate');
    return t('speed.slow');
}

function humanFit(fit) {
    switch (fit) {
        case 'Perfect': return t('fit.perfect');
        case 'Good': return t('fit.good');
        case 'Marginal': return t('fit.marginal');
        default: return t('fit.tight');
    }
}

function humanName(hf) {
    let n = (hf || '').split('/').pop() || hf || '';
    n = n.replace(/-(Instruct|Chat|Base|Preview|IT|SFT)(-[\w.]+)?$/i, '');
    n = n.replace(/[-_]+/g, ' ');
    return n.trim() || hf;
}

function formatGb(bytes) {
    if (!bytes) return '0 GB';
    return `${(bytes / (1024 ** 3)).toFixed(1)} GB`;
}

// ── Theme ─────────────────────────────────────────────────────
function initTheme() {
    const saved = localStorage.getItem('smf-theme');
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    document.documentElement.dataset.theme = saved || (prefersDark ? 'dark' : 'light');
}

function toggleTheme() {
    const next = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = next;
    try { localStorage.setItem('smf-theme', next); } catch (e) { /* private mode */ }
}

// ── Static translation pass ───────────────────────────────────
function applyStaticTranslations() {
    document.querySelectorAll('[data-i18n]').forEach(el => {
        el.textContent = t(el.dataset.i18n);
    });
    document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
        el.setAttribute('placeholder', t(el.dataset.i18nPlaceholder));
    });
    document.querySelectorAll('[data-i18n-aria]').forEach(el => {
        el.setAttribute('aria-label', t(el.dataset.i18nAria));
    });
    document.title = `${t('brand.name')} ${t('brand.finder')}`;
}

// ── View navigation ───────────────────────────────────────────
function showView(view) {
    currentView = view;
    document.querySelectorAll('.view').forEach(v => { v.hidden = true; });
    const target = document.getElementById(`view-${view}`);
    if (target) target.hidden = false;
    document.querySelectorAll('.nav-item').forEach(n => {
        const active = n.dataset.view === view;
        n.toggleAttribute('aria-current', active);
    });
    if (view === 'browse') runBrowse();
    if (view === 'installed') loadInstalled();
    if (view === 'guide') renderGuide();
    if (view === 'performance') { loadBenchModelSelect(); renderBenchHistory(); }
    if (view === 'planner') renderPlannerSuggestions('');
    updateModeVisibility();
    window.scrollTo({ top: 0, behavior: 'smooth' });
}

// ── Init ──────────────────────────────────────────────────────
document.addEventListener('DOMContentLoaded', async () => {
    initTheme();
    await I18N.init();
    applyStaticTranslations();
    document.documentElement.lang = I18N.lang;

    document.getElementById('themeToggle').addEventListener('click', toggleTheme);

    document.querySelectorAll('.nav-item').forEach(nav => {
        nav.addEventListener('click', () => showView(nav.dataset.view));
    });
    document.querySelectorAll('.seg-lang-btn').forEach(btn => {
        btn.addEventListener('click', () => I18N.setLang(btn.dataset.lang));
    });
    document.getElementById('ctaFind').addEventListener('click', () => showView('find'));
    document.getElementById('ctaBrowse').addEventListener('click', () => showView('browse'));

    setupModeToggle();
    setupUseCases();
    setupBrowse();
    setupCompare();
    setupGuide();
    setupProgressClose();
    setupPerformance();
    setupPlanner();
    document.getElementById('refreshInstalledBtn').addEventListener('click', () => {
        loadInstalled();
        loadOllamaStatus();
    });

    loadSystemInfo();
    loadOllamaStatus();
});

// ── Mode Toggle ───────────────────────────────────────────────
function setupModeToggle() {
    const toggle = document.getElementById('modeToggle');
    toggle.addEventListener('change', () => {
        currentMode = toggle.checked ? 'advanced' : 'easy';
        updateModeVisibility();
        if (currentUseCase) fetchRecommendations();
        if (currentView === 'browse') runBrowse();
        if (currentView === 'planner' && plannerPicked) renderSelectedModelCard(plannerPicked);
    });
}

function updateModeVisibility() {
    const raw = document.getElementById('rawSpecs');
    if (raw && raw.dataset.hasSpecs) raw.hidden = currentMode !== 'advanced';

    const panel = document.getElementById('advancedPanel');
    if (panel) panel.hidden = currentMode !== 'advanced';

    const filters = document.getElementById('browseFilters');
    if (filters) filters.hidden = currentMode !== 'advanced';

    const plannerAdvanced = document.querySelector('.planner-advanced');
    if (plannerAdvanced) plannerAdvanced.hidden = currentMode !== 'advanced';
}

// ── Use Cases ────────────────────────────────────────────────
function setupUseCases() {
    document.querySelectorAll('.use-case-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            document.querySelectorAll('.use-case-btn').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            currentUseCase = btn.dataset.useCase;
            fetchRecommendations();
        });
    });
}

// ── Browse ────────────────────────────────────────────────────
let lastBrowseData = null;
let lastBrowseQuery = '';

async function setupBrowse() {
    const input = document.getElementById('browseSearch');
    input.addEventListener('input', () => {
        clearTimeout(browseTimer);
        browseTimer = setTimeout(runBrowse, 350);
    });
    input.addEventListener('keydown', e => {
        if (e.key === 'Enter') runBrowse();
    });
    document.getElementById('browseSort').addEventListener('change', runBrowse);
    document.getElementById('browseMinFit').addEventListener('change', runBrowse);
    // Advanced filter row.
    document.getElementById('filterInstalled').addEventListener('change', runBrowse);
    document.getElementById('filterVision').addEventListener('change', runBrowse);
    document.getElementById('filterTools').addEventListener('change', runBrowse);
    document.getElementById('filterSize').addEventListener('change', runBrowse);
    document.getElementById('filterLicense').addEventListener('change', runBrowse);
    document.getElementById('filterLanguage').addEventListener('change', runBrowse);
    // Populate licence/language options from the catalogue (local data).
    try {
        const opts = await apiGet('/models/filter-options');
        const licSel = document.getElementById('filterLicense');
        (opts.licences || []).forEach(l => {
            const o = document.createElement('option');
            o.value = l;
            o.textContent = l;
            licSel.appendChild(o);
        });
        const langSel = document.getElementById('filterLanguage');
        (opts.languages || []).forEach(l => {
            const o = document.createElement('option');
            o.value = l;
            o.textContent = l;
            langSel.appendChild(o);
        });
    } catch (e) { /* options are a bonus */ }

    ['ovMemory', 'ovRam', 'ovCores', 'ovContext'].forEach(id => {
        document.getElementById(id).addEventListener('change', () => {
            if (currentView === 'browse') runBrowse();
            if (currentUseCase) fetchRecommendations();
        });
    });
}

function browseParams() {
    const p = new URLSearchParams({ limit: '30' });
    const q = document.getElementById('browseSearch').value.trim();
    if (q) p.set('q', q);
    const sort = document.getElementById('browseSort').value;
    if (sort) p.set('sort', sort);
    const minFit = document.getElementById('browseMinFit').value;
    if (minFit) p.set('min_fit', minFit);
    // Advanced filters (progressive disclosure).
    if (document.getElementById('filterInstalled').checked) p.set('installed', 'true');
    const caps = [];
    if (document.getElementById('filterVision').checked) caps.push('Vision');
    if (document.getElementById('filterTools').checked) caps.push('Tool Use');
    if (caps.length) p.set('caps', caps.join(','));
    const size = document.getElementById('filterSize').value;
    if (size) p.set('size', size);
    const lic = document.getElementById('filterLicense').value;
    if (lic) p.set('license', lic);
    const lang = document.getElementById('filterLanguage').value;
    if (lang) p.set('language', lang);
    applyOverrides(p);
    return p;
}

function applyOverrides(p) {
    const mem = document.getElementById('ovMemory').value;
    const ram = document.getElementById('ovRam').value;
    const cores = document.getElementById('ovCores').value;
    const ctx = document.getElementById('ovContext').value;
    if (mem) p.set('memory', mem);
    if (ram) p.set('ram', ram);
    if (cores) p.set('cpu_cores', cores);
    if (ctx) p.set('context', ctx);
}

function recommendationParams() {
    const p = new URLSearchParams();
    if (currentUseCase) p.set('use_case', currentUseCase);
    p.set('mode', currentMode);
    applyOverrides(p);
    return p;
}

async function runBrowse() {
    const box = document.getElementById('browseResults');
    const q = document.getElementById('browseSearch').value.trim();
    lastBrowseQuery = q;
    box.innerHTML = `<div class="spinner" role="status" aria-label="${escapeAttr(t('browse.searchingAria'))}"></div>`;

    try {
        const data = await apiGet(`/models/search?${browseParams().toString()}`);
        lastBrowseData = data;
        renderBrowse(data, q);
    } catch (e) {
        lastBrowseData = null;
        box.innerHTML = `<p class="browse-empty">${escapeHtml(t('browse.loadError', { msg: e.message }))}</p>`;
    }
}

function renderBrowse(data, query) {
    const box = document.getElementById('browseResults');
    const results = data.results || [];
    const n = data.total || 0;
    const summary = query
        ? `<p class="browse-summary">${escapeHtml(plural(n, 'browse.summaryQuery1', 'browse.summaryQueryN', { q: query }))}</p>`
        : `<p class="browse-summary">${escapeHtml(plural(n, 'browse.summaryAll1', 'browse.summaryAllN', { m: String(results.length) }))}</p>`;

    if (results.length === 0) {
        box.innerHTML = `${summary}<p class="browse-empty">${escapeHtml(t('browse.empty'))}</p>`;
        return;
    }

    box.innerHTML = summary + results.map(renderBrowseRow).join('');
}

function renderBrowseRow(r) {
    const detailsOpen = currentMode === 'advanced' ? ' open' : '';
    const fitChip = `browse-chip fit-${r.fit_level.toLowerCase()}`;
    const slowChip = r.slow ? ` <span class="browse-chip slow">${escapeHtml(t('rec.slowShort'))}</span>` : '';
    const detailChips = ((r.has_vision ? ` <span class="browse-chip cap-chip">${escapeHtml(t('filters.vision'))}</span>` : '') + (r.has_tools ? ` <span class="browse-chip cap-chip">${escapeHtml(t('filters.tools'))}</span>` : '') + (r.has_audio ? ` <span class="browse-chip cap-chip">${escapeHtml(t('details.audioShort'))}</span>` : '') + (r.has_tts ? ` <span class="browse-chip cap-chip">${escapeHtml(t('details.ttsShort'))}</span>` : ''));
    const comparing = compareSel.some(c => c.name === r.name);
    const compareBtn = comparing
        ? `<button class="compare-toggle is-on" type="button" aria-pressed="true" onclick="toggleCompareByName('${escapeAttr(r.name)}')">${ICONS.check} ${escapeHtml(t('browse.compare'))}</button>`
        : `<button class="compare-toggle" type="button" aria-pressed="false" aria-label="${escapeAttr(t('browse.compareAria'))}" onclick="toggleCompareByName('${escapeAttr(r.name)}')">${ICONS.compare} ${escapeHtml(t('browse.compare'))}</button>`;
    const action = r.installed
        ? `<button class="btn btn-secondary" type="button" onclick="testModel('${escapeAttr(r.name)}', '${escapeAttr(r.ollama_tag)}')">${ICONS.retry} ${escapeHtml(t('rec.testShort'))}</button>`
        : `<button class="btn btn-primary" type="button"${downloadActive ? ' disabled aria-disabled="true"' : ''} onclick="confirmDownload('${escapeAttr(r.name)}', '${escapeAttr(r.ollama_tag)}', ${r.disk_size_gb})">${ICONS.download} ${escapeHtml(t('rec.downloadShort'))}</button>`;

    return `
    <article class="browse-row">
        <div class="browse-row-main">
            <div class="browse-row-title">
                <span>${escapeHtml(humanName(r.name))}</span>
                <span class="browse-hf">${escapeHtml(r.name)}</span>
            </div>
            <div class="browse-row-desc">
                <span class="browse-chip ${fitChip}">${escapeHtml(humanFit(r.fit_level))}</span>
                <span class="browse-chip">${escapeHtml(r.parameter_count)}</span>
                <span class="browse-chip">${escapeHtml(humanSpeed(r.estimated_tps))} · ~${r.estimated_tps.toFixed(1)} tok/s</span>
                <span class="browse-chip">~${r.memory_required_gb.toFixed(1)} GB</span>
                ${slowChip}${detailChips}
            </div>
            ${currentMode === 'advanced' ? renderQuantPicker(r.quant_options) : ''}
            <details class="rec-details"${detailsOpen}>
                <summary>${escapeHtml(t('details.toggle'))}</summary>
                <div class="rec-details-grid">
                    <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.tag'))}</span><span class="rec-detail-value mono">${escapeHtml(r.ollama_tag)}</span></div>
                    <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.quant'))}</span><span class="rec-detail-value mono">${escapeHtml(r.quant)}</span></div>
                    <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.fit'))}</span><span class="rec-detail-value">${escapeHtml(r.fit_level)}</span></div>
                    <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.runMode'))}</span><span class="rec-detail-value">${escapeHtml(r.run_mode)}</span></div>
                    <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.disk'))}</span><span class="rec-detail-value">~${r.disk_size_gb.toFixed(1)} GB</span></div>
                    <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.useCase'))}</span><span class="rec-detail-value">${escapeHtml(r.use_case)}</span></div>
                    <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.caps'))}</span><span class="rec-detail-value">${escapeHtml(knownCaps(r))}</span></div>
                    ${r.release_date ? `<div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.release'))}</span><span class="rec-detail-value">${escapeHtml(r.release_date)}</span></div>` : ''}
                    ${(r.languages && r.languages.length) ? `<div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.languages'))}</span><span class="rec-detail-value">${escapeHtml(r.languages.join(', '))}</span></div>` : ''}
                    ${r.is_moe && r.active_parameters ? `<div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.activeParams'))}</span><span class="rec-detail-value">${escapeHtml(formatParams(r.active_parameters))}</span></div>` : ''}
                </div>
            </details>
        </div>
        <div class="browse-row-actions">
            ${compareBtn}
            ${action}
        </div>
    </article>`;
}

// Human-readable capability flags (known values only).
function knownCaps(r) {
    const caps = [];
    if (r.has_vision) caps.push(t('filters.vision'));
    if (r.has_tools) caps.push(t('filters.tools'));
    if (r.has_audio) caps.push(t('details.audioShort'));
    if (r.has_tts) caps.push(t('details.ttsShort'));
    return caps.length ? caps.join(', ') : '—';
}

function formatParams(n) {
    const b = (n || 0) / 1e9;
    return b >= 1 ? `${(b * 10).toFixed(0) / 10}B` : `${((n || 0) / 1e6).toFixed(1)}M`;
}

// ── Compare ───────────────────────────────────────────────────
let compareSel = [];

function toggleCompareByName(name) {
    const row = ((lastBrowseData && lastBrowseData.results) || []).find(r => r.name === name);
    if (!row) return;
    const idx = compareSel.findIndex(c => c.name === name);
    if (idx >= 0) {
        compareSel.splice(idx, 1);
    } else {
        if (compareSel.length >= 3) {
            showError(t('browse.compareMax'));
            return;
        }
        compareSel.push(row);
    }
    renderCompareBar();
    renderComparePanel();
    // Re-render rows from cache so every Compare button reflects the new state.
    if (lastBrowseData) renderBrowse(lastBrowseData, lastBrowseQuery);
}

function setupCompare() {
    document.getElementById('compareViewBtn').addEventListener('click', () => {
        const panel = document.getElementById('comparePanel');
        panel.hidden = !panel.hidden;
        if (!panel.hidden && compareSel.length >= 2) renderComparePanel();
    });
    document.getElementById('compareClearBtn').addEventListener('click', () => {
        compareSel = [];
        renderCompareBar();
        renderComparePanel();
        if (lastBrowseData) renderBrowse(lastBrowseData, lastBrowseQuery);
    });
}

function renderCompareBar() {
    const bar = document.getElementById('compareBar');
    if (compareSel.length === 0) {
        bar.hidden = true;
        return;
    }
    bar.hidden = false;
    document.getElementById('compareBarText').textContent = t('browse.compareBar');
    document.getElementById('compareCount').textContent = t('browse.compareCount', { n: compareSel.length });
    document.getElementById('compareViewBtn').textContent = t('browse.compareView');
    document.getElementById('compareClearBtn').textContent = t('browse.compareClear');
}

function renderComparePanel() {
    const panel = document.getElementById('comparePanel');
    if (compareSel.length < 2) {
        panel.hidden = true;
        return;
    }
    panel.hidden = false;

    const cols = compareSel.map(m => `
        <div class="cmp-col">
            <button class="cmp-col-remove" type="button" aria-label="${escapeAttr(t('browse.compareClose'))}" onclick="toggleCompareByName('${escapeAttr(m.name)}')">${ICONS.close}</button>
            <div class="cmp-col-name">${escapeHtml(humanName(m.name))}</div>
            <div class="cmp-col-tag mono">${escapeHtml(m.ollama_tag)}</div>
        </div>`).join('');

    const rows = [
        ['compareColSize', m => escapeHtml(m.parameter_count)],
        ['compareColSpeed', m => `${m.estimated_tps.toFixed(1)} tok/s · ${escapeHtml(humanSpeed(m.estimated_tps))}`],
        ['compareColMemory', m => `~${m.memory_required_gb.toFixed(1)} GB`],
        ['compareColFit', m => escapeHtml(humanFit(m.fit_level))],
        ['compareColQuant', m => escapeHtml(m.quant)],
        ['compareColContext', m => m.context_length ? `${(m.context_length / 1024).toFixed(0)}K` : '—'],
        ['compareColCaps', m => (m.capabilities || []).length ? escapeHtml((m.capabilities || []).join(', ')) : '—'],
        ['compareColLicense', m => escapeHtml(m.license || '—')],
        ['compareColArch', m => m.is_moe ? escapeHtml(t('details.moE')) : 'Dense'],
    ];

    const body = rows.map(([k, fn]) => `
        <div class="cmp-row">
            <div class="cmp-row-label">${escapeHtml(t(`browse.${k}`))}</div>
            ${compareSel.map(m => `<div class="cmp-cell">${fn(m)}</div>`).join('')}
        </div>`).join('');

    panel.innerHTML = `
        <div class="compare-panel-head">
            <h3>${escapeHtml(t('browse.compareView'))}</h3>
            <button class="btn btn-ghost btn-sm" type="button" onclick="document.getElementById('comparePanel').hidden = true">${escapeHtml(t('browse.compareClose'))}</button>
        </div>
        <div class="cmp-table">
            <div class="cmp-row cmp-row-head">
                <div class="cmp-row-label">${escapeHtml(t('browse.compareColModel'))}</div>
                ${cols}
            </div>
            ${body}
        </div>`;
}

// ── Quantisation picker ───────────────────────────────────────
function renderQuantPicker(options) {
    if (!options || options.length === 0) return '';
    const opts = options.map(o => {
        const state = o.selected ? ' selected' : '';
        const fits = o.fits
            ? `<span class="quant-opt-fits">${escapeHtml(t('quant.fits'))}</span>`
            : `<span class="quant-opt-nofit">${escapeHtml(t('quant.needs', { n: o.memory_gb.toFixed(1) }))}</span>`;
        return `
        <button class="quant-opt${state}" type="button" data-quant="${escapeAttr(o.quant)}" onclick="selectQuant(this, '${escapeAttr(o.quant)}')">
            <span class="quant-opt-name">${escapeHtml(o.quant)}</span>
            <span class="quant-opt-meta">~${o.memory_gb.toFixed(1)} GB · ${o.tps.toFixed(1)} tok/s</span>
            ${fits}
        </button>`;
    }).join('');
    return `
    <div class="quant-picker">
        <span class="quant-picker-label">${escapeHtml(t('quant.label'))}</span>
        <div class="quant-options">${opts}</div>
    </div>`;
}

function selectQuant(btn, quant) {
    const group = btn.closest('.quant-options');
    if (!group) return;
    group.querySelectorAll('.quant-opt').forEach(o => o.classList.remove('selected'));
    btn.classList.add('selected');
}

// ── API Calls ────────────────────────────────────────────────
async function apiGet(path) {
    const url = `${API}${path}`;
    const headers = {};
    if (sessionToken) headers['Authorization'] = `Bearer ${sessionToken}`;
    const res = await fetch(url, { headers });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
}

async function apiPost(path, body) {
    const url = `${API}${path}${sessionToken ? `?token=${encodeURIComponent(sessionToken)}` : ''}`;
    const headers = { 'Content-Type': 'application/json' };
    if (sessionToken) headers['Authorization'] = `Bearer ${sessionToken}`;
    const res = await fetch(url, { method: 'POST', headers, body: JSON.stringify(body) });
    if (!res.ok) {
        const err = await res.json().catch(() => ({ error: `HTTP ${res.status}` }));
        // Carry the HTTP status so callers can detect specific conditions
        // (e.g. 409 here means "another download already in progress")
        // without depending on the English message wording.
        const e = new Error(err.error || `HTTP ${res.status}`);
        e.status = res.status;
        throw e;
    }
    return res.json();
}

// Map known backend error strings to localised text (case-insensitive);
// unknown server messages keep their raw text as a fallback.
function localizeError(msg) {
    const m = String(msg || '');
    const lower = m.toLowerCase();
    if (lower.includes('session token')) return t('errors.token');
    if (lower.includes('already in progress')) return t('errors.downloadBusy');
    if (lower.includes('restricted to localhost')) return t('errors.localhost');
    if (lower.includes('ollama is not running')) return t('errors.ollamaDown');
    if (lower.includes('no download with id')) return t('errors.notFound');
    if (lower.includes('returned status')) return t('errors.modelStatus');
    if (lower.includes('invalid model tag')) return t('errors.invalidTag');
    if (lower.includes('not installed')) return t('errors.notInstalled');
    if (lower.includes('delete request failed')) return t('errors.deleteFailed');
    return m;
}

// ── System Info ──────────────────────────────────────────────
let lastSpecs = null;

async function loadSystemInfo() {
    try {
        const data = await apiGet('/system');
        lastSpecs = data;
        renderHardware(data);
    } catch (e) {
        const chip = document.querySelector('[data-field="cpu"]');
        if (chip) chip.textContent = t('overview.couldNotDetect');
    }
}

function renderHardwareFromCache() {
    if (lastSpecs) renderHardware(lastSpecs);
}

function setChip(field, value, hint, ok) {
    const el = document.querySelector(`.strip-chip[data-chip="${field}"]`);
    if (!el) return;
    const v = el.querySelector('[data-field]');
    const h = el.querySelector('.strip-hint');
    if (v) v.textContent = value;
    if (h) h.textContent = hint || '';
    if (ok !== undefined) el.dataset.ok = String(ok);
}

function renderHardware(specs) {
    const cpu = humanCpu(specs.cpu_name, specs.cpu_cores);
    const gpu = humanGpu(specs);
    setChip('cpu', cpu.value, cpu.hint);
    setChip('ram', `${Math.round(specs.total_ram_gb)} GB`, memoryMeaning(specs.total_ram_gb));
    setChip('gpu', gpu.value, gpu.hint, specs.has_gpu ? undefined : false);

    const raw = document.getElementById('rawSpecs');
    if (raw) {
        raw.dataset.hasSpecs = '1';
        const gpuPart = specs.has_gpu
            ? `${specs.gpu_name} (${specs.gpu_vram_gb || '?'} GB VRAM) · ${t('details.backend')} ${specs.backend}`
            : `${t('overview.gpuNone')} · CPU`;
        raw.textContent =
            `${specs.cpu_name} · ${t('overview.memory')} ${specs.total_ram_gb} GB ${t('details.total')} / ${specs.available_ram_gb} GB ${t('details.available')} · ` +
            gpuPart +
            ` · ${specs.os}`;
        raw.hidden = currentMode !== 'advanced';
    }
}

// ── Ollama Status ────────────────────────────────────────────
async function loadOllamaStatus() {
    try {
        const data = await apiGet('/ollama/status');
        renderOllamaStatus(data);
    } catch (e) {
        setChip('ollama', 'Unknown', t('nav.ollamaUnknown'), false);
        setSideOllama(false, t('nav.ollamaUnknown'));
    }
}

function setSideOllama(ok, text) {
    const el = document.getElementById('sideOllama');
    if (!el) return;
    el.dataset.ok = String(ok);
    el.querySelector('.side-status-text').textContent = text;
}

function renderOllamaStatus(data) {
    if (data.available) {
        setChip('ollama', t('overview.ready'), t('overview.canDownload'), true);
        setSideOllama(true, plural(data.model_count, 'nav.ollamaReady1', 'nav.ollamaReadyN'));
    } else {
        setChip('ollama', t('overview.notRunning'), t('overview.installHint'), false);
        setSideOllama(false, t('nav.ollamaDown'));
    }

    const count = document.getElementById('installedCount');
    if (count) {
        const n = data.model_count || 0;
        count.hidden = n === 0;
        count.textContent = String(n);
    }

    if (currentView === 'installed') loadInstalled();
}

// ── Installed models ─────────────────────────────────────────
async function loadInstalled() {
    const list = document.getElementById('installedList');
    if (!list) return;

    let status;
    try {
        status = await apiGet('/ollama/status');
    } catch (e) {
        list.innerHTML = `<p class="installed-empty">${escapeHtml(t('installed.loadError', { msg: e.message }))}</p>`;
        return;
    }

    if (!status.available) {
        list.innerHTML =
            `<p class="installed-empty">${escapeHtml(t('installed.emptyNoOllama'))}</p>`;
        return;
    }

    let sizes = {};
    try {
        const det = await apiGet('/ollama/installed');
        (det.models || []).forEach(m => { sizes[m.name] = m.size_bytes; });
    } catch (e) { /* sizes are a bonus */ }

    const tags = (status.installed_models || []).filter(m => m.includes(':'));
    if (tags.length === 0) {
        list.innerHTML = `<p class="installed-empty">${escapeHtml(t('installed.empty'))}</p>`;
        return;
    }

    list.innerHTML = tags.map(tag => {
        const size = formatGb(sizes[tag]);
        const pretty = humanName(tag);
        return `
        <article class="installed-item">
            <div class="installed-check" aria-hidden="true">${ICONS.check}</div>
            <div class="installed-body">
                <div class="installed-name">${escapeHtml(pretty)}</div>
                <div class="installed-tag mono">${escapeHtml(tag)}</div>
                <div class="installed-meta">
                    <span class="installed-state">${escapeHtml(t('installed.ready'))}</span>
                    <span class="installed-size">${escapeHtml(t('installed.diskSize'))}: ${size}</span>
                </div>
            </div>
            <div class="installed-actions">
                <button class="btn btn-secondary btn-sm" type="button" onclick="testModel('${escapeAttr(pretty)}', '${escapeAttr(tag)}')">${ICONS.retry} ${escapeHtml(t('rec.testShort'))}</button>
                <button class="btn btn-danger btn-sm" type="button" onclick="confirmRemove('${escapeAttr(pretty)}', '${escapeAttr(tag)}')">${ICONS.trash} ${escapeHtml(t('remove.action'))}</button>
            </div>
        </article>`;
    }).join('');
}

// ── Model removal ────────────────────────────────────────────
// Removal is a destructive, mutating action: it goes through the same
// loopback + session-token protected backend endpoint as downloads, and
// the confirmation dialog always shows the EXACT Ollama tag before
// anything is sent.
function confirmRemove(modelName, tag) {
    const modal = document.getElementById('downloadModal');
    const title = document.getElementById('modalTitle');
    const content = document.getElementById('modalContent');
    const confirmBtn = document.getElementById('modalConfirm');
    const cancelBtn = document.getElementById('modalCancel');

    title.textContent = t('remove.modalTitle');
    confirmBtn.textContent = t('remove.confirm');
    confirmBtn.classList.add('btn-danger');
    confirmBtn.classList.remove('btn-primary');
    confirmBtn.disabled = false;
    cancelBtn.disabled = false;

    content.innerHTML = `
        <div class="modal-detail"><strong>${escapeHtml(t('remove.model'))}:</strong> ${escapeHtml(humanName(modelName))}</div>
        <div class="modal-detail"><strong>${escapeHtml(t('remove.tag'))}:</strong> <code>${escapeHtml(tag)}</code></div>
        <div class="modal-warning">
            ${escapeHtml(t('remove.warning'))}
        </div>
        <div class="modal-warning modal-warning-danger">
            ${escapeHtml(t('remove.safeaiWarning'))}
        </div>
    `;

    modal.hidden = false;

    const close = () => {
        modal.hidden = true;
        document.removeEventListener('keydown', onKey);
    };
    const onKey = (e) => { if (e.key === 'Escape') close(); };
    document.addEventListener('keydown', onKey);

    confirmBtn.onclick = () => removeModel(modelName, tag);
    cancelBtn.onclick = close;
}

async function removeModel(modelName, tag) {
    const modal = document.getElementById('downloadModal');
    const content = document.getElementById('modalContent');
    const confirmBtn = document.getElementById('modalConfirm');
    const cancelBtn = document.getElementById('modalCancel');

    // Pending state: the dialog stays open, actions are locked, and the UI
    // explains the model is being removed.
    confirmBtn.disabled = true;
    cancelBtn.disabled = true;
    content.innerHTML = `
        <div class="spinner" role="status" aria-label="${escapeAttr(t('remove.runningAria'))}"></div>
        <p class="installed-empty">${escapeHtml(t('remove.running'))}</p>
    `;

    try {
        // Mutating request: same session-token gate as downloads. The
        // backend talks to Ollama's `DELETE /api/delete` internally.
        await apiPost(`/models/${encodeURIComponent(tag)}/delete`, {});
        modal.hidden = true;
        showSuccess(t('remove.removed', { tag }));
        // Refreshes the sidebar count and — when the Installed view is
        // active — the model list itself.
        await loadOllamaStatus();
    } catch (e) {
        modal.hidden = true;
        showError(t('remove.error', { msg: localizeError(e.message) }));
        // Re-sync from Ollama's truth so a failed request can never leave
        // the list in a stale state.
        await loadOllamaStatus();
    }
}

// ── Performance ──────────────────────────────────────────────
let benchPoll = null;
let pollBenchmarkTimer = null;
let lastBenchHistory = null;

async function loadBenchModelSelect() {
    const sel = document.getElementById('benchModelSelect');
    if (!sel) return;
    let tags = [];
    try {
        const status = await apiGet('/ollama/status');
        tags = (status.installed_models || []).filter(m => m.includes(':'));
    } catch (e) { /* unavailable */ }
    sel.innerHTML = tags.length
        ? tags.map(t => `<option value="${escapeAttr(t)}">${escapeHtml(humanName(t))} — <span class="mono">${escapeHtml(t)}</span></option>`).join('')
        : `<option value="">${escapeHtml(t('performance.noModels'))}</option>`;
    document.getElementById('benchRunBtn').disabled = tags.length === 0;
}

function setupPerformance() {
    document.getElementById('benchRunBtn').addEventListener('click', runBenchmark);
    document.getElementById('benchModelSelect').addEventListener('change', () => {
        document.getElementById('benchResult').hidden = true;
    });
}

async function runBenchmark() {
    const model = document.getElementById('benchModelSelect').value;
    if (!model) return;
    const progress = document.getElementById('benchProgress');
    const resultBox = document.getElementById('benchResult');
    const fill = document.getElementById('benchProgressFill');
    const bar = document.getElementById('benchProgressBar');
    const text = document.getElementById('benchProgressText');

    resultBox.hidden = true;
    progress.hidden = false;
    fill.style.width = '0%';
    bar.setAttribute('aria-valuenow', '0');
    document.getElementById('benchRunBtn').disabled = true;

    try {
        const job = await apiPost('/benchmarks', { model });
        text.textContent = t('performance.warmup');
        pollBenchmark(job.id);
    } catch (e) {
        progress.hidden = true;
        document.getElementById('benchRunBtn').disabled = false;
        showError(t('performance.startError', { msg: localizeError(e.message) }));
    }
}

function pollBenchmark(id) {
    if (pollBenchmarkTimer) clearInterval(pollBenchmarkTimer);
    pollBenchmarkTimer = setInterval(async () => {
        try {
            const data = await apiGet(`/benchmarks/${id}`);
            const total = data.total || 1;
            const pct = Math.min(100, Math.round(((data.done || 0) / total) * 100));
            document.getElementById('benchProgressFill').style.width = `${pct}%`;
            document.getElementById('benchProgressBar').setAttribute('aria-valuenow', String(pct));
            document.getElementById('benchProgressText').textContent =
                t('performance.running', { n: data.done || 0, total });
            if (data.status === 'done') {
                clearInterval(pollBenchmarkTimer);
                pollBenchmarkTimer = null;
                finishBenchmark(data);
            } else if (data.status === 'error') {
                clearInterval(pollBenchmarkTimer);
                pollBenchmarkTimer = null;
                failBenchmark(data.error || 'unknown');
            }
        } catch (e) { /* keep polling */ }
    }, 800);
}

function finishBenchmark(data) {
    document.getElementById('benchProgress').hidden = true;
    document.getElementById('benchRunBtn').disabled = false;
    const s = data.summary || {};
    const measured = s.avg_tps != null ? s.avg_tps.toFixed(1) : '—';
    const ttft = s.avg_ttft_ms != null ? (s.avg_ttft_ms / 1000).toFixed(1) : null;
    const estimate = benchEstimateFor(data.model);
    const estimateLine = estimate != null
        ? `<div class="perf-estimate">${escapeHtml(t('performance.estimated'))} <strong>~${Number(estimate).toFixed(0)}</strong> ${escapeHtml(t('performance.tpsUnit'))}</div>`
        : `<div class="perf-estimate muted">${escapeHtml(t('performance.noEstimate'))}</div>`;
    document.getElementById('benchResult').hidden = false;
    document.getElementById('benchResult').innerHTML = `
        <div class="result-success bench-banner">
            <div class="bench-measured-label">${escapeHtml(t('performance.measuredLabel'))}</div>
            <div class="bench-hero">
                <div class="bench-hero-item">
                    <span class="bench-hero-num">${measured}</span>
                    <span class="bench-hero-unit">${escapeHtml(t('performance.tpsPer'))}</span>
                </div>
                <div class="bench-hero-item">
                    <span class="bench-hero-num">${ttft != null ? ttft : '—'}</span>
                    <span class="bench-hero-unit">${escapeHtml(t('performance.sec'))}</span>
                </div>
            </div>
            <p class="bench-hero-model mono">${escapeHtml(data.model)}</p>
            ${estimateLine}
        </div>
        ${renderBenchAdvanced(data)}
    `;
    renderBenchHistory();
}

function renderBenchAdvanced(data) {
    if (currentMode !== 'advanced' || !data.summary) return '';
    const s = data.summary;
    const runs = (data.runs || []);
    return `
    <details class="rec-details">
        <summary>${escapeHtml(t('performance.advancedTitle'))}</summary>
        <div class="rec-details-grid">
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('performance.runsLabel'))}</span><span class="rec-detail-value">${s.num_runs}</span></div>
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('performance.minMax'))}</span><span class="rec-detail-value">${Number(s.min_tps).toFixed(1)} – ${Number(s.max_tps).toFixed(1)} tok/s</span></div>
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('performance.avgTotal'))}</span><span class="rec-detail-value">${Number(s.avg_total_ms).toFixed(0)} ms</span></div>
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('performance.avgOutput'))}</span><span class="rec-detail-value">${Number(s.avg_output_tokens).toFixed(0)}</span></div>
        </div>
    </details>`;
}

function failBenchmark(msg) {
    document.getElementById('benchProgress').hidden = true;
    document.getElementById('benchRunBtn').disabled = false;
    showError(t('performance.failed', { msg: localizeError(msg) }));
}

async function renderBenchHistory() {
    const box = document.getElementById('benchHistory');
    if (!box) return;
    let data;
    try {
        data = await apiGet('/benchmarks/history');
    } catch (e) {
        box.innerHTML = '';
        return;
    }
    lastBenchHistory = data;
    const items = data.measurements || [];
    if (items.length === 0) {
        box.innerHTML = `<p class="installed-empty">${escapeHtml(t('performance.noHistory'))}</p>`;
        return;
    }
    box.innerHTML = items.map(m => `
        <article class="bench-history-item">
            <div class="bench-history-main">
                <div class="bench-history-name mono">${escapeHtml(m.model)}</div>
                <div class="bench-history-meta">
                    <span>${escapeHtml(t('performance.measuredOn'))}: ${formatDate(m.measured_at_unix)}</span>
                    <span>${escapeHtml(t('performance.runsShort'))}: ${m.num_runs}</span>
                </div>
            </div>
            <div class="bench-history-vals">
                <span class="bench-history-num">${Number(m.avg_tps).toFixed(1)} tok/s</span>
                ${m.avg_ttft_ms != null ? `<span>${escapeHtml(t('performance.responseStart'))}: ${(Number(m.avg_ttft_ms) / 1000).toFixed(1)} ${escapeHtml(t('performance.secShort'))}</span>` : ''}
                ${m.estimate_tps != null ? `<span class="muted">${escapeHtml(t('performance.estimated'))}: ~${Number(m.estimate_tps).toFixed(0)}</span>` : ''}
            </div>
        </article>`).join('');
}

function formatDate(unixSec) {
    try {
        return new Date(unixSec * 1000).toLocaleString();
    } catch (e) {
        return String(Math.round(unixSec));
    }
}

// Estimated speed for an installed tag: pulled from the latest persisted
// measurement when available, otherwise shown honestly as unknown.
function benchEstimateFor(tag) {
    if (!lastBenchHistory) return null;
    const row = (lastBenchHistory.measurements || []).find(m => m.model === tag);
    return row && row.estimate_tps != null ? row.estimate_tps : null;
}

// ── Hardware Planner ─────────────────────────────────────────
let plannerTimer = null;
let lastPlanData = null;
let plannerPicked = null;   // currently selected model (for the Selected card)
let lastPlannerHits = [];   // full result set for the Show-more expansion
const PLANNER_PAGE = 7;

function setupPlanner() {
    const input = document.getElementById('plannerSearch');
    input.addEventListener('input', () => {
        clearTimeout(plannerTimer);
        plannerTimer = setTimeout(() => renderPlannerSuggestions(input.value), 250);
    });
    input.addEventListener('keydown', e => {
        if (e.key === 'Enter') {
            const first = document.querySelector('.planner-suggestion');
            if (first) first.click();
        }
    });
    document.addEventListener('click', (e) => {
        const wrap = document.querySelector('.planner-search-wrap');
        if (wrap && !wrap.contains(e.target)) {
            document.getElementById('plannerSuggestions').hidden = true;
        }
    });
    document.getElementById('plannerQuant').addEventListener('change', onPlanParamsChange);
    document.getElementById('plannerContext').addEventListener('change', onPlanParamsChange);
    document.getElementById('plannerTargetTps').addEventListener('change', onPlanParamsChange);
}

function onPlanParamsChange() {
    if (lastPlanData) loadPlanFor(lastPlanData.requestModel);
}

async function renderPlannerSuggestions(q) {
    const box = document.getElementById('plannerSuggestions');
    if (!box) return;
    if (!q || q.trim().length < 2) {
        box.hidden = true;
        return;
    }
    try {
        const data = await apiGet(`/plan/search?q=${encodeURIComponent(q.trim())}&limit=24`);
        lastPlannerHits = data.results || [];
        if (lastPlannerHits.length === 0) {
            box.hidden = true;
            return;
        }
        const rest = lastPlannerHits.length - PLANNER_PAGE;
        box.innerHTML = lastPlannerHits.slice(0, PLANNER_PAGE).map(plannerCardHtml).join('')
            + (rest > 0
                ? `<button class="planner-more" type="button" onclick="expandPlannerSuggestions()">${escapeHtml(t('planner.showMore'))} (${rest})</button>`
                : '');
        box.hidden = false;
    } catch (e) {
        box.hidden = true;
    }
}

// Compact “choice” card: clean human name first, size, then useful
// characteristics (Advanced only) and the technical id as muted secondary.
function plannerCardHtml(m) {
    const meta = [];
    if (m.parameter_count) meta.push(m.parameter_count);
    if (currentMode === 'advanced') {
        if (m.context_length) meta.push(`${Math.round(m.context_length / 1024)}K ${t('planner.ctxWord')}`);
        if (m.is_moe) meta.push(t('details.moE'));
        if (m.quant) meta.push(m.quant);
    }
    return `
    <button class="planner-suggestion" type="button" onclick="pickPlanModel('${escapeAttr(m.name)}')">
        <span class="ps-name">${escapeHtml(humanName(m.name))}</span>
        <span class="ps-meta">${escapeHtml(meta.join(' · ') || '')}</span>
        <span class="ps-id mono">${escapeHtml(m.name)}</span>
    </button>`;
}

function expandPlannerSuggestions() {
    const box = document.getElementById('plannerSuggestions');
    box.innerHTML = lastPlannerHits.map(plannerCardHtml).join('');
}

function pickPlanModel(name) {
    const hit = lastPlannerHits.find(h => h.name === name) || { name, parameter_count: '' };
    plannerPicked = hit;
    document.getElementById('plannerSearch').value = name;
    document.getElementById('plannerSuggestions').hidden = true;
    document.getElementById('plannerEmpty').hidden = true;
    renderSelectedModelCard(hit);
    loadPlan(name);
}

function renderSelectedModelCard(m) {
    const card = document.getElementById('selectedModelCard');
    if (!card) return;
    const meta = [];
    if (m.parameter_count) meta.push(m.parameter_count);
    if (currentMode === 'advanced') {
        if (m.context_length) meta.push(`${Math.round(m.context_length / 1024)}K ${t('planner.ctxWord')}`);
        if (m.is_moe) meta.push(t('details.moE'));
    }
    card.hidden = false;
    card.innerHTML = `
        <div class="smc-info">
            <span class="smc-label">${escapeHtml(t('planner.selectedTitle'))}</span>
            <span class="smc-name">${escapeHtml(humanName(m.name))}</span>
            <span class="smc-meta">${meta.length ? escapeHtml(meta.join(' · ')) : ''}</span>
        </div>
        <button class="btn btn-ghost btn-sm" type="button" onclick="changePlanModel()">${escapeHtml(t('planner.changeModel'))}</button>`;
}

function changePlanModel() {
    plannerPicked = null;
    document.getElementById('plannerSearch').value = '';
    document.getElementById('selectedModelCard').hidden = true;
    document.getElementById('plannerEmpty').hidden = false;
    document.getElementById('planResult').hidden = true;
    document.getElementById('plannerSearch').focus();
}

async function loadPlan(name) {
    const box = document.getElementById('planResult');
    box.hidden = false;
    box.innerHTML = `<div class="spinner" role="status" aria-label="${escapeAttr(t('planner.loadingAria'))}"></div>`;
    const params = new URLSearchParams({ model: name });
    const quant = document.getElementById('plannerQuant').value;
    if (quant) params.set('quant', quant);
    const ctx = document.getElementById('plannerContext').value;
    if (ctx) params.set('context', ctx);
    const tps = document.getElementById('plannerTargetTps').value;
    if (tps) params.set('target_tps', tps);
    try {
        const data = await apiGet(`/plan?${params.toString()}`);
        lastPlanData = { requestModel: name, plan: data.plan, computer: data.computer };
        renderPlan(lastPlanData);
    } catch (e) {
        box.innerHTML = `<div class="result-error"><p>${escapeHtml(t('planner.error', { msg: localizeError(e.message) }))}</p></div>`;
    }
}

function renderPlan(data) {
    const box = document.getElementById('planResult');
    if (!box) return;
    const p = data.plan;
    lastPlanData = data;
    const current = p.current || {};
    const fits = current.fit_level !== 'TooTight';
    const min = p.minimum || {};
    const rec = p.recommended || {};
    const preferred = (p.run_paths || []).find(r => r.feasible);
    const pathLabel = preferred ? preferred.path : (p.run_paths && p.run_paths[0] ? p.run_paths[0].path : 'cpu_only');

    const runsRow = fits
        ? `<div class="plan-verdict ok">${escapeHtml(t('planner.runsOnThis'))}</div>`
        : `<div class="plan-verdict no">${escapeHtml(t('planner.missingHardware'))}</div>`;
    const gap = !fits ? gapMessage(p, data.computer) : '';

    box.innerHTML = `
        <div class="plan-card">
            <div class="plan-head">
                <div class="plan-name">${escapeHtml(humanName(p.model_name || ''))}</div>
                <div class="mono muted">${escapeHtml(p.model_name)}</div>
                <div class="plan-quant mono">${escapeHtml(p.quantization || '')}</div>
            </div>
            ${runsRow}
            <div class="plan-grid">
                <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('planner.minRam'))}</span><span class="rec-detail-value">${fmtGb(min.ram_gb)}</span></div>
                <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('planner.recRam'))}</span><span class="rec-detail-value">${fmtGb(rec.ram_gb)}</span></div>
                ${min.vram_gb != null ? `<div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('planner.vram'))}</span><span class="rec-detail-value">${fmtGb(min.vram_gb)}</span></div>` : ''}
                <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('planner.path'))}</span><span class="rec-detail-value">${escapeHtml(humanPath(pathLabel))}</span></div>
                <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('planner.expectedPerf'))}</span><span class="rec-detail-value">${current.estimated_tps != null ? `${Number(current.estimated_tps).toFixed(0)} tok/s` : '—'}</span></div>
            </div>
            ${gap}
            ${currentMode === 'advanced' ? renderPlanAdvanced(p) : ''}
        </div>`;
}

function gapMessage(p, computer) {
    const min = p.minimum || {};
    const needGb = min.vram_gb != null ? min.vram_gb : (min.ram_gb || 0);
    const haveGb = computer && computer.vram_gb != null
        ? computer.vram_gb
        : (computer ? computer.ram_gb || 0 : 0);
    // Plain-language: what is missing on THIS computer (real specs).
    return `<p class="plan-gap">${escapeHtml(t('planner.gap', { need: fmtGb(needGb), have: fmtGb(haveGb) }))}</p>`;
}

function humanPath(p) {
    switch ((p || '').toLowerCase()) {
        case 'gpu': return t('planner.pathGpu');
        case 'cpu_offload': return t('planner.pathOffload');
        default: return t('planner.pathCpu');
    }
}

function fmtGb(v) {
    return v == null ? '—' : `${Number(v).toFixed(1)} GB`;
}

function renderPlanAdvanced(p) {
    const paths = (p.run_paths || []).map(rp => `
        <div class="plan-path ${rp.feasible ? 'ok' : 'no'}">
            <span class="plan-path-name">${escapeHtml(humanPath(rp.path))}</span>
            <span class="muted">${rp.feasible ? escapeHtml(t('planner.feasible')) : escapeHtml(t('planner.notFeasible'))}${rp.estimated_tps != null ? ` · ~${Number(rp.estimated_tps).toFixed(0)} tok/s` : ''}</span>
        </div>`).join('');
    const upgrades = (p.upgrade_deltas || []).map(d => `
        <div class="plan-upgrade">
            <span class="plan-upgrade-res">${escapeHtml(d.resource)} ${d.add_gb != null ? `+${Number(d.add_gb).toFixed(0)} GB` : d.add_cores != null ? `+${d.add_cores} cores` : ''}</span>
            <span class="muted">${escapeHtml(d.description || '')}</span>
        </div>`).join('');
    return `
    <details class="rec-details"${currentMode === 'advanced' ? ' open' : ''}>
        <summary>${escapeHtml(t('planner.advancedTitle'))}</summary>
        <div class="planner-advanced-body">
            <h4>${escapeHtml(t('planner.pathsTitle'))}</h4>
            ${paths || '<p class="muted">—</p>'}
            ${upgrades ? `<h4>${escapeHtml(t('planner.upgradesTitle'))}</h4>${upgrades}` : ''}
        </div>
    </details>`;
}

// ── Recommendations ──────────────────────────────────────────
function useCaseLabel(uc) {
    return t(`find.${uc}`);
}

function recDescription(rec, uc) {
    if (I18N.lang === 'en') return rec.description;
    const ucName = useCaseLabel(uc).toLowerCase();
    switch (rec.label_key) {
        case 'recommended': {
            const how = rec.fit_level === 'Perfect' ? t('rec.descRecommendedHowPerfect')
                : rec.fit_level === 'Good' ? t('rec.descRecommendedHowGood')
                : t('rec.descRecommendedHowOther');
            return t('rec.descRecommended', { uc: ucName, how });
        }
        case 'faster':
            return t('rec.descFaster', { gb: rec.memory_required_gb.toFixed(1) });
        case 'better_quality':
            return t('rec.descBetter', { uc: ucName });
        default:
            return t('rec.descAlt');
    }
}

function recLabel(rec) {
    const key = { recommended: 'recommended', faster: 'faster', better_quality: 'betterQuality', alternative: 'alternative' }[rec.label_key] || 'alternative';
    return t(`rec.${key}`);
}

async function fetchRecommendations() {
    const area = document.getElementById('recommendationsArea');
    const grid = document.getElementById('recommendationsGrid');
    const sub = document.getElementById('recSub');
    area.hidden = false;
    grid.innerHTML = `<div class="spinner" role="status" aria-label="${escapeAttr(t('rec.loadingAria'))}"></div>`;
    sub.textContent = t('rec.subtitle', { uc: useCaseLabel(currentUseCase) });

    try {
        const data = await apiGet(`/recommendations?${recommendationParams().toString()}`);
        renderRecommendations(data);
    } catch (e) {
        grid.innerHTML = `<p class="installed-empty">${escapeHtml(t('rec.loadError', { msg: localizeError(e.message) }))}</p>`;
    }
}

function renderRecommendations(data) {
    const grid = document.getElementById('recommendationsGrid');
    const recs = data.recommendations || [];

    if (recs.length === 0) {
        grid.innerHTML =
            `<p class="installed-empty" style="text-align:center;max-width:46ch;margin:0 auto">
                ${escapeHtml(t('rec.noResults'))}
             </p>`;
        return;
    }

    const hero = recs[0];
    const alts = recs.slice(1);
    grid.innerHTML = renderHero(hero) + renderAlts(alts);
}

function renderHero(rec) {
    const installed = rec.installed;
    const detailsOpen = currentMode === 'advanced' ? ' open' : '';
    const slowPill = rec.slow ? `<span class="pill pill-slow">${escapeHtml(t('rec.slow'))}</span>` : '';
    return `
    <article class="rec-hero">
        <div class="rec-hero-top">
            <span class="pill pill-primary">${escapeHtml(t('rec.recommended'))}</span>
            ${installed ? `<span class="pill pill-installed">${escapeHtml(t('rec.installed'))}</span>` : ''}
            ${slowPill}
        </div>
        <h3 class="rec-hero-name">${escapeHtml(humanName(rec.name))}</h3>
        <p class="rec-hero-why">${escapeHtml(recDescription(rec, currentUseCase))}</p>
        <div class="rec-hero-indicators">
            <div class="indicator"><span class="ind-label">${escapeHtml(t('rec.perfLabel'))}</span><span class="ind-value">${escapeHtml(humanSpeed(rec.estimated_tps))}</span></div>
            <div class="indicator"><span class="ind-label">${escapeHtml(t('rec.qualityLabel'))}</span><span class="ind-value">${escapeHtml(rec.parameter_count)}</span></div>
            <div class="indicator"><span class="ind-label">${escapeHtml(t('rec.fitLabel'))}</span><span class="ind-value">${escapeHtml(humanFit(rec.fit_level))}</span></div>
        </div>
        ${currentMode === 'advanced' ? renderQuantPicker(rec.quant_options) : ''}
        <div class="rec-hero-actions">
            ${installed
                ? `<button class="btn btn-secondary" type="button" onclick="testModel('${escapeAttr(rec.name)}', '${escapeAttr(rec.ollama_tag)}')">${ICONS.retry} ${escapeHtml(t('rec.test'))}</button>`
                : `<button class="btn btn-primary btn-lg" type="button"${downloadActive ? ' disabled aria-disabled="true"' : ''} onclick="confirmDownload('${escapeAttr(rec.name)}', '${escapeAttr(rec.ollama_tag)}', ${rec.disk_size_gb})">${ICONS.download} ${escapeHtml(t('rec.download'))}</button>`}
        </div>
        ${renderDetails(rec, detailsOpen)}
    </article>`;
}

function renderAlts(alts) {
    if (alts.length === 0) return '';
    return `<div class="rec-alts">${alts.map(renderAlt).join('')}</div>`;
}

function renderAlt(rec) {
    const installed = rec.installed;
    const detailsOpen = currentMode === 'advanced' ? ' open' : '';
    const slowNote = rec.slow ? `<span class="browse-chip slow">${escapeHtml(t('rec.slowShort'))}</span>` : '';
    return `
    <article class="rec-alt">
        <div class="rec-alt-head">
            <span class="pill pill-alt">${escapeHtml(recLabel(rec))}</span>
            ${installed ? `<span class="rec-installed-note">${escapeHtml(t('rec.installedShort'))}</span>` : ''}
            ${slowNote}
        </div>
        <h4 class="rec-alt-name">${escapeHtml(humanName(rec.name))}</h4>
        <p class="rec-alt-why">${escapeHtml(recDescription(rec, currentUseCase))}</p>
        <p class="rec-alt-meta">~${rec.memory_required_gb.toFixed(1)} GB · ${escapeHtml(humanSpeed(rec.estimated_tps))}</p>
        ${currentMode === 'advanced' ? renderQuantPicker(rec.quant_options) : ''}
        <div class="rec-alt-actions">
            ${installed
                ? `<button class="btn btn-secondary btn-block" type="button" onclick="testModel('${escapeAttr(rec.name)}', '${escapeAttr(rec.ollama_tag)}')">${ICONS.retry} ${escapeHtml(t('rec.test'))}</button>`
                : `<button class="btn btn-secondary btn-block" type="button"${downloadActive ? ' disabled aria-disabled="true"' : ''} onclick="confirmDownload('${escapeAttr(rec.name)}', '${escapeAttr(rec.ollama_tag)}', ${rec.disk_size_gb})">${ICONS.download} ${escapeHtml(t('rec.downloadShort'))}</button>`}
        </div>
        ${renderDetails(rec, detailsOpen)}
    </article>`;
}

function renderDetails(rec, openAttr) {
    const caps = (rec.capabilities || []).join(', ');
    return `
    <details class="rec-details"${openAttr}>
        <summary>${escapeHtml(t('details.toggle'))}</summary>
        <div class="rec-details-grid">
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.model'))}</span><span class="rec-detail-value">${escapeHtml(rec.name)}</span></div>
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.tag'))}</span><span class="rec-detail-value mono">${escapeHtml(rec.ollama_tag)}</span></div>
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.quant'))}</span><span class="rec-detail-value mono">${escapeHtml(rec.quant)}</span></div>
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.speed'))}</span><span class="rec-detail-value">${rec.estimated_tps.toFixed(1)} tokens/s</span></div>
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.memory'))}</span><span class="rec-detail-value">~${rec.memory_required_gb.toFixed(1)} GB</span></div>
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.disk'))}</span><span class="rec-detail-value">~${rec.disk_size_gb.toFixed(1)} GB</span></div>
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.fit'))}</span><span class="rec-detail-value">${escapeHtml(rec.fit_level)}</span></div>
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.context'))}</span><span class="rec-detail-value">${rec.context_length ? `${(rec.context_length / 1024).toFixed(0)}K` : '—'}</span></div>
            <div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.license'))}</span><span class="rec-detail-value">${escapeHtml(rec.license || '—')}</span></div>
            ${rec.is_moe ? `<div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.moE'))}</span><span class="rec-detail-value">Yes</span></div>` : ''}
            ${caps ? `<div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.caps'))}</span><span class="rec-detail-value">${escapeHtml(caps)}</span></div>` : ''}
            ${rec.release_date ? `<div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.release'))}</span><span class="rec-detail-value">${escapeHtml(rec.release_date)}</span></div>` : ''}
            ${(rec.languages && rec.languages.length) ? `<div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.languages'))}</span><span class="rec-detail-value">${escapeHtml(rec.languages.join(', '))}</span></div>` : ''}
            ${rec.is_moe && rec.active_parameters ? `<div class="rec-detail-item"><span class="rec-detail-label">${escapeHtml(t('details.activeParams'))}</span><span class="rec-detail-value">${escapeHtml(formatParams(rec.active_parameters))}</span></div>` : ''}
        </div>
    </details>`;
}

// ── Download Flow ────────────────────────────────────────────
function confirmDownload(modelName, ollamaTag, diskSize) {
    const modal = document.getElementById('downloadModal');
    const content = document.getElementById('modalContent');

    // The removal flow reuses this dialog; always restore its download
    // identity (the static data-i18n pass also re-applies on language
    // switch).
    document.getElementById('modalTitle').textContent = t('download.modalTitle');
    const confirmBtn = document.getElementById('modalConfirm');
    confirmBtn.textContent = t('download.confirm');
    confirmBtn.classList.add('btn-primary');
    confirmBtn.classList.remove('btn-danger');
    confirmBtn.disabled = false;
    document.getElementById('modalCancel').disabled = false;

    content.innerHTML = `
        <div class="modal-detail"><strong>${escapeHtml(t('download.model'))}:</strong> ${escapeHtml(humanName(modelName))}</div>
        <div class="modal-detail"><strong>${escapeHtml(t('download.tag'))}:</strong> <code>${escapeHtml(ollamaTag)}</code></div>
        <div class="modal-detail"><strong>${escapeHtml(t('download.size'))}:</strong> ~${diskSize} GB</div>
        <div class="modal-warning">
            ${escapeHtml(t('download.warning'))}
        </div>
    `;

    modal.hidden = false;

    document.getElementById('modalConfirm').onclick = () => {
        modal.hidden = true;
        startDownload(modelName, ollamaTag);
    };
    document.getElementById('modalCancel').onclick = () => {
        modal.hidden = true;
    };
    // Esc closes the modal.
    const onKey = (e) => {
        if (e.key === 'Escape') {
            modal.hidden = true;
            document.removeEventListener('keydown', onKey);
        }
    };
    document.addEventListener('keydown', onKey);
}

function setupProgressClose() {
    const close = document.getElementById('progressClose');
    if (close) close.addEventListener('click', () => {
        document.getElementById('progressPane').hidden = true;
    });
}

function localizeProgress(msg) {
    const m = String(msg || '').toLowerCase();
    if (m.includes('pulling manifest')) return t('progress.pullingManifest');
    if (m.includes('downloading')) return t('progress.downloading');
    if (m.includes('verifying')) return t('progress.verifying');
    if (m.includes('writing manifest')) return t('progress.writing');
    if (m === 'success') return t('progress.success');
    if (m === 'starting' || m === '') return t('progress.starting');
    return String(msg || '');
}

// While one download runs, every other Download button is greyed out with
// a “wait” tooltip so a second download cannot be started by accident.
function setDownloadButtonsDisabled(disabled) {
    document.querySelectorAll('[onclick*="confirmDownload"]').forEach(b => {
        b.disabled = disabled;
        b.setAttribute('aria-disabled', String(disabled));
        if (disabled) b.title = t('download.busyTitle');
        else b.removeAttribute('title');
    });
}

async function startDownload(modelName, ollamaTag) {
    const pane = document.getElementById('progressPane');
    pane.hidden = false;
    const bar = document.getElementById('progressBar');
    document.getElementById('progressFill').className = 'progress-fill';
    document.getElementById('progressFill').style.width = '0%';
    document.getElementById('progressText').textContent = t('progress.starting');
    document.getElementById('progressPct').textContent = '0%';
    document.getElementById('downloadResult').hidden = true;
    if (bar) bar.setAttribute('aria-valuenow', '0');

    downloadActive = true;
    setDownloadButtonsDisabled(true);

    try {
        const data = await apiPost('/pulls', {
            model: modelName,
            ollama_tag: ollamaTag,
        });
        pollDownload(data.id, modelName, ollamaTag);
    } catch (e) {
        // Detect "another download is still running" primarily via the HTTP
        // status code (409 Conflict), with the server message as a fallback
        // so the client is robust against future wording tweaks.
        const busy = e.status === 409 || /already in progress/i.test(String(e.message));
        showError(t('download.startError', { msg: localizeError(e.message) }));
        if (busy) {
            // Another download is still running: keep its progress visible
            // instead of hiding the only indicator the user has.
            pane.hidden = false;
        } else {
            pane.hidden = true;
            downloadActive = false;
            setDownloadButtonsDisabled(false);
        }
    }
}

function pollDownload(jobId, modelName, ollamaTag) {
    if (pollInterval) clearInterval(pollInterval);

    pollInterval = setInterval(async () => {
        try {
            const data = await apiGet(`/pulls/${jobId}`);
            updateProgress(data);

            if (data.status === 'done') {
                clearInterval(pollInterval);
                pollInterval = null;
                onDownloadComplete(modelName, ollamaTag);
            } else if (data.status === 'error') {
                clearInterval(pollInterval);
                pollInterval = null;
                onDownloadError(data.message);
            }
        } catch (e) {
            // Continue polling on transient errors
        }
    }, 1000);
}

function updateProgress(data) {
    const pct = Math.round(data.progress_pct || 0);
    document.getElementById('progressFill').style.width = `${pct}%`;
    document.getElementById('progressText').textContent = localizeProgress(data.message);
    document.getElementById('progressPct').textContent = `${pct}%`;
    const bar = document.getElementById('progressBar');
    if (bar) bar.setAttribute('aria-valuenow', String(pct));
}

function onDownloadComplete(modelName, ollamaTag) {
    downloadActive = false;
    setDownloadButtonsDisabled(false);
    document.getElementById('progressFill').classList.add('done');
    document.getElementById('progressPct').textContent = '100%';
    document.getElementById('progressText').textContent = t('download.complete');

    const result = document.getElementById('downloadResult');
    result.hidden = false;
    result.innerHTML = `
        <div class="result-success">
            <h3>${escapeHtml(t('download.installedTitle'))}</h3>
            <p><strong>${escapeHtml(humanName(modelName))}</strong> (${escapeHtml(ollamaTag)}) ${escapeHtml(t('download.readyText'))}</p>
            <p>${escapeHtml(t('download.safeaiHint', { name: humanName(modelName) }))}</p>
            <button class="btn btn-secondary" style="margin-top:12px" type="button" onclick="testModel('${escapeAttr(modelName)}', '${escapeAttr(ollamaTag)}')">
                ${ICONS.retry} ${escapeHtml(t('download.runTest'))}
            </button>
        </div>
    `;

    loadOllamaStatus();
}

function onDownloadError(message) {
    downloadActive = false;
    setDownloadButtonsDisabled(false);
    document.getElementById('progressFill').classList.add('error');
    const result = document.getElementById('downloadResult');
    result.hidden = false;
    result.innerHTML = `
        <div class="result-error">
            <h3>${escapeHtml(t('download.failedTitle'))}</h3>
            <p>${escapeHtml(localizeError(message))}</p>
            <p>${escapeHtml(t('download.failedHint'))}</p>
        </div>
    `;
}

// ── Readiness Test ───────────────────────────────────────────
async function testModel(modelName, ollamaTag) {
    const result = document.getElementById('downloadResult');
    const pane = document.getElementById('progressPane');
    pane.hidden = false;
    result.hidden = false;
    result.innerHTML = `<div class="spinner" role="status" aria-label="${escapeAttr(t('readiness.runningAria'))}"></div><p class="installed-empty">${escapeHtml(t('readiness.running'))}</p>`;

    try {
        const data = await apiPost(`/models/${encodeURIComponent(ollamaTag)}/readiness-test`, {});
        result.innerHTML = `
            <div class="result-success">
                <h3>${escapeHtml(t('readiness.readyTitle'))}</h3>
                <p><strong>${escapeHtml(humanName(modelName))}</strong> ${escapeHtml(t('readiness.responded'))}</p>
                <p>${escapeHtml(t('readiness.safeaiHint', { name: humanName(modelName) }))}</p>
            </div>
        `;
    } catch (e) {
        result.innerHTML = `
            <div class="result-error">
                <h3>${escapeHtml(t('readiness.failedTitle'))}</h3>
                <p>${escapeHtml(localizeError(e.message))}</p>
            </div>
        `;
    }
}

// ── Guide / Learn ────────────────────────────────────────────
function setupGuide() {
    const input = document.getElementById('guideSearch');
    input.addEventListener('input', () => {
        clearTimeout(guideTimer);
        guideTimer = setTimeout(renderGuide, 200);
    });
}

// Visual-first teaching chunks rendered at the top of each topic body.
// Labels are international technical identifiers (RAM/VRAM, Q4, 1B …) so the
// visuals work in every locale without a separate translation.
function guideVisualFor(id) {
    const V = (key) => t(`guide.visuals.${key}`);
    switch (id) {
        case 'size': {
            const ids = ['1B', '3B', '7B', '14B', '32Bp'];
            return `<div class="gv-size-row" role="list">
                ${ids.map(k => `<span class="gv-size-pill" role="listitem">
                    <span class="gv-size-pill-lbl mono">${V('size.' + k + '.lbl')}</span>
                    <span class="gv-size-pill-meta">${V('size.' + k + '.v')}</span>
                </span>`).join('')}
            </div>`;
        }
        case 'quant': {
            const rows = [
                { id: 'q4', mem: 88, spd: 92, qual: 60 },
                { id: 'q5', mem: 70, spd: 78, qual: 75 },
                { id: 'q6', mem: 54, spd: 60, qual: 88 },
                { id: 'q8', mem: 36, spd: 44, qual: 98 },
            ];
            return `<div class="gv-tradeoff">
                <div class="gv-tradeoff-row gv-tradeoff-header">
                    <span class="gv-tradeoff-ax">${V('quant.axes.quant')}</span>
                    <span class="gv-tradeoff-ax"><span class="gv-tradeoff-tag gv-tag-memory">${V('quant.axes.memory')}</span></span>
                    <span class="gv-tradeoff-ax"><span class="gv-tradeoff-tag gv-tag-speed">${V('quant.axes.speed')}</span></span>
                    <span class="gv-tradeoff-ax"><span class="gv-tradeoff-tag gv-tag-quality">${V('quant.axes.quality')}</span></span>
                </div>
                ${rows.map(q => `<div class="gv-tradeoff-row">
                    <span class="gv-tradeoff-lbl mono">${V('quant.' + q.id + '.lbl')}</span>
                    <span class="gv-tradeoff-cell"><span class="gv-tradeoff-fill" style="--w:${q.mem}%"></span></span>
                    <span class="gv-tradeoff-cell"><span class="gv-tradeoff-fill" style="--w:${q.spd}%"></span></span>
                    <span class="gv-tradeoff-cell"><span class="gv-tradeoff-fill" style="--w:${q.qual}%"></span></span>
                </div>`).join('')}
            </div>`;
        }
        case 'ramvram':
            return `<div class="gv-memflow">
                <div class="gv-mem-item">
                    <span class="gv-mem-mark gv-mem-mark-ram"></span>
                    <span class="gv-mem-name">${V('ram.ram.lbl')}</span>
                    <span class="gv-mem-desc">${V('ram.ram.desc')}</span>
                </div>
                <div class="gv-mem-item">
                    <span class="gv-mem-mark gv-mem-mark-vram"></span>
                    <span class="gv-mem-name">${V('ram.vram.lbl')}</span>
                    <span class="gv-mem-desc">${V('ram.vram.desc')}</span>
                </div>
                <div class="gv-mem-item">
                    <span class="gv-mem-mark gv-mem-mark-uni"></span>
                    <span class="gv-mem-name">${V('ram.unified.lbl')}</span>
                    <span class="gv-mem-desc">${V('ram.unified.desc')}</span>
                </div>
            </div>`;
        case 'tokens': {
            const steps = [
                { id: 'slow',        w: 14 },
                { id: 'usable',      w: 38 },
                { id: 'comfortable', w: 64 },
                { id: 'fast',        w: 100 },
            ];
            return `<div class="gv-scale">
                ${steps.map(s => `<div class="gv-scale-step" style="--w:${s.w}%">
                    <span class="gv-scale-rail" aria-hidden="true"></span>
                    <span class="gv-scale-lvl">${V('tokens.' + s.id + '.lbl')}</span>
                    <span class="gv-scale-num mono">${V('tokens.' + s.id + '.num')} tok/s</span>
                </div>`).join('')}
            </div>`;
        }
        case 'context': {
            const bands = [
                { id: 'short',  w: 22 },
                { id: 'medium', w: 56 },
                { id: 'long',   w: 100 },
            ];
            return `<div class="gv-context">
                ${bands.map(c => `<div class="gv-context-band" style="--w:${c.w}%">
                    <span class="gv-context-lbl mono">${V('context.' + c.id + '.lbl')}</span>
                </div>`).join('')}
            </div>`;
        }
        case 'caps': {
            const subs = (I18N.data.guide.visuals.caps.subs || []).map(s => escapeHtml(s));
            return `<div class="gv-caps">
                <span class="gv-cap gv-cap-primary">${escapeHtml(V('caps.primary'))}</span>
                ${subs.map(s => `<span class="gv-cap">${s}</span>`).join('')}
            </div>`;
        }
        case 'moe':
            return `<div class="gv-diagram">
                ${['total', 'active', 'result'].map((k, i) => `<div class="gv-diagram-step">
                    <span class="gv-num" aria-hidden="true">${i + 1}</span>
                    <div class="gv-diagram-body">
                        <span class="gv-diagram-title">${V('moe.' + k + '.t')}</span>
                        <span class="gv-diagram-text">${V('moe.' + k + '.d')}</span>
                    </div>
                </div>`).join('<span class="gv-diagram-arrow" aria-hidden="true">›</span>')}
            </div>`;
        case 'tags': {
            const ex = V('tags.example');
            const parts = ex.split(':');
            const last = parts[parts.length - 1].split('-');
            // Reassemble: family : version - quantization, where version may contain
            // the size quantifier (e.g. 7b-instruct) and the final segment is the quant.
            const family = escapeHtml(parts[0]);
            const rest = last[0];   // version with size, e.g. "7b"
            const quant = escapeHtml(last.slice(1).join('-') || parts[parts.length - 1]);
            return `<div class="gv-tag-flow">
                <span class="gv-tag-piece gv-tag-piece-family mono">${family}</span>
                <span class="gv-tag-piece-sep">·</span>
                <span class="gv-tag-piece gv-tag-piece-version mono">${escapeHtml(parts[1] || rest)}</span>
                <span class="gv-tag-piece-sep">·</span>
                <span class="gv-tag-piece gv-tag-piece-quant mono">${quant}</span>
            </div>`;
            }
        case 'license':
            return `<div class="gv-lic">
                ${['open', 'llama', 'restricted'].map(k => `<div class="gv-lic-row gv-lic-${k === 'open' ? 'open' : k === 'llama' ? 'limited' : 'restricted'}">
                    <span class="gv-lic-bar" aria-hidden="true"></span>
                    <span class="gv-lic-tag">${V('license.' + k + '.tag')}</span>
                    <span class="gv-lic-desc">${V('license.' + k + '.d')}</span>
                </div>`).join('')}
            </div>`;
        case 'measured':
            return `<div class="gv-callout">
                <span class="gv-callout-rule" aria-hidden="true"></span>
                <div>
                    <span class="gv-callout-title">${V('measured.title')}</span>
                    <span class="gv-callout-body">${V('measured.body')}</span>
                </div>
            </div>`;
        default:
            return '';
    }
}

function guideTopicHtml(id, topic, state) {
    const icon = GUIDE_ICONS[topic.icon] || GUIDE_ICONS.size;
    const visuals = guideVisualFor(id);
    const paras = (topic.paragraphs || []).slice(0, 1).map(p => `<p>${escapeHtml(p)}</p>`).join('');
    const bullets = '';
    const open = state && state.open ? ' open' : '';
    const hidden = state && state.hidden ? ' hidden' : '';
    return `
    <details class="guide-topic${open}" data-topic="${escapeAttr(id)}"${hidden}>
        <summary>
            <span class="guide-topic-icon" aria-hidden="true">${icon}</span>
            <span class="guide-topic-head">
                <span class="guide-topic-title">${escapeHtml(topic.title)}</span>
                <span class="guide-topic-summary">${escapeHtml(topic.summary)}</span>
            </span>
        </summary>
        <div class="guide-topic-body">
            <div class="guide-visuals">${visuals}</div>
            ${paras}
            ${bullets}
        </div>
    </details>`;
}

function renderGuide() {
    const topicsBox = document.getElementById('guideTopics');
    const glossaryBox = document.getElementById('guideGlossary');
    const guide = I18N.data.guide;
    if (!guide) return;

    const q = document.getElementById('guideSearch').value.trim().toLowerCase();

    const topics = Object.entries(guide.topics || {});
    let anyTopic = false;
    topicsBox.innerHTML = topics.map(([id, topic]) => {
        const hay = [topic.title, topic.summary, ...(topic.paragraphs || []), ...(topic.bullets || [])].join(' ').toLowerCase();
        const match = !q || hay.includes(q);
        if (match) anyTopic = true;
        return guideTopicHtml(id, topic, { open: q && match, hidden: q && !match });
    }).join('');

    const entries = (guide.glossary && guide.glossary.entries) || [];
    const matched = q ? entries.filter(e => (e.term + ' ' + e.def).toLowerCase().includes(q)) : entries;
    glossaryBox.innerHTML = matched.length
        ? `<dl class="guide-glossary-list">${matched.map(e => `
            <details class="guide-term">
                <summary class="guide-term-name mono"><span>${escapeHtml(e.term)}</span></summary>
                <dd class="guide-term-def">${escapeHtml(e.def)}</dd>
            </details>`).join('')}</dl>`
        : `<p class="guide-empty">${escapeHtml(t('guide.noMatches', { q: document.getElementById('guideSearch').value.trim() }))}</p>`;
}

// ── Helpers ──────────────────────────────────────────────────
function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

function escapeAttr(str) {
    return escapeHtml(String(str)).replace(/"/g, '&quot;');
}

function showError(msg) {
    const banner = document.getElementById('errorBanner');
    banner.textContent = msg;
    banner.classList.remove('banner-success');
    banner.hidden = false;
    setTimeout(() => { banner.hidden = true; }, 5000);
}

function showSuccess(msg) {
    const banner = document.getElementById('errorBanner');
    banner.textContent = msg;
    banner.classList.add('banner-success');
    banner.hidden = false;
    setTimeout(() => {
        banner.hidden = true;
        banner.classList.remove('banner-success');
    }, 6000);
}
