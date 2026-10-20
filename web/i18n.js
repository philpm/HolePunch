/* Shared browser and Tauri localization. English source messages are catalog keys. */
'use strict';
(() => {
  const catalogs = globalThis.HP_MESSAGES;
  const supported = Object.keys(catalogs);
  const storageKey = 'holepunch.locale';
  let locale = 'en';
  try { const saved = localStorage.getItem(storageKey); if (supported.includes(saved)) locale = saved; } catch { /* Storage can be disabled in private webviews. */ }
  const staticBindings = [];
  const renderedMessages = new Map();
  function t(message, ...values) {
    const key = Array.isArray(message) ? message.map((part, i) => part + (i < values.length ? `{${i}}` : '')).join('') : message;
    const translated = Object.hasOwn(catalogs[locale], key) ? catalogs[locale][key] : Object.hasOwn(catalogs.en, key) ? catalogs.en[key] : key;
    // A single pass prevents interpolated user content from becoming another placeholder.
    const result = values.length ? translated.replace(/\{(\d+)\}/g, (match, index) => index < values.length ? String(values[index]) : match) : translated;
    if (Object.hasOwn(catalogs.en, key)) {
      renderedMessages.set(result, {key, values});
      if (renderedMessages.size > 512) renderedMessages.delete(renderedMessages.keys().next().value);
    }
    return result;
  }
  function translateExisting(message) {
    const previous = renderedMessages.get(message);
    return previous ? t(previous.key, ...previous.values) : t(message);
  }
  function apply() {
    document.documentElement.lang = locale;
    for (const binding of staticBindings) {
      const current = binding.attribute ? binding.node.getAttribute(binding.attribute) : binding.node.data;
      // Application-rendered content takes precedence over the initial HTML text.
      if (current !== binding.previous) continue;
      const next = binding.leading + t(binding.key) + binding.trailing;
      if (binding.attribute) binding.node.setAttribute(binding.attribute, next); else binding.node.data = next;
      binding.previous = next;
    }
    document.querySelectorAll('[data-locale]').forEach(select => { select.value = locale; select.setAttribute('aria-label', t('Language')); });
    document.querySelectorAll('[data-language-label]').forEach(label => label.textContent = t('Language'));
  }
  function setLocale(next) {
    if (!supported.includes(next)) return;
    locale = next;
    try { localStorage.setItem(storageKey, locale); } catch { /* Localization also works without persistence. */ }
    apply();
    document.dispatchEvent(new CustomEvent('hp:localechange', {detail: {locale}}));
  }
  function capture(node, attribute = null) {
    const value = attribute ? node.getAttribute(attribute) : node.data;
    if (!value) return;
    const key = value.trim();
    if (!Object.hasOwn(catalogs.en, key)) return;
    staticBindings.push({node, attribute, key, leading: value.match(/^\s*/)[0], trailing: value.match(/\s*$/)[0], previous: value});
  }
  globalThis.HPI18n = {t, translateExisting, setLocale, get locale() { return locale; }, number: (value, options) => new Intl.NumberFormat(locale, options).format(value)};
  // Deferred scripts execute after parsing. Capture static markup before app scripts render data.
  const walker = document.createTreeWalker(document.documentElement, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) {
    const node = walker.currentNode;
    if (!node.parentElement.closest('script,style,textarea,pre,[data-locale]')) capture(node);
  }
  document.querySelectorAll('[title],[aria-label],[placeholder]').forEach(node => {
    for (const attr of ['title', 'aria-label', 'placeholder']) capture(node, attr);
  });
  document.querySelectorAll('[data-locale]').forEach(select => select.addEventListener('change', () => setLocale(select.value)));
  apply();
})();
