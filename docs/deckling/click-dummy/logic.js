class Component extends DCLogic {
constructor(props) {
super(props);
this.dragApp = null;
this.dragKey = null;
this.sig = { t: 0, lampOn: true, hue: 40, bright: 80, dir: -3, micMuted: false, camOn: false, ams1: [92, 64, 38, 80], ams2: [55, 20, 77, 96], amsHue: 200, pomo: 35, pomoRun: true, cpu: 30, playing: true, media: 20 };
this.state = Object.assign({}, this.state, {
view: 'deck', libTab: 'apps', profile: 0, page: 0, sel: 5, editProfile: false, front: 'Finder', pcfgOpen: null, autostart: true, appHint: '',
pcfg: { core: { hold: '500', follow: true, saver: '5' }, 'com.ricktap.m365': { client: '', tenant: '' }, 'com.ricktap.hue': { bridge: 'wohnzimmer' }, 'com.ricktap.bambu': { host: 'homeassistant.local', topic: 'd200' } }, slot: 'press', openLong: false, openDown: false,
hint: 'Taste wählen, dann links eine App antippen. Unter Icons siehst du, was zu ihr passt.',
history: [], decks: this.initDecks(), off: {}, theme: 'a', themeOpen: false, cfgOpen: null, ov: {}, m365: 0, n: 0
});
}
componentDidMount() { this.timer = setInterval(() => { this.tick(); this.setState({ n: this.state.n + 1 }); }, 300); }
componentWillUnmount() { clearInterval(this.timer); }
tick() {
const g = this.sig;
g.t += 1;
if (g.lampOn) { g.hue = (g.hue + 7) % 360; g.bright += g.dir; if (g.bright <= 30 || g.bright >= 100) { g.dir = -g.dir; } }
const drop = [0.9, 0.5, 0.3, 0];
g.ams1 = g.ams1.map((v, i) => (v - drop[i] < 0 ? 100 : v - drop[i]));
g.ams2 = g.ams2.map((v, i) => (v - drop[3 - i] < 0 ? 100 : v - drop[3 - i]));
g.amsHue = (g.amsHue + 9) % 360;
if (g.pomoRun) { g.pomo += 0.8; if (g.pomo > 100) { g.pomo = 0; } }
g.cpu = Math.max(3, Math.round(40 + 30 * Math.sin(g.t / 5) + 10 * Math.sin(g.t / 1.7)));
if (g.playing) { g.media += 1.2; if (g.media > 100) { g.media = 0; } }
}
clock() { const d = new Date(); return ('0' + d.getHours()).slice(-2) + ':' + ('0' + d.getMinutes()).slice(-2); }
lampColor() { return 'hsl(' + this.sig.hue + ', 85%, 60%)'; }
apps() {
const amsColors = ['#69E0C1', '#FFBF69', '#FF8278'];
return {
'core:button': { plugin: 'core', name: 'Button', roles: [], icon: 'core:blank', label: '' },
'core:shortcut': { plugin: 'core', name: 'Tastenkürzel', roles: [], icon: 'core:keys', label: 'Shot', press: [{ id: 'core:keys', val: 'cmd+shift+4' }] },
'core:command': { plugin: 'core', name: 'Befehl', roles: [], icon: 'core:terminal', label: 'Tests', press: [{ id: 'core:command', val: 'npm test' }] },
'core:page': { plugin: 'core', name: 'Seite wechseln', roles: [], icon: 'core:page', label: 'Seite', press: [{ id: 'core:page', val: 'Media' }] },
'core:clock': { plugin: 'core', name: 'Uhr', roles: ['clock'], icon: 'core:clock', label: 'Uhr',
config: [{ key: 'display', label: 'Anzeige', type: 'select', def: 'analog', fills: 'clock.style', values: [{ label: 'Analog', value: 'analog' }, { label: 'Digital', value: 'digital' }] }],
props: (g, time, cfg) => { const d = new Date(); return { 'clock.time': time, 'clock.hour': d.getHours(), 'clock.minute': d.getMinutes(), 'clock.style': cfg.display }; } },
'core:sysmon': { plugin: 'core', name: 'Systemmonitor', roles: ['progress'], icon: 'core:bar', label: 'CPU', props: (g) => ({ 'progress.value': g.cpu }) },
'core:pomodoro': { plugin: 'core', name: 'Pomodoro', roles: ['progress'], icon: 'core:ring', label: 'Fokus', press: [{ id: 'core:pomodoro.toggle' }], long: [{ id: 'core:pomodoro.reset' }], props: (g) => ({ 'progress.value': g.pomo }) },
'core:media': { plugin: 'core', name: 'Musik', roles: ['progress', 'toggle'], icon: 'core:player', label: 'Musik', press: [{ id: 'core:media.toggle' }], long: [{ id: 'core:media.next' }], props: (g) => ({ 'progress.value': g.media, 'toggle.on': g.playing }) },
'com.ricktap.m365:mic': { plugin: 'com.ricktap.m365', name: 'Mikrofon', roles: ['toggle'], icon: 'core:mic', label: 'Mikro', press: [{ id: 'com.ricktap.m365:mic.toggle' }], props: (g) => ({ 'toggle.on': !g.micMuted }) },
'com.ricktap.m365:cam': { plugin: 'com.ricktap.m365', name: 'Kamera', roles: ['toggle'], icon: 'core:camera', label: 'Kamera', press: [{ id: 'com.ricktap.m365:cam.toggle' }], props: (g) => ({ 'toggle.on': g.camOn }) },
'com.ricktap.hue:light': { plugin: 'com.ricktap.hue', name: 'Lampe', roles: ['com.ricktap.hue:rgb_light', 'rgb', 'toggle'], icon: 'com.ricktap.hue:bulb', label: 'Lampe', press: [{ id: 'com.ricktap.hue:lamp.toggle' }],
props: (g) => { const c = 'hsl(' + g.hue + ', 85%, 60%)'; return { 'com.ricktap.hue:rgb_light.on': g.lampOn, 'com.ricktap.hue:rgb_light.color': c, 'com.ricktap.hue:rgb_light.brightness': g.bright, 'rgb.color': c, 'toggle.on': g.lampOn }; } },
'com.ricktap.bambu:ams': { plugin: 'com.ricktap.bambu', name: 'AMS', roles: ['com.ricktap.bambu:ams'], icon: 'com.ricktap.bambu:rings', label: 'AMS',
config: [{ key: 'unit', label: 'Gerät', type: 'select', def: 'ams1', note: 'Typ select, Schlüssel unit, Werte liefert das Plugin zur Laufzeit. Wählt die Signale com.ricktap.bambu:{unit}.*', values: [{ label: 'AMS 1', value: 'ams1' }, { label: 'AMS 2', value: 'ams2' }] }],
props: (g, time, cfg) => { const o = {}; g[cfg.unit].forEach((v, i) => { o['com.ricktap.bambu:ams.slot' + (i + 1) + '.remaining'] = v; o['com.ricktap.bambu:ams.slot' + (i + 1) + '.color'] = i === 3 ? 'hsl(' + g.amsHue + ', 80%, 62%)' : amsColors[i]; }); return o; } }
};
}
actions() {
const g = this.sig;
return {
'core:keys': { plugin: 'core', name: 'Tastenkürzel senden', val: 'cmd+shift+4' },
'core:command': { plugin: 'core', name: 'Befehl ausführen', val: 'npm test' },
'core:page': { plugin: 'core', name: 'Seite wechseln', val: 'Media', run: (val) => { const at = this.state.decks[this.pi()].pages.findIndex((p) => p.name === val); if (at >= 0) { this.setState({ page: at, sel: 0 }); } } },
'core:pomodoro.toggle': { plugin: 'core', name: 'Pomodoro starten oder anhalten', run: () => { g.pomoRun = !g.pomoRun; } },
'core:pomodoro.reset': { plugin: 'core', name: 'Pomodoro zurücksetzen', run: () => { g.pomo = 0; } },
'core:media.toggle': { plugin: 'core', name: 'Musik starten oder anhalten', run: () => { g.playing = !g.playing; } },
'core:media.next': { plugin: 'core', name: 'Nächster Titel', run: () => { g.media = 0; } },
'com.ricktap.m365:mic.toggle': { plugin: 'com.ricktap.m365', name: 'Mikrofon schalten', run: () => { g.micMuted = !g.micMuted; } },
'com.ricktap.m365:cam.toggle': { plugin: 'com.ricktap.m365', name: 'Kamera schalten', run: () => { g.camOn = !g.camOn; } },
'com.ricktap.hue:lamp.toggle': { plugin: 'com.ricktap.hue', name: 'Lampe schalten', run: () => { g.lampOn = !g.lampOn; } }
};
}
st(d, s, w, dash) { return { d: d, s: s, f: 'none', w: w || 5, dash: dash || 'none' }; }
fl(d, f) { return { d: d, s: 'none', f: f, w: 0, dash: 'none' }; }
circ(cx, cy, r) { return 'M' + cx + ' ' + (cy - r) + 'a' + r + ' ' + r + ' 0 1 1 0 ' + (2 * r) + 'a' + r + ' ' + r + ' 0 1 1 0 ' + (-2 * r); }
arc(len, v) { return (len * Math.max(0, Math.min(100, v)) / 100).toFixed(1) + ' 400'; }
icons() {
const S = (d, s, w, dash) => this.st(d, s, w, dash), F = (d, f) => this.fl(d, f), C = (x, y, r) => this.circ(x, y, r), A = (len, v) => this.arc(len, v);
const V24 = '0 0 24 24', CORAL = '#FF8278';
const GLASS = 'M50 10a25 25 0 0 0-15 45c3 3 4 6 4 10h22c0-4 1-7 4-10a25 25 0 0 0-15-45z';
const BASE = 'M41 72h18M44 79h12';
const HOUSING = 'M18 14h64a6 6 0 0 1 6 6v60a6 6 0 0 1-6 6H18a6 6 0 0 1-6-6V20a6 6 0 0 1 6-6z';
const pct = (v) => (v === null ? '' : Math.round(v) + '%');
const AMS = 'com.ricktap.bambu:ams', HUE = 'com.ricktap.hue:rgb_light';
return {
'core:blank': { set: 'core', name: 'Leer', roles: ['button'], vb: V24, draw: (c) => ({ paths: [S('M5 5h14v14H5z', c.mut, 1.7)] }) },
'core:keys': { set: 'core', name: 'Tastatur', roles: ['button'], vb: V24, draw: (c) => ({ paths: [S('M3 7h18v10H3zM7 11h.01M11 11h.01M15 11h.01', c.line, 1.7), S('M8 14h8', c.acc, 1.7)] }) },
'core:terminal': { set: 'core', name: 'Terminal', roles: ['button'], vb: V24, draw: (c) => ({ paths: [S('M4 5h16v14H4z', c.line, 1.7), S('M8 10l3 2-3 2M13 15h3', c.acc, 1.7)] }) },
'core:page': { set: 'core', name: 'Seite', roles: ['button'], vb: V24, draw: (c) => ({ paths: [S('M5 4h14v16H5z', c.line, 1.7), S('M9 9h6M9 13h6', c.acc, 1.7)] }) },
'core:clock': { set: 'core', name: 'Uhr', roles: ['clock', 'button'], vb: V24, draw: (c) => {
if (c.P('clock.style', 'analog') === 'digital') { return { paths: [], value: c.P('clock.time', ''), big: true }; }
const h = c.P('clock.hour', 10), m = c.P('clock.minute', 10);
const hand = (deg, len) => 'M12 12L' + (12 + len * Math.sin(deg * Math.PI / 180)).toFixed(2) + ' ' + (12 - len * Math.cos(deg * Math.PI / 180)).toFixed(2);
return { paths: [S('M12 3a9 9 0 1 0 0 18a9 9 0 0 0 0-18z', c.line, 1.7), S(hand((h % 12 + m / 60) * 30, 4.5), c.line, 1.7), S(hand(m * 6, 6.8), c.acc, 1.7)] };
} },
'core:ring': { set: 'core', name: 'Ring', roles: ['progress', 'button'], draw: (c) => { const v = c.P('progress.value', null); return { paths: [S(C(50, 50, 30), c.mut, 9), v === null ? null : S(C(50, 50, 30), c.acc, 9, A(188.5, v))], value: pct(v) }; } },
'core:bar': { set: 'core', name: 'Balken', roles: ['progress', 'button'], draw: (c) => { const v = c.P('progress.value', null); return { paths: [S('M18 50H82', c.mut, 12), v === null ? null : S('M18 50H82', c.acc, 12, A(64, v))], value: pct(v) }; } },
'core:player': { set: 'core', name: 'Player', roles: ['progress', 'toggle', 'button'], draw: (c) => { const v = c.P('progress.value', null), on = c.P('toggle.on', false); return { paths: [S(C(50, 50, 34), c.mut, 7), v === null ? null : S(C(50, 50, 34), c.acc, 7, A(213.6, v)), on ? F('M39 36h8v28h-8zM53 36h8v28h-8z', c.line) : F('M42 34l24 16-24 16z', c.line)] }; } },
'core:mic': { set: 'core', name: 'Mikrofon', roles: ['toggle', 'button'], vb: V24, draw: (c) => { const on = c.P('toggle.on', null), ok = on !== false; return { paths: [S('M12 3a3 3 0 0 0-3 3v5a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3zM12 18v3', c.line, 1.7), S('M6 11a6 6 0 0 0 12 0', ok ? c.acc : CORAL, 1.7), ok ? null : S('M4 4l16 16', CORAL, 1.7)], value: on === null ? '' : (on ? 'An' : 'Stumm') }; } },
'core:camera': { set: 'core', name: 'Kamera', roles: ['toggle', 'button'], vb: V24, draw: (c) => { const on = c.P('toggle.on', null), ok = on !== false; return { paths: [S('M3 7h12v10H3z', c.line, 1.7), S('M15 10l6-3v10l-6-3', ok ? c.acc : CORAL, 1.7), ok ? null : S('M4 4l16 16', CORAL, 1.7)], value: on === null ? '' : (on ? 'An' : 'Aus') }; } },
'com.ricktap.hue:bulb': { set: 'com.ricktap.hue', name: 'Hue Lampe', roles: [HUE], draw: (c) => { const on = c.P(HUE + '.on', false), col = c.P(HUE + '.color', '#FFD84D'), br = c.P(HUE + '.brightness', 0); return { paths: [on ? F(GLASS, col) : null, S(GLASS + BASE, c.line, 5), S('M22 92H78', c.mut, 5), S('M22 92H78', col, 5, A(56, br))] }; } },
'com.ricktap.bambu:rings': { set: 'com.ricktap.bambu', name: 'AMS Ringe', roles: [AMS], draw: (c) => { const pos = [[33, 33], [67, 33], [33, 67], [67, 67]]; const paths = [S(HOUSING, c.T(AMS + '.casing', c.line), 4), S(pos.map((p) => C(p[0], p[1], 11)).join(''), c.mut, 6)]; pos.forEach((p, i) => { paths.push(S(C(p[0], p[1], 11), c.P(AMS + '.slot' + (i + 1) + '.color', c.acc), 6, A(69.1, c.P(AMS + '.slot' + (i + 1) + '.remaining', 0)))); }); return { paths: paths }; } },
'com.jemand.neon:bulb': { set: 'com.jemand.neon', name: 'Glühbirne', roles: ['rgb', 'toggle', 'button'], vb: '0 -6 100 100', draw: (c) => { const on = c.P('toggle.on', false); return { paths: [on ? F(GLASS, c.P('rgb.color', '#FFD84D')) : null, S(GLASS + BASE, c.line, 5)] }; } },
'com.jemand.neon:switch': { set: 'com.jemand.neon', name: 'Schalter', roles: ['toggle', 'button'], draw: (c) => { const on = c.P('toggle.on', false); return { paths: [F('M34 34h32a16 16 0 0 1 0 32H34a16 16 0 0 1 0-32z', on ? c.acc : c.mut), F(C(on ? 66 : 34, 50, 11), c.line)] }; } },
'com.jemand.neon:dial': { set: 'com.jemand.neon', name: 'Skala', roles: ['progress', 'button'], draw: (c) => { const v = c.P('progress.value', null); return { paths: [S('M20 64a30 30 0 0 1 60 0', c.mut, 9), v === null ? null : S('M20 64a30 30 0 0 1 60 0', c.acc, 9, A(94.2, v))], value: pct(v) }; } },
'com.jemand.neon:spools': { set: 'com.jemand.neon', name: 'AMS Säulen', roles: [AMS], draw: (c) => { const xs = [29, 43, 57, 71]; const paths = [S(HOUSING, c.T(AMS + '.casing', c.line), 4)]; xs.forEach((x, i) => { const r = c.P(AMS + '.slot' + (i + 1) + '.remaining', 0); paths.push(S('M' + x + ' 74V' + (74 - 46 * r / 100).toFixed(1), c.P(AMS + '.slot' + (i + 1) + '.color', c.acc), 8)); }); return { paths: paths }; } },
'com.jemand.neon:lamp': { set: 'com.jemand.neon', name: 'Tischlampe', roles: ['button'], draw: (c) => ({ paths: [S('M28 86h44M50 86V50M30 50l10-28h20l10 28z', c.line, 5)] }) }
};
}
mk(id, label, val) {
const a = this.apps()[id];
const copy = (list) => (list || []).map((x) => ({ id: x.id, val: x.val || '' }));
const k = { app: id, icon: a.icon, label: label !== undefined ? label : a.label, press: copy(a.press), long: copy(a.long), down: [], config: this.defaults(a) };
if (val && k.press.length) { k.press[0].val = val; }
return k;
}
initDecks() {
const e = null, m = (id, l, v) => this.mk(id, l, v);
return [
{ name: 'Default', apps: [], pages: [
{ name: 'Main', keys: [m('core:clock'), m('core:sysmon'), m('core:pomodoro'), m('com.ricktap.m365:mic'), m('com.ricktap.m365:cam'), m('com.ricktap.hue:light'), m('com.ricktap.bambu:ams'), m('core:media'), m('core:shortcut'), e, Object.assign(m('com.ricktap.bambu:ams', 'AMS 2'), { config: { unit: 'ams2' } }), e, m('core:page', 'Media', 'Media')] },
{ name: 'Media', keys: [m('core:media'), m('com.ricktap.hue:light'), e, e, e, e, e, e, e, e, e, e, m('core:page', 'Main', 'Main')] }
] },
{ name: 'Meeting', apps: ['Microsoft Teams'], pages: [{ name: 'Main', keys: [m('com.ricktap.m365:mic'), m('com.ricktap.m365:cam'), m('com.ricktap.hue:light'), m('core:clock'), e, e, e, e, e, e, e, e, e] }] }
];
}
defaults(app) { const o = {}; (app.config || []).forEach((f) => { o[f.key] = f.def; }); return o; }
pi() { return Math.min(this.state.profile, this.state.decks.length - 1); }
gi() { return Math.min(this.state.page, this.state.decks[this.pi()].pages.length - 1); }
curKeys() { return this.state.decks[this.pi()].pages[this.gi()].keys; }
mutateDecks(fn, hint, extra) {
const snap = JSON.stringify(this.state.decks);
const decks = JSON.parse(snap);
fn(decks);
this.setState(Object.assign({ decks: decks, history: this.state.history.concat([snap]).slice(-30), hint: hint || this.state.hint }, extra || {}));
}
mutate(fn, hint) {
const snap = JSON.stringify(this.state.decks);
const decks = JSON.parse(snap);
fn(decks[this.pi()].pages[this.gi()].keys);
this.setState({ decks: decks, history: this.state.history.concat([snap]).slice(-30), hint: hint || this.state.hint });
}
palette(id) { return (id || this.state.theme) === 'a' ? { background: '#12151A', base: '#F4F1E8', muted: '#3A4350', accent: '#69E0C1' } : { background: '#211A14', base: '#FFE6C2', muted: '#5A4A3A', accent: '#FF9F43' }; }
tvar(name, idx) {
const s = this.state, g = this.sig;
if (name === 'accent') {
if (s.ov.hue && !s.off['com.ricktap.hue']) { return this.lampColor(); }
if (s.ov.wave) { return 'hsl(' + ((g.t * 12 + (idx % 5) * 30 + Math.floor(idx / 5) * 18) % 360) + ', 85%, 68%)'; }
if (s.ov.rainbow) { return 'hsl(' + ((g.t * 12) % 360) + ', 85%, 68%)'; }
}
if (name === 'com.ricktap.bambu:ams.casing') { return s.ov.casing ? '#6F8FD9' : undefined; }
return this.palette()[name];
}
share(icon, app) { return app.roles.concat(['button']).filter((r) => icon.roles.indexOf(r) >= 0); }
short(role) { return role.indexOf(':') >= 0 ? role.split(':')[1] : role; }
effIcon(appId, iconId) {
const icons = this.icons(), app = this.apps()[appId];
const icon = icons[iconId];
if (!icon || this.state.off[icon.set] || this.share(icon, app).length === 0) { return app.icon; }
return iconId;
}
face(appId, iconId, idx, cfg) {
const app = this.apps()[appId], icon = this.icons()[this.effIcon(appId, iconId)];
const props = app.props ? app.props(this.sig, this.clock(), Object.assign(this.defaults(app), cfg || {})) : {};
const c = { P: (n, fb) => (n in props ? props[n] : fb), T: (n, fb) => { const v = this.tvar(n, idx); return v === undefined ? fb : v; } };
c.line = c.T('base'); c.acc = c.T('accent'); c.mut = c.T('muted');
const out = icon.draw(c);
const paths = out.paths.filter((p) => !!p);
const f = { vb: icon.vb || '0 0 100 100', value: out.value || '', big: !!out.big, has: paths.length > 0 };
for (let i = 0; i < 6; i += 1) { f['p' + i] = paths[i] || { d: '', s: 'none', f: 'none', w: 0, dash: 'none' }; }
return f;
}
emptyFace() { const f = { vb: '0 0 24 24', value: '', big: false, has: false }; for (let i = 0; i < 6; i += 1) { f['p' + i] = { d: '', s: 'none', f: 'none', w: 0, dash: 'none' }; } return f; }
pill(on) { return on ? { border: '#B79AF7', bg: '#1A1626', color: '#F4F1E8' } : { border: '#262B33', bg: '#12151A', color: '#C9CED6' }; }
sw(on) { return { track: on ? '#69E0C1' : '#5A6270', align: on ? 'flex-end' : 'flex-start' }; }
assign(index, id) {
const a = this.apps()[id];
if (index === 13) { this.setState({ hint: 'Die breite Taste ist in diesem Dummy nicht belegbar.' }); return; }
this.mutate((keys) => { keys[index] = this.mk(id); }, a.name + ' liegt jetzt auf Taste ' + (index + 1) + ', mit dem Standard Icon des Plugins.');
this.setState({ sel: index, slot: 'press' });
}
renderVals() {
const s = this.state, g = this.sig, apps = this.apps(), acts = this.actions(), icons = this.icons();
const keysData = this.curKeys();
const on = (p) => p === 'core' || !s.off[p];
const time = this.clock();
const plugins = ['core', 'com.ricktap.m365', 'com.ricktap.hue', 'com.ricktap.bambu'];

const nav = [['deck', 'Deck'], ['packages', 'Pakete'], ['signals', 'Signale']].map((n) => Object.assign({ name: n[1], go: () => this.setState({ view: n[0] }) }, this.pill(s.view === n[0])));
const libTabs = [['apps', 'Apps'], ['actions', 'Aktionen'], ['icons', 'Icons']].map((t) => Object.assign({ name: t[1], go: () => this.setState({ libTab: t[0] }) }, this.pill(s.libTab === t[0])));

const appGroups = plugins.filter(on).map((p) => ({
title: p,
items: Object.keys(apps).filter((id) => apps[id].plugin === p).map((id) => ({
name: apps[id].name, roles: apps[id].roles.map((r) => this.short(r)).concat(['button']).join(', '), face: this.face(id, apps[id].icon, 0),
pick: () => this.assign(s.sel, id),
drag: (e) => { this.dragApp = id; this.dragKey = null; if (e.dataTransfer) { e.dataTransfer.setData('text/plain', id); } }
}))
}));

const selK = s.sel === 13 ? null : keysData[s.sel];
const selMissing = !!selK && !on(apps[selK.app].plugin);
const selKey = !!selK && !selMissing;
const selApp = selKey ? apps[selK.app] : null;

const actionGroups = plugins.filter(on).map((p) => ({
title: p,
items: Object.keys(acts).filter((id) => acts[id].plugin === p).map((id) => ({
name: acts[id].name,
pick: () => {
if (s.sel === 13) { this.setState({ hint: 'Die breite Taste ist in diesem Dummy nicht belegbar.' }); return; }
this.mutate((keys) => { if (!keys[s.sel]) { keys[s.sel] = this.mk('core:button'); } keys[s.sel][s.slot].push({ id: id, val: acts[id].val || '' }); }, acts[id].name + ' hängt jetzt an Taste ' + (s.sel + 1) + '.');
}
}))
})).filter((grp) => grp.items.length > 0);

let iconChoices = [];
if (selKey) {
const offered = selApp.roles.concat(['button']);
const cur = this.effIcon(selK.app, selK.icon);
iconChoices = Object.keys(icons).filter((id) => !s.off[icons[id].set] && this.share(icons[id], selApp).length > 0)
.map((id) => ({ id: id, rank: offered.indexOf(this.share(icons[id], selApp)[0]) }))
.sort((a, b) => a.rank - b.rank)
.map((x) => {
const chosen = cur === x.id;
return {
name: icons[x.id].name, role: this.short(offered[x.rank]), face: this.face(selK.app, x.id, s.sel, selK.config), on: chosen,
aria: 'Icon ' + icons[x.id].name + ' für Rolle ' + this.short(offered[x.rank]),
border: chosen ? '#B79AF7' : '#262B33', bg: this.tvar('background', s.sel),
pick: () => this.mutate((keys) => { keys[s.sel].icon = x.id; }, 'Taste ' + (s.sel + 1) + ' nutzt jetzt ' + x.id + '.')
};
});
}
const bulk = (pickIcon, hint) => this.mutate((keys) => { keys.forEach((k) => { if (k) { k.icon = pickIcon(k); } }); }, hint);

const keys = keysData.map((k, i) => {
const missing = !!k && !on(apps[k.app].plugin);
const face = k && !missing ? this.face(k.app, k.icon, i, k.config) : this.emptyFace();
const line = missing ? '#9AA3B0' : this.tvar('base', i);
return {
aria: 'Taste ' + (i + 1) + (k ? ', ' + apps[k.app].name : ', leer'),
showIcon: !!k && !missing && face.has, valueSize: face.big ? '17px' : '10px', face: face,
hasLabel: !!k && (missing || !!k.label), label: missing ? 'Plugin fehlt' : (k ? k.label : ''),
hasValue: !!face.value, value: face.value, accent: this.tvar('accent', i), line: line,
bg: k ? this.tvar('background', i) : '#0B0D10',
border: s.sel === i ? '#B79AF7' : (missing ? '#FFBF69' : (k ? '#262B33' : '#1C2027')),
borderStyle: missing || !k ? 'dashed' : 'solid',
select: () => this.setState({ sel: i, slot: 'press' }),
drag: (e) => { this.dragKey = i; this.dragApp = null; if (e.dataTransfer) { e.dataTransfer.setData('text/plain', 'key'); } },
over: (e) => { e.preventDefault(); },
drop: (e) => {
e.preventDefault();
if (this.dragApp) { this.assign(i, this.dragApp); }
else if (this.dragKey !== null && this.dragKey !== i) {
const from = this.dragKey;
this.mutate((arr) => { const tmp = arr[from]; arr[from] = arr[i]; arr[i] = tmp; }, 'Taste ' + (from + 1) + ' und Taste ' + (i + 1) + ' getauscht.');
this.setState({ sel: i });
}
this.dragApp = null; this.dragKey = null;
}
};
});

const prof = s.decks[this.pi()], pageNow = prof.pages[this.gi()];
const wide = { eyebrow: 'Profil ' + prof.name, title: 'Seite ' + pageNow.name, sub: time, line: this.tvar('base', 13), accent: this.tvar('accent', 13), bg: this.tvar('background', 13), border: s.sel === 13 ? '#B79AF7' : '#262B33' };
const profiles = s.decks.map((p, i) => Object.assign({ name: p.name, go: () => this.setState({ profile: i, page: 0, sel: 0, slot: 'press' }) }, this.pill(this.pi() === i)));
const pageTabs = prof.pages.map((p, i) => Object.assign({ name: p.name, go: () => this.setState({ page: i, sel: 0, slot: 'press' }) }, this.pill(this.gi() === i)));
const P = this.pi();
const programs = ['Safari', 'Microsoft Teams', 'WebStorm', 'Spotify', 'Bambu Studio'];
const emptyKeys = () => [null, null, null, null, null, null, null, null, null, null, null, null, null];
const bound = prof.apps.map((name) => ({ name: name, aria: name + ' von diesem Profil lösen', remove: () => this.mutateDecks((d) => { d[P].apps = d[P].apps.filter((a) => a !== name); }, name + ' ist nicht mehr an ' + prof.name + ' gebunden.') }));
const candidates = programs.filter((name) => prof.apps.indexOf(name) < 0).map((name) => ({ name: name, add: () => this.mutateDecks((d) => { d.forEach((x) => { x.apps = x.apps.filter((a) => a !== name); }); d[P].apps.push(name); }, name + ' ist jetzt an ' + prof.name + ' gebunden.') }));
const pageRows = prof.pages.map((p, i) => ({
name: p.name, isHome: i === 0, canHome: i > 0, canDelete: prof.pages.length > 1,
rename: (e) => { const v = e.target.value; this.mutateDecks((d) => { d[P].pages[i].name = v; }); },
makeHome: () => this.mutateDecks((d) => { const moved = d[P].pages.splice(i, 1)[0]; d[P].pages.unshift(moved); }, p.name + ' ist jetzt die Startseite.', { page: 0 }),
remove: () => this.mutateDecks((d) => { d[P].pages.splice(i, 1); }, 'Seite ' + p.name + ' gelöscht.', { page: 0 })
}));
const follow = !!s.pcfg.core.follow;
const fronts = ['Finder'].concat(programs).map((name) => Object.assign({ name: name, go: () => {
const hit = s.decks.findIndex((d) => d.apps.indexOf(name) >= 0);
if (!follow) { this.setState({ front: name, hint: name + ' ist vorn. Profile folgen dem Programm gerade nicht, das steht unter Pakete beim Core Plugin.' }); return; }
const to = hit >= 0 ? hit : 0;
this.setState({ front: name, profile: to, page: 0, sel: 0, hint: hit >= 0 ? name + ' ist vorn, das Deck wechselt auf Profil ' + s.decks[to].name + '.' : name + ' ist an kein Profil gebunden, das Deck zeigt das Standardprofil ' + s.decks[0].name + '.' });
} }, this.pill(s.front === name)));

const themes = [['a', 'Night Signal', 'com.ricktap.nightsignal'], ['b', 'Bernstein', 'com.jemand.bernstein']].map((t) => { const p = this.palette(t[0]), cur = s.theme === t[0]; return { key: t[0], name: t[1], id: t[2], c0: p.background, c1: p.base, c2: p.accent, c3: p.muted, on: cur, border: cur ? '#B79AF7' : 'transparent', bg: cur ? '#1A1626' : 'transparent', go: () => this.setState({ theme: t[0], themeOpen: false }) }; });
const ovDefs = [['rainbow', 'Regenbogen', 'accent an time.tick', 'com.jemand.regenbogen'], ['wave', 'Welle', 'accent an time.tick und key.column', 'com.jemand.welle'], ['hue', 'Hue Sync', 'accent an com.ricktap.hue:lamp.color', 'com.ricktap.hue-sync'], ['casing', 'AMS Gehäuse', 'com.ricktap.bambu:ams.casing in Blau', 'com.jemand.ams-blau']];
const flipOv = (key) => { const ov = Object.assign({}, s.ov); ov[key] = !ov[key]; this.setState({ ov: ov }); };
const overlays = ovDefs.map((o) => ({ name: o[1], what: 'Setzt ' + o[2], on: !!s.ov[o[0]], go: () => flipOv(o[0]) }));

const slots = !selKey ? [] : [['press', 'Beim Drücken', true], ['long', 'Lang drücken', s.openLong || selK.long.length > 0], ['down', 'Erweitert: Taste unten', s.openDown || selK.down.length > 0]];
const sections = slots.map((d) => {
const list = selK[d[0]], isTarget = s.slot === d[0];
return {
title: d[1], open: d[2], target: isTarget, border: isTarget ? '#4B3F73' : '#262B33',
count: list.length === 0 ? (d[2] ? '' : 'hinzufügen') : list.length + (list.length === 1 ? ' Aktion' : ' Aktionen'),
empty: d[2] && list.length === 0,
toggle: () => {
if (d[0] === 'press') { this.setState({ slot: 'press' }); }
else if (d[0] === 'long') { this.setState({ slot: 'long', openLong: isTarget ? !s.openLong : true }); }
else { this.setState({ slot: 'down', openDown: isTarget ? !s.openDown : true }); }
},
list: list.map((a, idx) => ({ name: acts[a.id].name + (a.val ? ': ' + a.val : ''), id: a.id, remove: () => this.mutate((arr) => { arr[s.sel][d[0]].splice(idx, 1); }, 'Aktion entfernt.') }))
};
});

const setCfg = (pid, key, value) => { const pcfg = Object.assign({}, s.pcfg); pcfg[pid] = Object.assign({}, pcfg[pid], { [key]: value }); this.setState({ pcfg: pcfg, pcfgOpen: null }); };
const field = (pid, f) => {
const cur = s.pcfg[pid][f.key], openKey = pid + '|' + f.key, sw = this.sw(!!cur);
const out = { label: f.label, isText: f.type === 'text', isToggle: f.type === 'toggle', isSelect: f.type === 'select', value: f.type === 'text' ? cur : '', placeholder: f.placeholder || '',
set: (e) => setCfg(pid, f.key, e.target.value), on: !!cur, track: sw.track, align: sw.align, state: cur ? 'an' : 'aus', flip: () => setCfg(pid, f.key, !cur),
open: s.pcfgOpen === openKey, toggle: () => this.setState({ pcfgOpen: s.pcfgOpen === openKey ? null : openKey }), current: '', options: [], note: f.note || '' };
if (f.type === 'select') {
out.current = f.values.filter((v) => v.value === cur)[0].label;
out.options = f.values.map((v) => ({ label: v.label, on: v.value === cur, border: v.value === cur ? '#B79AF7' : 'transparent', bg: v.value === cur ? '#1A1626' : 'transparent', pick: () => setCfg(pid, f.key, v.value) }));
}
return out;
};
const mSteps = ['Einrichtung nötig', 'Schritt 1 von 2', 'Verbunden'];
const mText = ['Client ID und Tenant eintragen, dann einrichten.', 'In Teams unter Einstellungen, Datenschutz, API verwalten die Schnittstelle für Drittanbieter einschalten.', 'Angemeldet. Mikrofon und Kamera melden ihren echten Zustand als Signal.'];
const mAct = ['Einrichten', 'Erledigt, anmelden', ''];
const flipOff = (id) => { const off = Object.assign({}, s.off); off[id] = !off[id]; this.setState({ off: off }); };
const plugin = (id, detail, fields, status) => ({ id: id, kind: 'Plugin', detail: detail, toggleFn: id === 'core' ? null : () => flipOff(id), active: id === 'core' || !s.off[id], fields: fields, status: status });
const ok = { text: 'Verbunden', color: '#69E0C1' };
const packs = [
plugin('core', 'Apps für Tastenkürzel, Uhr, Pomodoro, Systemmonitor und Musik. Eingebettetes Icon Set. Seine Einstellungen sind die globalen Einstellungen des Decks.', [
{ key: 'hold', label: 'Lang drücken ab', type: 'select', values: [{ label: '300 ms', value: '300' }, { label: '500 ms', value: '500' }, { label: '800 ms', value: '800' }] },
{ key: 'follow', label: 'Profile folgen dem Programm im Vordergrund', type: 'toggle' },
{ key: 'saver', label: 'Bildschirmschoner', type: 'select', values: [{ label: 'Aus', value: '0' }, { label: 'Nach 5 Minuten', value: '5' }, { label: 'Nach 15 Minuten', value: '15' }] }], null),
plugin('com.ricktap.m365', 'Apps Mikrofon und Kamera. Beide erfüllen das Interface toggle.', [
{ key: 'client', label: 'Client ID der App Registrierung', type: 'text', placeholder: '00000000-0000-0000-0000-000000000000' },
{ key: 'tenant', label: 'Tenant', type: 'text', placeholder: 'firma.de' }], { text: mSteps[s.m365], color: s.m365 >= 2 ? '#69E0C1' : '#FFBF69', note: mText[s.m365], action: s.m365 < 2 ? mAct[s.m365] : '', act: () => this.setState({ m365: s.m365 + 1 }) }),
plugin('com.ricktap.hue', 'App Lampe mit eigenem Interface rgb_light. Eingebettetes Icon.', [
{ key: 'bridge', label: 'Bridge', type: 'select', note: 'Die Liste liefert das Plugin zur Laufzeit.', values: [{ label: 'Wohnzimmer', value: 'wohnzimmer' }, { label: 'Büro', value: 'buero' }] }], Object.assign({ note: 'Die Lampe meldet Zustand, Farbe und Helligkeit als Signale.' }, ok)),
plugin('com.ricktap.bambu', 'App AMS mit eigenem Interface ams. Eingebettetes Icon.', [
{ key: 'host', label: 'MQTT Broker', type: 'text', placeholder: 'homeassistant.local' },
{ key: 'topic', label: 'Basis Topic', type: 'text', placeholder: 'd200' }], Object.assign({ note: 'Zwei Geräte melden Füllstand und Farbe ihrer Rollen.' }, ok)),
{ id: 'com.jemand.neon', kind: 'Icon Set', detail: 'Glühbirne, Schalter, Skala, AMS Säulen, Tischlampe. Zeichnet gegen Interfaces, nicht gegen Plugins.', toggleFn: () => flipOff('com.jemand.neon'), active: !s.off['com.jemand.neon'], fields: [], status: null },
{ id: 'com.ricktap.nightsignal', kind: 'Theme', detail: 'Volles Theme. Setzt background, base, muted und accent.', toggleFn: () => this.setState({ theme: 'a' }), active: s.theme === 'a', fields: [], status: null },
{ id: 'com.jemand.bernstein', kind: 'Theme', detail: 'Volles Theme in warmen Tönen.', toggleFn: () => this.setState({ theme: 'b' }), active: s.theme === 'b', fields: [], status: null }
].concat(ovDefs.map((o) => ({ id: o[3], kind: 'Theme, Typ partial', detail: 'Setzt ' + o[2] + '.', toggleFn: () => flipOv(o[0]), active: !!s.ov[o[0]], fields: [], status: null })));
const packages = packs.map((p) => {
const st = p.active ? p.status : null;
return Object.assign({ id: p.id, kind: p.kind, detail: p.detail, switchable: !!p.toggleFn, fixed: !p.toggleFn, on: p.active, state: p.active ? 'aktiv' : 'aus', aria: p.id + (p.active ? ' ausschalten' : ' einschalten'), toggle: p.toggleFn || (() => {}),
hasStatus: !!st, status: st ? st.text : '', statusColor: st ? st.color : '#9AA3B0', statusText: st ? st.note : '', hasAction: !!(st && st.action), actionLabel: st && st.action ? st.action : '', act: st && st.act ? st.act : (() => {}),
fields: p.active ? p.fields.map((f) => field(p.id, f)) : [] }, this.sw(p.active));
});
const autoSw = this.sw(s.autostart);

const rows = [['time.clock', time, ''], ['time.tick', String(g.t), '']];
rows.push(['pomodoro.progress', Math.round(g.pomo) + '%', ''], ['sysmon.cpu', g.cpu + '%', ''], ['media.playing', g.playing ? 'ja' : 'nein', ''], ['media.progress', Math.round(g.media) + '%', '']);
if (on('com.ricktap.m365')) { rows.push(['com.ricktap.m365:mic.muted', g.micMuted ? 'ja' : 'nein', ''], ['com.ricktap.m365:cam.on', g.camOn ? 'ja' : 'nein', '']); }
if (on('com.ricktap.hue')) { rows.push(['com.ricktap.hue:lamp.power', g.lampOn ? 'ja' : 'nein', ''], ['com.ricktap.hue:lamp.color', this.lampColor(), this.lampColor()], ['com.ricktap.hue:lamp.brightness', g.bright + '%', '']); }
if (on('com.ricktap.bambu')) { ['ams1', 'ams2'].forEach((u) => g[u].forEach((v, i) => rows.push(['com.ricktap.bambu:' + u + '.slot' + (i + 1) + '.remaining', Math.round(v) + '%', '']))); rows.push(['com.ricktap.bambu:ams1.slot4.color', 'hsl(' + g.amsHue + ', 80%, 62%)', 'hsl(' + g.amsHue + ', 80%, 62%)']); }
const signalRows = rows.map((r) => ({ name: r[0], value: r[1], swatch: r[2] || 'transparent' }));

const configFields = !selKey ? [] : (selApp.config || []).map((f) => {
const cur = (selK.config && selK.config[f.key]) || f.def;
const open = s.cfgOpen === f.key;
return {
label: f.label, open: open, current: f.values.filter((v) => v.value === cur)[0].label,
note: f.note || ('Typ ' + f.type + ', Schlüssel ' + f.key + ', füllt ' + f.fills),
toggle: () => this.setState({ cfgOpen: open ? null : f.key }),
options: f.values.map((v) => ({ label: v.label, on: v.value === cur, border: v.value === cur ? '#B79AF7' : 'transparent', bg: v.value === cur ? '#1A1626' : 'transparent',
pick: () => { this.mutate((arr) => { arr[s.sel].config = Object.assign({}, arr[s.sel].config, { [f.key]: v.value }); }, f.label + ' steht jetzt auf ' + v.label + '.'); this.setState({ cfgOpen: null }); } }))
};
});
const effId = selKey ? this.effIcon(selK.app, selK.icon) : '';
return {
isDeck: s.view === 'deck', isPackages: s.view === 'packages', isSignals: s.view === 'signals',
nav: nav, undoColor: s.history.length ? '#F4F1E8' : '#9AA3B0',
undo: () => { const h = s.history; if (!h.length) { return; } this.setState({ decks: JSON.parse(h[h.length - 1]), history: h.slice(0, -1), hint: 'Letzte Änderung zurückgenommen.' }); },
libTabs: libTabs, libApps: s.libTab === 'apps', libActions: s.libTab === 'actions', libIcons: s.libTab === 'icons',
appGroups: appGroups, actionGroups: actionGroups, iconChoices: iconChoices, hasIconChoices: iconChoices.length > 0,
iconIntro: selKey ? 'Passend zu ' + selApp.name + '. Jedes Icon läuft schon mit den Werten dieser App. Oben stehen die Icons mit der reichsten gemeinsamen Rolle.' : 'Wähle eine belegte Taste. Dann erscheinen hier alle Icons, die eine Rolle mit ihrer App teilen.',
applyNeon: () => {
if (s.off['com.jemand.neon']) { this.setState({ hint: 'Das Icon Set Neon ist unter Pakete ausgeschaltet.' }); return; }
bulk((k) => { const app = apps[k.app], offered = app.roles.concat(['button']); let best = null, rank = 99; Object.keys(icons).forEach((id) => { if (icons[id].set !== 'com.jemand.neon') { return; } const sh = this.share(icons[id], app); if (!sh.length || sh[0] === 'button') { return; } const r = offered.indexOf(sh[0]) * 10 + icons[id].roles.length; if (r < rank) { rank = r; best = id; } }); return best || k.icon; }, 'Set Neon angewendet, wo ein Icon mehr als button teilt.');
},
applyDefault: () => bulk((k) => apps[k.app].icon, 'Alle Tasten dieser Seite nutzen wieder das Standard Icon ihres Plugins.'),
profiles: profiles, pageTabs: pageTabs, keys: keys, wide: wide, selectWide: () => this.setState({ sel: 13 }), hint: s.hint,
themes: themes, overlays: overlays, curTheme: themes.filter((t) => t.on)[0], themeOpen: s.themeOpen, toggleThemeMenu: () => this.setState({ themeOpen: !s.themeOpen }),
selTitle: s.sel === 13 ? 'Breite Taste' : 'Taste ' + (s.sel + 1),
selEmpty: s.sel !== 13 && !selK, selMissing: selMissing, selKey: selKey, selWide: s.sel === 13,
appName: selKey ? selApp.name : '', appId: selKey ? selK.app : '', appRoles: selKey ? selApp.roles.map((r) => this.short(r)).concat(['button']).join(', ') : '',
thumb: selKey ? this.face(selK.app, selK.icon, s.sel, selK.config) : this.emptyFace(), thumbBg: this.tvar('background', s.sel),
iconId: effId, iconRole: selKey ? this.short(this.share(icons[effId], selApp)[0]) : '',
openIcons: () => this.setState({ libTab: 'icons' }),
selLabel: selKey ? selK.label : '', setLabel: (e) => { const v = e.target.value; this.mutate((arr) => { arr[s.sel].label = v; }); },
sections: sections, hasConfig: configFields.length > 0, configFields: configFields,
testKey: () => {
const names = selK.press.map((a) => { const def = acts[a.id]; if (def.run) { def.run(a.val); } return def.name; });
this.setState({ hint: names.length ? 'Taste ' + (s.sel + 1) + ' gedrückt: ' + names.join(', ') + '.' : 'Diese Taste hat keine Aktion unter Beim Drücken.' });
},
clearKey: () => this.mutate((arr) => { arr[s.sel] = null; }, 'Taste ' + (s.sel + 1) + ' geleert.'),
packages: packages, signalRows: signalRows,
editOpen: s.editProfile, editLabel: s.editProfile ? 'Fertig' : 'Profil bearbeiten', toggleEdit: () => this.setState({ editProfile: !s.editProfile }),
addProfile: () => { let n = s.decks.length + 1; while (s.decks.some((d) => d.name === 'Profil ' + n)) { n += 1; } this.mutateDecks((d) => { d.push({ name: 'Profil ' + n, apps: [], pages: [{ name: 'Main', keys: emptyKeys() }] }); }, 'Profil ' + n + ' angelegt. Gib ihm einen Namen und binde es an ein Programm.', { profile: s.decks.length, page: 0, sel: 0, editProfile: true }); },
pName: prof.name, setPName: (e) => { const v = e.target.value; this.mutateDecks((d) => { d[P].name = v; }); },
bound: bound, noBound: bound.length === 0, candidates: candidates, pageRows: pageRows,
addPage: () => { let n = prof.pages.length + 1; while (prof.pages.some((p) => p.name === 'Seite ' + n)) { n += 1; } this.mutateDecks((d) => { d[P].pages.push({ name: 'Seite ' + n, keys: emptyKeys() }); }, 'Seite ' + n + ' angelegt.', { page: prof.pages.length, sel: 0 }); },
canDeleteProfile: s.decks.length > 1 && P > 0, deleteProfile: () => this.mutateDecks((d) => { d.splice(P, 1); }, 'Profil ' + prof.name + ' gelöscht.', { profile: 0, page: 0, sel: 0 }),
isDefaultProfile: P === 0, fronts: fronts,
autoOn: s.autostart, autoTrack: autoSw.track, autoAlign: autoSw.align, autoState: s.autostart ? 'an' : 'aus', flipAuto: () => this.setState({ autostart: !s.autostart }),
exportCfg: () => this.setState({ appHint: 'deck.json exportiert. Enthält Profile, Belegung und gewählte Themes, aber keine Geheimnisse.' }),
importCfg: () => this.setState({ appHint: 'Hier öffnet sich der Dateidialog zum Importieren einer deck.json.' }), appHint: s.appHint
};
}
}
