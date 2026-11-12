// UI test: drives the configurator in Chromium against the real engine (examples/uibridge).
// Run with tests/ui/run.sh.
const { chromium } = require('playwright');
const fs = require('fs');
(async () => {
  const [DECK, OUT, URL] = process.argv.slice(2);
  const cfg = () => JSON.parse(fs.readFileSync(DECK, 'utf8'));
  let failed = 0;
  const page0 = () => { const c = cfg(); return c.profiles[0].pages[Object.keys(c.profiles[0].pages)[0]]; };
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1440, height: 940 } });
  const errs = [];
  p.on('pageerror', e => errs.push('PAGEERROR ' + e.message));
  p.on('console', m => { if (m.type() === 'error' && !m.text().startsWith('Failed to load')) errs.push(m.text()); });
  const ok = (c, msg) => { if (!c) failed++; console.log((c ? 'ok   ' : 'FAIL ') + msg); };
  const wait = (ms = 900) => p.waitForTimeout(ms);
  await p.goto(URL); await wait(2000);

  // 1. tap an app: Weather on key 7
  await p.click('.key[data-key="7"]'); await wait(300);
  await p.click('.item:has-text("Weather")'); await wait();
  ok(page0()['7']?.app === 'com.deckling.core:weather', 'tap places Weather on key 7: ' + page0()['7']?.app);
  ok((await p.textContent('#hint')).includes('Weather'), 'hint says so');

  // 2. drag an app onto key 8
  await p.dragAndDrop('.item:has-text("Pomodoro")', '.key[data-key="8"]'); await wait();
  ok(page0()['8']?.app === 'com.deckling.core:pomodoro', 'drag places Pomodoro on key 8: ' + page0()['8']?.app);

  // 3. swap keys 7 and 8 by dragging
  await p.dragAndDrop('.key[data-key="7"]', '.key[data-key="8"]'); await wait();
  ok(page0()['7']?.app === 'com.deckling.core:pomodoro' && page0()['8']?.app === 'com.deckling.core:weather', 'drag swaps keys 7 and 8');

  // 4. add an action to Long press of key 7
  await p.click('.key[data-key="7"]'); await wait(300);
  await p.click('.sec-head:has-text("Long press")'); await wait(300);
  await p.click('#lib-tabs [data-tab="actions"]'); await wait(300);
  await p.click('.item:has-text("Run a command"), .item:has-text("Command")'); await wait();
  const lp = page0()['7']?.states?.[0]?.on?.long || [];
  ok(lp.some(a => a.action === 'com.deckling.core:command'), 'action lands in Long press: ' + JSON.stringify(lp));

  // 5. edit its parameter
  const inp = p.locator('.sec.target .act input').first();
  await inp.fill('echo hi'); await wait(1200);
  const lp2 = page0()['7']?.states?.[0]?.on?.long || [];
  ok(JSON.stringify(lp2).includes('echo hi'), 'parameter saved: ' + JSON.stringify(lp2));

  // 6. undo twice: parameter, then action
  await p.click('#btn-undo'); await wait(); await p.click('#btn-undo'); await wait();
  ok(!(page0()['7']?.states?.[0]?.on?.long || []).length, 'undo removes the action');

  // 7. label
  await p.fill('#insp textarea', 'Focus'); await wait(1200);
  ok(page0()['7']?.states?.[0]?.label === 'Focus', 'label saved');

  // 8. clear key 8
  await p.click('.key[data-key="8"]'); await wait(300);
  await p.click('#insp button:has-text("Clear key")'); await wait();
  ok(!page0()['8'], 'clear key 8');

  // 9. icon: pick a non-default icon for key 7 (pomodoro)
  await p.click('.key[data-key="7"]'); await wait(300);
  await p.click('#lib-tabs [data-tab="icons"]'); await wait(800);
  await p.locator('.icon-cell').nth(1).click(); await wait();
  ok(!!page0()['7']?.states?.[0]?.icon, 'icon chosen: ' + page0()['7']?.states?.[0]?.icon);
  await p.click('.box button:has-text("Restore default icons")'); await wait();
  ok(!page0()['7']?.states?.[0]?.icon, 'restore default icons');

  // 10. switch off the Teams plugin: its keys show Plugin missing
  await p.click('.key[data-key="9"]'); await wait(300);
  await p.click('#lib-tabs [data-tab="apps"]'); await wait(300);
  await p.click('.item:has-text("Teams mic")'); await wait();
  await p.click('#side [data-view="packages"]'); await wait();
  await p.locator('.pkg:has-text("com.deckling.teams") input.switch').click(); await wait();
  ok(cfg().plugins['com.deckling.teams'].enabled === false, 'teams switched off');
  await p.click('#side [data-view="deck"]'); await wait(1200);
  const missing = await p.locator('.key.missing').count();
  ok(missing > 0, `keys with Plugin missing: ${missing}`);
  await p.screenshot({ path: OUT + '/7-missing.png' });
  await p.click('#side [data-view="packages"]'); await wait();
  await p.locator('.pkg:has-text("com.deckling.teams") input.switch').click(); await wait();

  // 11. packages: change a setting
  await p.locator('.pkg:has-text("Core apps") button:has-text("Settings")').click(); await wait(500);
  await p.screenshot({ path: OUT + '/8-coreapps.png', fullPage: true });
  const work = p.locator('.pkg:has-text("Core apps") label:has-text("Focus") input');
  await work.fill('30'); await wait(1200);
  ok(cfg().plugins['com.deckling.core'].config.pomodoro.work === 30, 'pomodoro work saved: ' + cfg().plugins['com.deckling.core'].config.pomodoro.work);

  // 12. profile: add, rename, bind a program, add page
  await p.click('#side [data-view="deck"]'); await wait();
  const before = cfg().profiles.length;
  await p.click('#pf-add'); await wait(1200);
  ok(cfg().profiles.length === before + 1, 'profile added');
  const name = p.locator('#pf-editor input').first();
  await name.fill('Meeting'); await name.press('Tab'); await wait(1000);
  ok(cfg().profiles.at(-1).name === 'Meeting', 'profile renamed');
  await p.click('#pf-editor .chip.add >> nth=0'); await wait(1000);
  ok(cfg().profiles.at(-1).apps.length === 1, 'program bound: ' + cfg().profiles.at(-1).apps);
  await p.click('#pf-editor button:has-text("+ Page")'); await wait(1500);
  ok(Object.keys(cfg().profiles.at(-1).pages).length === 2, 'page added');
  await p.screenshot({ path: OUT + '/9-newprofile.png' });

  // 13. a button with two states, emoji, colours
  await p.click('#sel-profile'); await p.selectOption('#sel-profile', '0'); await wait(1200);
  await p.click('.key[data-key="12"]'); await wait(300);
  await p.click('#lib-tabs [data-tab="apps"]'); await wait(300);
  await p.click('.item:has-text("Button")'); await wait();
  await p.click('#insp button:has-text("+ State")'); await wait();
  const k12 = () => page0()['12'];
  ok((k12()?.states || []).length === 2, 'button has two states');
  ok((k12()?.states || []).every(st => JSON.stringify(st.on || {}).includes('core:state')), 'both states switch to the next');
  await p.click('#lib-tabs [data-tab="icons"]'); await wait(800);
  await p.click('.emoji-grid button >> nth=3'); await wait();
  ok(k12()?.states?.[1]?.icon === '🎵', 'emoji on state 2: ' + k12()?.states?.[1]?.icon);
  await p.locator('#insp .box:has-text("Colours") input[type=color]').first().evaluate((el) => { el.value = '#334455'; el.dispatchEvent(new Event('input')); }); await wait(1200);
  ok(k12()?.states?.[1]?.bg === '#334455', 'background colour of state 2');

  // 14. a theme variable bound to a signal
  await p.selectOption('#theme select >> nth=-1', 'accent'); await wait(800);
  await p.locator('#theme .box:has-text("accent") select').first().selectOption('signal'); await wait(1200);
  const acc = cfg().plugins.core.config.theme.variables.accent;
  ok(acc && typeof acc === 'object' && !!acc.bind, 'accent bound to a signal: ' + JSON.stringify(acc));
  await p.screenshot({ path: OUT + '/11-states-theme.png', fullPage: true });

  // 15. themes: a full theme changes the keys; overlays switch on in order
  const imgBefore = await p.locator('.key[data-key="0"] img').getAttribute('src');
  await p.click('#theme .theme-pick'); await wait(300);
  await p.click('#theme .theme-menu .theme-pick:has-text("Amber")'); await wait(2000);
  ok(cfg().theme === 'com.deckling.amber', 'theme Amber: ' + cfg().theme);
  const after = await p.locator('.key[data-key="0"] img').getAttribute('src');
  ok(imgBefore !== after, 'the keys are drawn in the new theme');
  await p.locator('#theme .overlay:has-text("Rainbow") input.switch').click(); await wait(800);
  await p.locator('#theme .overlay:has-text("CPU heat") input.switch').click(); await wait(800);
  ok(JSON.stringify(cfg().overlays) === '["com.deckling.rainbow","com.deckling.cpu-heat"]', 'overlays in order: ' + JSON.stringify(cfg().overlays));
  await p.locator('#theme .overlay:has-text("CPU heat") button[title="Up"]').click(); await wait(800);
  ok(JSON.stringify(cfg().overlays) === '["com.deckling.cpu-heat","com.deckling.rainbow"]', 'overlay moved up: ' + JSON.stringify(cfg().overlays));

  // 16. a theme package from a folder: added, listed, used, removed
  const pkgSrc = OUT + '/ocean';
  fs.mkdirSync(pkgSrc, { recursive: true });
  fs.writeFileSync(pkgSrc + '/deckling.json', JSON.stringify({ id: 'com.test.ocean', type: 'theme', name: 'Ocean', theme: { type: 'full', variables: { background: '#0A1A2A', base: '#E0F0FF', muted: '#335577', accent: '#33CCFF' } } }));
  await p.click('#side [data-view="packages"]'); await wait();
  await p.evaluate((path) => window.__TAURI__.core.invoke('install_package', { path }), pkgSrc); await wait(1800);
  ok(await p.locator('.pkg:has-text("com.test.ocean")').count() === 1, 'the new theme is listed under Packages');
  await p.locator('.pkg:has-text("com.test.ocean") input.switch').click(); await wait(1000);
  ok(cfg().theme === 'com.test.ocean', 'the new theme is active');
  p.once('dialog', () => {});
  await p.locator('.pkg:has-text("com.test.ocean") button:has-text("Remove")').click(); await wait(400);
  await p.click('#dlg-prompt #prompt-ok'); await wait(1800);
  ok(await p.locator('.pkg:has-text("com.test.ocean")').count() === 0, 'the theme is removed');
  await p.screenshot({ path: OUT + '/12-packages.png', fullPage: true });
  await p.click('#side [data-view="deck"]'); await wait(1200);
  ok((await p.locator('#theme .theme-pick').first().textContent()).includes('missing'), 'the deck says the theme is missing');

  // 17. web API: generate a token, switch the server on, see where it listens
  await p.click('#side [data-view="packages"]'); await wait();
  await p.locator('.pkg:has-text("com.deckling.web") button:has-text("Settings")').click(); await wait(500);
  await p.locator('.pkg:has-text("com.deckling.web") button:has-text("Generate")').click(); await wait(1200);
  const tok = cfg().plugins['com.deckling.web']?.config?.token || '';
  ok(/^[0-9a-f]{32}$/.test(tok), 'token generated: ' + tok.length + ' hex digits');
  await p.locator('.pkg:has-text("com.deckling.web") label:has-text("Run the web server") input').click(); await wait(2500);
  ok(cfg().plugins['com.deckling.web'].config.serve === true, 'web server switched on');
  await p.click('#side [data-view="deck"]'); await wait(300);
  await p.click('#side [data-view="packages"]'); await wait(800);
  const st = await p.locator('.pkg:has-text("com.deckling.web") .status').textContent();
  ok(/Listening on port \d+/.test(st), 'status: ' + st.slice(0, 60));
  await p.locator('.pkg:has-text("com.deckling.web") label:has-text("Run the web server") input').click(); await wait(800);
  await p.screenshot({ path: OUT + '/13-web.png', fullPage: true });

  // narrow window
  await p.setViewportSize({ width: 820, height: 1000 }); await wait(500);
  await p.screenshot({ path: OUT + '/10-narrow.png', fullPage: true });
  if (errs.length) { failed++; console.log(errs.join('\n')); }
  await b.close();
  console.log(failed ? `${failed} failed` : 'all passed');
  process.exit(failed ? 1 : 0);
})();
