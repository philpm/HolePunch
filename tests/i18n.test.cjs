'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const root = path.resolve(__dirname, '..');

function runtime(saved, blocked = false) {
  const values = new Map(saved ? [['holepunch.locale', saved]] : []);
  const events = [];
  const document = {
    documentElement: {lang: ''},
    createTreeWalker: () => ({nextNode: () => false}),
    querySelectorAll: () => [],
    dispatchEvent: event => events.push(event),
  };
  const context = vm.createContext({document, NodeFilter: {SHOW_TEXT: 4}, Intl,
    CustomEvent: class { constructor(type, options) {this.type = type; this.detail = options.detail;} },
    localStorage: {
      getItem(key) {if (blocked) throw Error('Storage unavailable'); return values.get(key);},
      setItem(key, value) {if (blocked) throw Error('Storage unavailable'); values.set(key, value);},
    },
  });
  for (const file of ['web/locales/catalogs.js', 'web/i18n.js']) vm.runInContext(fs.readFileSync(path.join(root, file), 'utf8'), context);
  return {api:context.HPI18n, catalogs:context.HP_MESSAGES, document, events, values};
}

test('English is the default and unsupported persisted locales fall back', () => {
  for (const saved of [undefined, 'unsupported', '__proto__']) {
    const {api, document} = runtime(saved);
    assert.equal(api.locale, 'en');
    assert.equal(document.documentElement.lang, 'en');
    assert.equal(api.t('Save changes'), 'Save changes');
  }
});

test('locale selection persists, sets document language, and emits a change', () => {
  const {api, document, values, events} = runtime();
  api.setLocale('zh-CN');
  assert.equal(values.get('holepunch.locale'), 'zh-CN');
  assert.equal(document.documentElement.lang, 'zh-CN');
  assert.equal(api.t('Save changes'), '\u4fdd\u5b58\u4fee\u6539');
  assert.equal(events.at(-1).type, 'hp:localechange');
  assert.equal(runtime(values.get('holepunch.locale')).api.locale, 'zh-CN');
  api.setLocale('unsupported');
  assert.equal(api.locale, 'zh-CN');
  api.setLocale('en');
  assert.equal(api.t('Save changes'), 'Save changes');
});

test('blocked storage does not prevent switching languages', () => {
  const {api} = runtime(undefined, true);
  api.setLocale('zh-CN');
  assert.equal(api.locale, 'zh-CN');
});

test('missing translations fall back to English and unknown diagnostics remain intact', () => {
  const {api, catalogs} = runtime('zh-CN');
  delete catalogs['zh-CN']['Save changes'];
  assert.equal(api.t('Save changes'), 'Save changes');
  assert.equal(api.t('Unrecognized OS diagnostic'), 'Unrecognized OS diagnostic');
  for (const key of ['__proto__', 'constructor', 'toString']) assert.equal(api.t(key), key);
});

test('interpolation supports reordering without recursively interpreting values', () => {
  const {api, catalogs} = runtime('zh-CN');
  catalogs.en['{0} follows {1}'] = '{0} follows {1}';
  catalogs['zh-CN']['{0} follows {1}'] = '{1} precedes {0}';
  assert.equal(api.t(['', ' follows ', ''], '{1}<device>', 'server'), 'server precedes {1}<device>');
  assert.equal(api.t`Request failed (${401})`, '\u8bf7\u6c42\u5931\u8d25 (401)');
});

test('translated HTML retains already escaped resource names and literal placeholders', () => {
  const {api, catalogs} = runtime('zh-CN');
  const key = Object.keys(catalogs.en).find(k => k.startsWith('<div class="session-row">'));
  assert.ok(key);
  const supplied = ['&lt;script&gt;{2}', '2030-01-01', 'safe'];
  const translated = api.t(key, ...supplied);
  assert.ok(translated.includes('&lt;script&gt;{2}'));
  assert.ok(!translated.includes('<script>'));
});

test('all catalogs have identical keys and numbered placeholder occurrences', () => {
  const english = JSON.parse(fs.readFileSync(path.join(root, 'web/locales/en.json')));
  for (const file of fs.readdirSync(path.join(root, 'web/locales')).filter(f => f.endsWith('.json'))) {
    const messages = JSON.parse(fs.readFileSync(path.join(root, 'web/locales', file)));
    assert.deepEqual(Object.keys(messages).sort(), Object.keys(english).sort(), file);
    for (const [key, value] of Object.entries(messages)) {
      assert.deepEqual((value.match(/\{\d+\}/g) || []).sort(), (key.match(/\{\d+\}/g) || []).sort(), key);
    }
  }
});

test('literal UI messages are cataloged and all HTML entry points load shared assets', () => {
  const english = JSON.parse(fs.readFileSync(path.join(root, 'web/locales/en.json')));
  for (const file of ['web/portal.js','web/legacy/app.js','apps/desktop/ui/app.js']) {
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    for (const match of source.matchAll(/tr\(("(?:[^"\\]|\\.)*")\)/g)) assert.ok(Object.hasOwn(english, JSON.parse(match[1])), `${file}: ${match[1]}`);
  }
  for (const file of ['web/portal.html','web/legacy/index.html','apps/desktop/ui/index.html']) {
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    assert.match(source, /<html lang="en">/);
    assert.match(source, /data-locale/);
    assert.ok(source.indexOf('locales/catalogs.js') < source.indexOf('i18n.js'));
  }
});
