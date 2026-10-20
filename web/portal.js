'use strict';
const {t: tr} = HPI18n;
const $ = selector => document.querySelector(selector);
const $$ = selector => [...document.querySelectorAll(selector)];
const esc = value => String(value ?? '').replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
const mbps = value => value == null ? tr("Unlimited") : `${HPI18n.number(Number(value) / 1e6)} Mbps`;
let capabilities = {multi_user:false,quotas:false,unlimited_bandwidth:true};
const cloud = () => capabilities.multi_user;
const capValue = value => value == null ? '' : value / 1e6;
const inputBps = input => !cloud() && input.value === '' ? null : Math.round(Number(input.value) * 1e6);
const bytes = value => { const n = Number(value || 0); return n >= 1073741824 ? `${(n / 1073741824).toFixed(1)} GB` : n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${(n / 1024).toFixed(1)} KB`; };
const pageLabels = {overview:"Overview",clients:"Clients",tunnels:"Tunnels",bandwidth:"Bandwidth allocation",account:"Account settings",users:"User management",policy:"Default policy",sites:"Local websites",node:"Node settings",audit:"Audit log",domains:"Domain settings",setup:"Setup guide"};
const pages = {...pageLabels};
const adminPages = new Set(['users','policy','sites','node','audit','domains','setup']);
const resourcePages = new Set(['overview','clients','tunnels','bandwidth']);
let actor = null, current = null, csrf = '', managing = '', view = 'overview', mode = 'login';
let devices = [], tunnels = [], users = [], sites = [], dashboard = null;
let lastPageData = null, lastDiagnostics = null;
let setupOffered = false, setupStep = 0, setupData = null, setupChecks = null;
let generation = 0, loading = false, mutating = 0;
const dirtyForms = new Set();

async function api(path, method = 'GET', body, owner = managing) {
  const response = await fetch('/api/v1' + path, {method, credentials:'same-origin', headers:{'Content-Type':'application/json','X-CSRF-Token':csrf,'X-Manage-User':owner}, body:body === undefined ? undefined : JSON.stringify(body)});
  const data = await response.json().catch(() => ({}));
  if (!response.ok) { const error = new Error(tr(data.error) || tr`Request failed (${response.status})`); error.status = response.status; throw error; }
  return data;
}
function notice(text, error = false) {
  const dialog = $('dialog[open]');
  if (dialog) {
    let message = dialog.querySelector('.dialog-notice');
    if (!message) { message = document.createElement('p'); message.className = 'dialog-notice'; message.setAttribute('role','alert'); dialog.prepend(message); }
    message.textContent = text; message.classList.toggle('error', error);
  }
  const target = $('#panel').hidden ? $('#auth-notice') : $('#notice');
  target.textContent = text; target.hidden = false; target.className = error ? 'error' : '';
}
function handleError(error) {
  if (error.status === 401 && actor) { resetSession(); notice(tr("Your session has expired. Please sign in again."), true); }
  else notice(error.message, true);
}
function authMode(next) {
  mode = next; $('#auth-shell').hidden = false; $('#panel').hidden = true; setMenu(false);
  $('#email-field').hidden = ['verify','reset'].includes(mode);
  $('#password-field').hidden = ['verify','reset-request'].includes(mode);
  $('#code-field').hidden = !['verify','reset'].includes(mode);
  const fields = $('#auth-form').elements;
  fields.email.required = !$('#email-field').hidden; fields.password.required = !$('#password-field').hidden; fields.token.required = !$('#code-field').hidden;
  fields.password.value = ''; fields.token.value = '';
  fields.password.autocomplete = mode === 'login' ? 'current-password' : 'new-password';
  $('#auth-title').textContent = $('#auth-submit').textContent = ({login:tr("Sign in"),register:tr("Register"),verify:tr("Verify email"),'reset-request':tr("Send reset email"),reset:tr("Set new password")})[mode];
  $$('[data-mode]').forEach(button => {button.setAttribute('aria-pressed', String(button.dataset.mode === mode));button.hidden=!cloud() && button.dataset.mode!=='login';});
  $('#resend').hidden = mode !== 'verify';
  document.title = `${$('#auth-title').textContent} · HolePunch`;
}
function resetSession() {
  setupOffered = false; setupStep = 0; setupData = setupChecks = null; $('#setup-form').reset();
  setupPasswordFields();
  generation++; actor = current = null; csrf = managing = ''; loading = false; dirtyForms.clear();
  devices = []; tunnels = []; users = []; sites = []; dashboard = null;
  $('#notice').hidden = true; $('#auth-notice').hidden = true;
  ['#users','#tunnels','#devices','#sessions','#audit-list','#site-list','#recent-tunnels'].forEach(selector => $(selector).replaceChildren());
  $$('dialog[open]').forEach(dialog => dialog.close()); $('#password-form').reset(); authMode('login');
}
function routeHref(name, owner = managing) { return '#' + name + (resourcePages.has(name) && owner ? '?user=' + encodeURIComponent(owner) : ''); }
function go(name, owner = managing) {
  const hash = routeHref(name, owner);
  if (location.hash === hash) refresh(true).catch(handleError); else location.hash = hash;
}
function readRoute() {
  const [name, query] = location.hash.slice(1).split('?');
  view = Object.hasOwn(pages, name) ? name : 'overview';
  if ((adminPages.has(view) && actor.user.role !== 'admin') || (!cloud() && ['users','policy'].includes(view)) || (cloud() && view === 'domains')) view = 'overview';
  managing = cloud() && actor.user.role === 'admin' && resourcePages.has(view) ? new URLSearchParams(query || '').get('user') || '' : '';
  if (managing === actor.user.id) managing = '';
  const expected = routeHref(view);
  if (location.hash !== expected) history.replaceState(null, '', expected);
}
function setMenu(open, restoreFocus = false) {
  const mobile = matchMedia('(max-width: 760px)').matches;
  $('#sidebar').classList.toggle('is-open', open && mobile);
  $('#sidebar').inert = mobile && !open;
  $('#workspace').inert = mobile && open;
  $('#nav-backdrop').hidden = !open || !mobile;
  $('#menu-toggle').setAttribute('aria-expanded', String(open && mobile));
  document.body.classList.toggle('nav-open', open && mobile);
  if (open && mobile) $('#sidebar a[aria-current=page]')?.focus();
  else if (restoreFocus && mobile) $('#menu-toggle').focus();
}
function renderShell() {
  for (const key of Object.keys(pages)) pages[key] = tr(pageLabels[key]);
  $('#auth-shell').hidden = true; $('#panel').hidden = false;
  $('#admin-nav').hidden = actor.user.role !== 'admin';
  $('#signed-name').textContent = actor.profile || actor.user.email; $('#signed-name').title = actor.user.email;
  $('#signed-role').textContent = !cloud() ? 'Self-hosted' : actor.user.role === 'admin' ? tr("Cloud · Administrator") : tr("Cloud · Member");
  $$('[data-nav]').forEach(link => {link.hidden=(!cloud() && ['users','policy'].includes(link.dataset.nav)) || (cloud() && link.dataset.nav === 'domains');});
  pages.bandwidth=cloud()?tr("Bandwidth allocation"):tr("Tunnel rate limits");
  $('#tunnel-bandwidth-note').textContent=cloud()?tr("Disabled tunnels retain their allocation. To rebalance bandwidth, open "):tr("Tunnels are unlimited by default. To set individual limits, open ");
  $('#view-tunnels [data-link=bandwidth]').textContent=pages.bandwidth+' →';
  $('[data-nav=bandwidth]').lastChild.textContent=pages.bandwidth;
  $('#page-title').textContent = pages[view]; $('#page-group').textContent = adminPages.has(view) ? tr("Administration") : tr("Workspace");
  document.title = `${pages[view]} · HolePunch`;
  $$('[data-page]').forEach(section => section.hidden = section.dataset.page !== view);
  $$('[data-nav]').forEach(link => { link.href = routeHref(link.dataset.nav); if (link.dataset.nav === view) link.setAttribute('aria-current','page'); else link.removeAttribute('aria-current'); });
  $$('[data-link]').forEach(link => link.href = routeHref(link.dataset.link));
  $('#setup-reminder').hidden = actor.user.role !== 'admin' || actor.setup_complete !== false || view === 'setup';
  $('#manage-banner').hidden = !managing;
  $('#identity').textContent = current?.user.email || '';
  $('#verification').hidden = !resourcePages.has(view) || current?.user.verified !== false;
  $('#overview-title').textContent = managing ? tr("User resources") : tr("My resources");
  $('#host-overview').hidden = view !== 'overview' || actor.user.role !== 'admin' || !!managing;
}
async function refresh(entry = false) {
  const ticket = ++generation; loading = true;
  if (entry) $$('[data-page]').forEach(section => { section.inert = true; });
  $('#workspace').setAttribute('aria-busy', 'true');
  try {
    const signed = await api('/me', 'GET', undefined, '');
    if (ticket !== generation) return;
    actor = signed; capabilities = signed.capabilities || capabilities; csrf = signed.csrf;
    if (signed.user.role === 'admin' && signed.setup_complete === false && !setupOffered) {
      setupOffered = true; history.replaceState(null, '', '#setup');
    }
    readRoute();
    const owner = managing, page = view;
    const resource = resourcePages.has(page);
    const requests = resource ? [owner ? api('/me','GET',undefined,owner) : Promise.resolve(signed), api('/portal/clients','GET',undefined,owner), api('/portal/tunnels','GET',undefined,owner)] : [];
    let extra = Promise.resolve(null);
    if (page === 'overview' && signed.user.role === 'admin' && !owner) extra = api('/dashboard','GET',undefined,'');
    if (page === 'users') extra = api('/admin/users','GET',undefined,'');
    if (page === 'policy') extra = api('/admin/policy','GET',undefined,'');
    if (page === 'setup') extra = api('/setup','GET',undefined,'');
    if (page === 'domains') extra = api('/domains','GET',undefined,'');
    if (page === 'sites') extra = api('/sites','GET',undefined,'');
    if (page === 'node') extra = Promise.all([api('/settings','GET',undefined,''), api('/dashboard','GET',undefined,'')]);
    if (page === 'audit') extra = api('/audit','GET',undefined,'');
    if (page === 'account') extra = api('/me/sessions','GET',undefined,'');
    const [resources, data] = await Promise.all([Promise.all(requests), extra]);
    if (ticket !== generation) return;
    lastPageData = data;
    if (resource) [current, devices, tunnels] = resources; else current = signed;
    renderShell();
    $$('[data-page]').forEach(section => { section.inert = false; });
    if (resource) renderResources(entry);
    if (page === 'overview' && data) { dashboard = data; renderHost(); }
    if (page === 'users') { users = data; renderUsers(); }
    if (page === 'sites') { sites = data; renderSites(); }
    if (page === 'policy' && !dirtyForms.has('policy-form')) {
      const f = $('#policy-form').elements;
      f.clients.value = data.clients; f.tunnels.value = data.tunnels; f.bandwidth.value = Number(data.bps) / 1e6; f.registration.checked = data.registration; f.domain_suffix.value = data.domain_suffix;
    }
    if (page === 'domains' && !dirtyForms.has('domains-form')) $('#domains-form').elements.domain_suffix.value=data.domain_suffix;
    if (page === 'setup') { setupData = data; renderSetup(!dirtyForms.has('setup-form')); }
    if (page === 'node') renderNode(...data);
    if (page === 'audit') renderAudit(data);
    if (page === 'account') renderAccount(data);
    $('#updated').textContent = tr("Updated at ") + new Date().toLocaleTimeString(HPI18n.locale);
  } finally { if (ticket === generation) { loading = false; $('#workspace').setAttribute('aria-busy', 'false'); } }
}
function empty(title, detail = '') { return `<div class="empty"><h2>${esc(title)}</h2><p>${esc(detail)}</p></div>`; }
function table(heads, rows) { return `<div class="table-wrap"><table><thead><tr>${heads.map(head => `<th scope="col">${head}</th>`).join('')}</tr></thead><tbody>${rows}</tbody></table></div>`; }
function status(text, kind = '') { return `<span class="status ${kind}">${esc(text)}</span>`; }
function tunnelStatus(tunnel) { return tunnel.quota_paused ? status(tr("Paused: count limit exceeded"),'warn') : !tunnel.enabled ? status(tr("Disabled")) : tunnel.policy_status === 'applied' ? status(tr("Policy applied"),'good') : status(tr("Policy pending"),'warn'); }
function endpoint(tunnel) {
  if (tunnel.domain) return `<a href="https://${esc(tunnel.domain)}" target="_blank" rel="noreferrer">${esc(tunnel.domain)} ↗</a>`;
  let host = current.public_addr; try { host = new URL('tcp://' + host).hostname; } catch { /* Keep configured address readable. */ }
  return esc(`${host}:${tunnel.public_port}`);
}
function renderResources(entry) {
  $('#device-quota').innerHTML = `${current.client_count} <small>/ ${current.limits.clients ?? tr("Unlimited")}</small>`;
  $('#tunnel-quota').innerHTML = `${current.tunnel_count} <small>/ ${current.limits.tunnels ?? tr("Unlimited")}</small>`;
  $('#bandwidth-quota').textContent = mbps(current.limits.bps); $('#bandwidth-free').textContent = mbps(current.remaining_bps);
  if (view === 'overview') $('#recent-tunnels').innerHTML = tunnels.length ? table([tr("Tunnels"),tr("Public endpoint"),tr("Bandwidth limit"),tr("Status")], tunnels.slice(0,5).map(t => `<tr><td><strong>${esc(t.name)}</strong><small>${esc(t.protocol.toUpperCase())}</small></td><td>${endpoint(t)}</td><td>${mbps(t.bandwidth_limit_bps)}</td><td>${tunnelStatus(t)}</td></tr>`).join('')) : empty(tr("No tunnels yet"),tr("Connect a device in Clients, then publish a local service in Tunnels."));
  if (view === 'clients') {
    $('#clients-summary').textContent = cloud() ? tr`Used ${current.client_count} / ${current.limits.clients} client slots` : tr`${current.client_count} clients · No count limit`;
    $('#client-command').textContent = `holepunch-client connect --server ${location.origin}`;
    $('#devices').innerHTML = devices.length ? table([tr("Clients"),tr("Connection"),tr("Actions")], devices.map(c => `<tr><td><strong>${esc(c.name)}</strong></td><td>${!c.active ? status(tr("Revoked")) : Date.now()/1000 - c.last_seen < 45 ? status(tr("Heartbeat online"),'good') : status(tr("Offline"))}</td><td><div class="row-actions">${c.active ? tr`<button data-enroll="${esc(c.id)}">Generate enrollment code</button><button data-revoke="${esc(c.id)}" class="danger">Revoke</button>` : '—'}</div></td></tr>`).join('')) : empty(tr("Connect your first device"),tr("Select Add client to create a device and receive a one-time enrollment code."));
  }
  if (view === 'tunnels') renderTunnels();
  if (view === 'bandwidth') {
    $('#allocation-limit').textContent = mbps(current.limits.bps);
    if (!dirtyForms.has('allocation-form') || entry) {
      dirtyForms.delete('allocation-form');
      $('#allocation-fields').innerHTML = tunnels.length ? tunnels.map(t => tr`<div class="allocation-row"><div><strong>${esc(t.name)}</strong><small>${esc(t.protocol.toUpperCase())} · ${esc(t.local_host)}:${t.local_port}</small></div><label>Bandwidth limit (Mbps)<input aria-label="${esc(t.name)} Bandwidth limit (Mbps)" data-allocation="${esc(t.id)}" type="number" min="0" step="0.001" value="${capValue(t.bandwidth_limit_bps)}" ${cloud()?'required':tr("placeholder=Unlimited")}></label></div>`).join('') : empty(tr("No tunnels to allocate"),tr("Create a tunnel first, then return to adjust its allocation."));
    }
    $('#allocation-title').textContent=cloud()?tr("Allocate bandwidth per tunnel"):tr("Set optional tunnel rate limits");
    $('#allocation-description').textContent=cloud()?tr("The sum of all tunnel allocations must fit within the user limit. Save all changes together."):tr("Leave a limit blank for unlimited bandwidth, or set a rate to reserve capacity for other applications.");
    if(!cloud()) $('#bandwidth-help').textContent=tr("Blank means unlimited; 0 disables the tunnel. Limits combine both directions and all connections. 1 Mbps = 1,000,000 bit/s. Changes disconnect existing tunnel connections. Re-enable disabled tunnels manually after increasing their limit. Cumulative traffic is not recorded.");
    updateAllocation();
  }
}
function renderTunnels() {
  const query = $('#search').value.trim().toLowerCase(), protocol = $('#protocol-filter').value;
  const rows = tunnels.filter(t => (!protocol || t.protocol === protocol) && [t.name,t.domain].some(value => (value || '').toLowerCase().includes(query)));
  $('#tunnels-summary').textContent = cloud() ? tr`${rows.length} tunnels · Used ${current.tunnel_count} / ${current.limits.tunnels}` : tr`${rows.length} tunnels · No count limit`;
  $('#tunnels').innerHTML = rows.length ? table([tr("Tunnels"),tr("Local service"),tr("Public endpoint"),tr("Bandwidth limit"),tr("Status"),tr("Actions")], rows.map(t => tr`<tr><td><strong>${esc(t.name)}</strong><small>${esc(t.protocol.toUpperCase())}</small></td><td>${esc(t.local_host)}:${t.local_port}</td><td>${endpoint(t)}</td><td>${mbps(t.bandwidth_limit_bps)}</td><td>${tunnelStatus(t)}</td><td><div class="row-actions"><button data-edit="${esc(t.id)}">Edit</button><button data-toggle="${esc(t.id)}">${t.enabled ? tr("Disable") : tr("Enable")}</button><button data-delete="${esc(t.id)}" class="danger">Delete</button></div></td></tr>`).join('')) : empty(tunnels.length ? tr("No matching tunnels") : tr("Publish your first local service"), tunnels.length ? tr("Try another search or protocol.") : tr("Select New tunnel, then choose a client and local port."));
}
function updateAllocation() {
  const fields = $$('[data-allocation]');
  const total = fields.reduce((sum, input) => sum + Math.round(Number(input.value) * 1e6), 0);
  const remaining = cloud() ? current.limits.bps - total : null;
  const valid = fields.every(input => input.validity.valid);
  $('#allocation-total').textContent = !cloud() && fields.some(input=>input.value==='') ? tr("Includes unlimited tunnels") : mbps(total); $('#allocation-remaining').textContent = mbps(remaining);
  $('#allocation-remaining').classList.toggle('danger', remaining < 0);
  $('#allocation-error').textContent = remaining < 0 ? tr`Allocation exceeds the limit by ${mbps(-remaining)}. Reduce it before saving.` : !valid ? tr("Enter a valid, non-negative bandwidth limit.") : '';
  $('#allocation-form button.primary').disabled = !fields.length || remaining < 0 || !valid;
  $('#allocation-dirty').hidden = !dirtyForms.has('allocation-form');
}
function renderUsers() {
  const query = $('#user-search').value.toLowerCase();
  const rows = users.filter(r => r.user.email.toLowerCase().includes(query));
  $('#users-count').textContent = tr`${rows.length} users`;
  $('#users').innerHTML = rows.length ? table([tr("User"),tr("Role / Status"),tr("Clients / Tunnels"),tr("Allocated / Total bandwidth"),tr("Actions")], rows.map(r => tr`<tr><td><strong>${esc(r.user.email)}</strong><small>${r.user.verified ? tr("Email verified") : tr("Email unverified")} · Email: ${esc(r.mail_status || tr("Not sent"))}</small></td><td>${r.user.role === 'admin' ? tr("Administrator") : tr("Member")}<small>${({active:tr("Active"),frozen:tr("Frozen"),deleted:tr("Deleted")})[r.user.status] || esc(r.user.status)}</small></td><td>${r.client_count} / ${r.limits.clients} clients<small>${r.tunnel_count} / ${r.limits.tunnels} tunnels</small></td><td>${mbps(r.allocated_bps)}<small>Total ${mbps(r.limits.bps)}</small></td><td><div class="row-actions"><button data-user="${esc(r.user.id)}" ${r.user.status === 'deleted' ? 'disabled' : ''}>Edit</button><button data-manage="${esc(r.user.id)}" ${r.user.status === 'deleted' ? 'disabled' : ''}>Manage resources</button><button data-invite="${esc(r.user.id)}" ${r.user.status !== 'active' ? 'disabled' : ''}>Resend invitation</button></div></td></tr>`).join('')) : empty(tr("No matching users"),tr("Try another email search."));
}
function renderSites() {
  const query = $('#site-search').value.toLowerCase();
  const rows = sites.filter(s => [s.name,s.domain,String(s.upstream_port)].some(value => value.toLowerCase().includes(query)));
  $('#site-count').textContent = tr`${rows.length} websites`;
  $('#site-list').innerHTML = rows.length ? table([tr("Websites"),tr("HTTPS domain"),tr("Local application"),tr("Status"),tr("Actions")], rows.map(s => tr`<tr><td><strong>${esc(s.name)}</strong></td><td><a href="https://${esc(s.domain)}" target="_blank" rel="noreferrer">${esc(s.domain)} ↗</a></td><td>${esc(s.upstream_host === '::1' ? '[::1]' : s.upstream_host)}:${s.upstream_port}</td><td>${status(s.enabled ? tr("Enabled") : tr("Disabled"),s.enabled ? 'good' : '')}</td><td><div class="row-actions"><button data-site-edit="${esc(s.id)}">Edit</button><button data-site-toggle="${esc(s.id)}">${s.enabled ? tr("Disable") : tr("Enable")}</button><button data-site-delete="${esc(s.id)}" class="danger">Delete</button></div></td></tr>`).join('')) : empty(query ? tr("No matching websites") : tr("Publish local applications over HTTPS"), query ? tr("Try another name or domain.") : tr("Add a domain and a port on this server to manage the public endpoint."));
}
function renderNode(settings, data) {
  if (!dirtyForms.has('settings-form')) for (const key of ['caddy_url','public_addr','tls_ask_url']) $('#settings-form').elements[key].value = settings.config[key];
  $('#port-range').textContent = tr`Public port pool ${settings.port_start}–${settings.port_end}`;
  $('#sync-detail').textContent = tr(data.sync.error) || tr`Desired revision ${data.sync.revision} · Applied revision ${data.sync.applied_revision || '—'}`;
}
function renderAudit(rows) {
  $('#audit-list').innerHTML = rows.length ? table([tr("Time"),tr("Actions"),tr("Target")],rows.map(row => `<tr><td>${esc(new Date(row.created_at * 1000).toLocaleString(HPI18n.locale))}</td><td>${esc(row.action)}</td><td class="audit-target">${esc(row.target)}</td></tr>`).join('')) : empty(tr("No audit events yet"));
}
function renderAccount(rows) {
  $('#account-email').textContent = actor.user.email;
  if (!dirtyForms.has('profile-form')) $('#profile-form').elements.display_name.value = actor.profile || '';
  $('#sessions').innerHTML = rows.map(session => tr`<div class="session-row"><div>${session.current ? tr("Current session") : tr("Other session")}<small>Expires ${esc(new Date(session.expires * 1000).toLocaleString(HPI18n.locale))}</small></div>${session.current ? status(tr("Active"),'good') : tr`<button data-session="${esc(session.id)}">Revoke</button>`}</div>`).join('');
}
function renderHost() {
  $('#host-cpu').textContent = HPI18n.number(dashboard.cpu_percent, {minimumFractionDigits:1,maximumFractionDigits:1}) + '%';
  $('#host-memory').textContent = bytes(dashboard.memory_bytes); $('#host-memory').title = tr("Total memory ") + bytes(dashboard.total_memory_bytes);
  $('#host-clients').textContent = dashboard.online_clients; $('#host-tunnels').textContent = dashboard.running_tunnels;
  $('#sync-status').textContent = ({applied:tr("Configuration applied"),pending:tr("Configuration pending"),error:tr("Synchronization failed")})[dashboard.sync.status];
  $('#sync-status').className = 'badge ' + (dashboard.sync.status === 'applied' ? 'good' : dashboard.sync.status === 'error' ? 'bad' : '');
  drawChart();
}
function drawChart() {
  if (view !== 'overview' || !dashboard || $('#host-overview').hidden) return;
  const canvas = $('#host-chart'), rect = canvas.getBoundingClientRect(); if (!rect.width) return;
  const scale = devicePixelRatio || 1; canvas.width = rect.width * scale; canvas.height = rect.height * scale;
  const ctx = canvas.getContext('2d'), css = getComputedStyle(document.documentElement); ctx.scale(scale,scale);
  ctx.strokeStyle = css.getPropertyValue('--line');
  for (let i = 1; i < 5; i++) { ctx.beginPath(); ctx.moveTo(0,rect.height * i/5); ctx.lineTo(rect.width,rect.height * i/5); ctx.stroke(); }
  const samples = dashboard.traffic, max = Math.max(1024,...samples.flatMap(s => [s.upload_bps,s.download_bps]));
  ['download_bps','upload_bps'].forEach((key,j) => {
    ctx.strokeStyle = css.getPropertyValue(j ? '--muted' : '--accent'); ctx.lineWidth = j ? 1.5 : 2; ctx.setLineDash(j ? [4,4] : []); ctx.beginPath();
    samples.forEach((sample,i) => { const x = rect.width * (60 - samples.length + i) / 59, y = rect.height - 8 - sample[key] / max * (rect.height - 20); if (i === 0) ctx.moveTo(x,y); else ctx.lineTo(x,y); }); ctx.stroke();
  });
  $('#host-upload').textContent = bytes(samples.at(-1)?.upload_bps) + '/s'; $('#host-download').textContent = bytes(samples.at(-1)?.download_bps) + '/s';
}
function enrollment(data) {
  $('#device-form').hidden = true; $('#enrollment').hidden = false;
  $('#connect-command').textContent = `holepunch-client connect --server ${location.origin}`; $('#enroll-code').value = data.enroll_token;
  if (!$('#device-dialog').open) $('#device-dialog').showModal();
}
function renderTunnelDialogText(tunnel) {
  const f = $('#tunnel-form').elements;
  $('#tunnel-title').textContent = tunnel ? tr("Edit tunnel") : tr("New tunnel");
  f.bandwidth.placeholder = cloud() ? '' : tr("Leave blank for unlimited");
  $('#available-bandwidth').textContent = cloud() ? tr`This tunnel can use up to ${mbps(current.remaining_bps + (tunnel?.bandwidth_limit_bps || 0))}; 0 disables it.` : tr("Leave bandwidth blank for unlimited; 0 disables the tunnel. Enter your full domain, or configure a default suffix to generate a random subdomain.");
  $('#domain-field').firstChild.textContent = cloud() ? tr("Full platform domain (blank for random assignment)") : tr("Full custom domain (blank for random assignment when a suffix is configured)");
}
function tunnelDialog(tunnel) {
  if (!devices.some(client => client.active)) { notice(tr("Add a device in Clients first.")); return; }
  const form = $('#tunnel-form'), f = form.elements; form.reset(); f.id.value = tunnel?.id || '';
  f.client_id.innerHTML = devices.filter(client => client.active).map(client => `<option value="${esc(client.id)}">${esc(client.name)}</option>`).join('');
  if (tunnel) { for (const key of ['name','client_id','protocol','local_host','local_port','domain']) f[key].value = tunnel[key] ?? ''; f.enabled.checked = tunnel.enabled; }
  f.bandwidth.value = capValue(tunnel ? tunnel.bandwidth_limit_bps : current.remaining_bps); f.bandwidth.required=cloud();
  renderTunnelDialogText(tunnel);
  f.domain.placeholder = `food-api.${current.domain_suffix || 't.example.com'}`;
  $('#domain-field').hidden = f.protocol.value !== 'web'; $('#tunnel-dialog').showModal();
}
function siteDialog(site) {
  const form = $('#site-form'); form.reset(); form.elements.id.value = site?.id || '';
  if (site) { for (const key of ['name','domain','upstream_host','upstream_port']) form.elements[key].value = site[key]; form.elements.enabled.checked = site.enabled; }
  $('#site-title').textContent = site ? tr("Edit website") : tr("Add website"); $('#site-dialog').showModal();
}
function submit(form, callback) {
  form.addEventListener('submit', async event => {
    event.preventDefault(); if (form.dataset.submitting) return;
    const button = form.querySelector('button.primary'); form.dataset.submitting = 'true'; button.disabled = true; mutating++;
    try { await callback(form.elements); } catch (error) { handleError(error); }
    finally { mutating--; delete form.dataset.submitting; button.disabled = false; if (form.id === 'allocation-form' && current) updateAllocation(); }
  });
}
submit($('#auth-form'), async f => {
  const action = mode, path = ({login:'/auth/login',register:'/auth/register',verify:'/auth/verify','reset-request':'/auth/request-reset',reset:'/auth/reset'})[action];
  const result = await api(path,'POST',{email:f.email.value,password:f.password.value,token:f.token.value},''); f.password.value = ''; f.token.value = '';
  if (action === 'login') { csrf = result.csrf; $('#auth-notice').hidden = true; await refresh(true); }
  else { authMode(action === 'register' ? 'verify' : action === 'reset-request' ? 'reset' : 'login'); notice(action === 'verify' ? tr("Email verified. Please sign in.") : action === 'reset' ? tr("Password updated. Please sign in.") : tr("Email sent. Please check your inbox.")); }
});
submit($('#device-form'), async f => { enrollment(await api('/portal/clients','POST',{name:f.name.value})); await refresh(); });
submit($('#tunnel-form'), async f => {
  const data = {name:f.name.value,client_id:f.client_id.value,protocol:f.protocol.value,local_host:f.local_host.value,local_port:Number(f.local_port.value),domain:f.protocol.value === 'web' ? f.domain.value || null : null,enabled:f.enabled.checked,bandwidth_limit_bps:inputBps(f.bandwidth)};
  await api('/portal/tunnels' + (f.id.value ? '/' + f.id.value : ''), f.id.value ? 'PUT' : 'POST', data);
  $('#tunnel-dialog').close(); await refresh(); notice(tr("Tunnel saved. Applying its policy."));
});
submit($('#allocation-form'), async () => {
  const allocations = $$('[data-allocation]').map(input => ({tunnel_id:input.dataset.allocation,bps:inputBps(input)}));
  await api('/portal/bandwidth','POST',{allocations}); dirtyForms.delete('allocation-form'); await refresh(); notice(tr("Bandwidth allocation saved. Applying changes."));
});
submit($('#policy-form'), async f => {
  if (!confirm(tr("Save the default policy? Users without overrides will inherit the new limits."))) return;
  await api('/admin/policy','PUT',{clients:Number(f.clients.value),tunnels:Number(f.tunnels.value),bps:Math.round(Number(f.bandwidth.value) * 1e6),registration:f.registration.checked,domain_suffix:f.domain_suffix.value},'');
  dirtyForms.delete('policy-form'); await refresh(); notice(tr("Default policy saved."));
});
submit($('#invite-form'), async f => { await api('/admin/users','POST',{email:f.email.value},''); $('#invite-dialog').close(); await refresh(); notice(tr("Invitation sent.")); });
submit($('#user-form'), async f => {
  if (!confirm(f.status.value === 'deleted' ? tr("Permanently delete this user and profile, revoke their devices, and delete their tunnels? This cannot be undone.") : tr("Update this user's role, status, and limits?"))) return;
  const number = key => f[key].value === '' ? null : Number(f[key].value);
  await api('/admin/users/' + f.id.value,'PUT',{status:f.status.value,role:f.role.value,clients:number('clients'),tunnels:number('tunnels'),bps:number('bandwidth') === null ? null : Math.round(number('bandwidth') * 1e6),allocations:$$('[data-user-allocation]').map(input => ({tunnel_id:input.dataset.userAllocation,bps:inputBps(input)}))},'');
  $('#user-dialog').close(); await refresh(); notice(tr("User settings saved."));
});
submit($('#profile-form'), async f => { await api('/me/profile','PUT',{display_name:f.display_name.value},''); dirtyForms.delete('profile-form'); await refresh(); notice(tr("Profile updated.")); });
submit($('#password-form'), async f => { await api('/me/password','PUT',{current_password:f.current_password.value,password:f.password.value},''); resetSession(); notice(tr("Password changed. Please sign in again.")); });
submit($('#site-form'), async f => {
  await api('/sites' + (f.id.value ? '/' + f.id.value : ''),f.id.value ? 'PUT' : 'POST',{name:f.name.value,domain:f.domain.value,upstream_host:f.upstream_host.value,upstream_port:Number(f.upstream_port.value),enabled:f.enabled.checked},'');
  $('#site-dialog').close(); await refresh(); notice(tr("Website saved. Synchronizing configuration."));
});
submit($('#domains-form'), async f => {await api('/domains','PUT',{domain_suffix:f.domain_suffix.value},'');dirtyForms.delete('domains-form');await refresh();notice(tr("Domain settings saved."));});
submit($('#settings-form'), async f => {
  await api('/settings','PUT',{caddy_url:f.caddy_url.value,public_addr:f.public_addr.value,tls_ask_url:f.tls_ask_url.value},'');
  dirtyForms.delete('settings-form'); await refresh(); notice(tr("Node settings saved."));
});

document.addEventListener('input', event => {
  const form = event.target.closest('form'); if (form && ['allocation-form','policy-form','settings-form','profile-form','domains-form','setup-form'].includes(form.id)) dirtyForms.add(form.id);
  if (event.target.matches('[data-allocation]')) updateAllocation();
});
document.addEventListener('click', async event => {
  const link = event.target.closest('[data-nav], [data-link]');
  if (link) setMenu(false);
  const button = event.target.closest('button'); if (!button || button.disabled) return;
  const data = button.dataset;
  if (!Object.keys(data).length) return;
  button.disabled = true; mutating++;
  try {
    if (data.mode) authMode(data.mode);
    if (data.close) $('#' + data.close).close();
    if (data.manage) go('clients',data.manage);
    if (data.enroll) enrollment(await api('/portal/clients/' + data.enroll + '/enrollment','POST',{}));
    if (data.revoke && confirm(tr("Permanently revoke this client and delete all its tunnels? This releases its count and bandwidth allocations."))) { await api('/portal/clients/' + data.revoke + '/revoke','POST',{}); await refresh(); }
    if (data.edit) tunnelDialog(tunnels.find(t => t.id === data.edit));
    if (data.toggle) {
      const tunnel = tunnels.find(t => t.id === data.toggle); if (!tunnel.enabled && tunnel.bandwidth_limit_bps === 0) throw new Error(tr("Edit the tunnel and allocate bandwidth first."));
      await api('/portal/tunnels/' + tunnel.id,'PUT',{...tunnel,enabled:!tunnel.enabled}); await refresh();
    }
    if (data.delete && confirm(tr("Delete this tunnel and release its bandwidth allocation?"))) { await api('/portal/tunnels/' + data.delete,'DELETE'); await refresh(); }
    if (data.invite) { await api('/admin/users/' + data.invite + '/invitation','POST',{},''); await refresh(); notice(tr("A new password setup email has been sent.")); }
    if (data.session) { await api('/me/sessions/' + data.session,'DELETE',undefined,''); await refresh(); }
    if (data.user) {
      const row = users.find(r => r.user.id === data.user), f = $('#user-form').elements;
      f.id.value = row.user.id; f.role.value = row.user.role; f.status.value = row.user.status; f.clients.value = row.overrides?.clients ?? ''; f.tunnels.value = row.overrides?.tunnels ?? ''; f.bandwidth.value = row.overrides?.bps == null ? '' : row.overrides.bps / 1e6;
      const rows = await api('/portal/tunnels','GET',undefined,row.user.id);
      $('#user-allocations').innerHTML = rows.length ? rows.map(t => `<label>${esc(t.name)}（Mbps）<input data-user-allocation="${esc(t.id)}" type="number" min="0" step="0.001" value="${capValue(t.bandwidth_limit_bps)}" ${cloud()?'required':tr("placeholder=Unlimited")}></label>`).join('') : tr("<p>This user has no tunnels yet.</p>");
      $('#user-email').textContent = row.user.email; $('#user-dialog').showModal();
    }
    if (data.siteEdit) siteDialog(sites.find(site => site.id === data.siteEdit));
    if (data.siteToggle) { const site = sites.find(s => s.id === data.siteToggle); await api('/sites/' + site.id,'PUT',{...site,enabled:!site.enabled},''); await refresh(); }
    if (data.siteDelete && confirm(tr("Delete this website's public endpoint?"))) { await api('/sites/' + data.siteDelete,'DELETE',undefined,''); await refresh(); }
  } catch (error) { handleError(error); }
  finally { mutating--; button.disabled = false; }
});
function action(id, callback) {
  $(id).addEventListener('click', async () => {
    const button = $(id); if (button.disabled) return; button.disabled = true; mutating++;
    try { await callback(); } catch (error) { handleError(error); }
    finally { button.disabled = false; mutating--; }
  });
}
action('#signout', async () => { await api('/auth/logout','POST',{},''); resetSession(); });
action('#refresh', () => refresh());
action('#resend', async () => { await api('/auth/resend','POST',{email:$('#auth-form').elements.email.value},''); notice(tr("Please check your verification email.")); });
action('#copy-enroll', async () => { await navigator.clipboard.writeText($('#enroll-code').value); notice(tr("Enrollment code copied.")); });
action('#sync-now', async () => { await api('/sync','POST',{},''); await refresh(); notice(tr("Configuration synchronization requested.")); });
action('#rotate-key', async () => { if (confirm(tr("Rotate the certificate authorization key and synchronize Caddy again?"))) { await api('/settings/rotate-ask-key','POST',{},''); await refresh(); notice(tr("Authorization key rotated.")); } });
action('#diagnose', async () => {
  lastDiagnostics = await api('/diagnostics','GET',undefined,''); renderDiagnostics();
});
function renderDiagnostics() {
  if (!lastDiagnostics) return;
  const data = lastDiagnostics; $('#diagnostic-result').hidden = false;
  $('#diagnostic-result').textContent = tr`v${data.version}\nConfiguration: ${data.config_synchronized ? tr("Synchronized") : tr("Not ready")}\nrathole control port: ${data.rathole_control_listener_reachable ? tr("Reachable") : tr("Unreachable")}\n${tr(data.note)}`;
}
$('#go-verify').onclick = () => { $('#auth-form').elements.email.value = actor.user.email; authMode('verify'); };
$('#end-manage').onclick = () => go('users','');
$('#add-device').onclick = () => { $('#device-form').reset(); $('#device-form').hidden = false; $('#enrollment').hidden = true; $('#device-dialog').showModal(); };
$('#add-tunnel').onclick = () => tunnelDialog();
$('#new-site').onclick = () => siteDialog();
$('#invite-user').onclick = () => { $('#invite-form').reset(); $('#invite-dialog').showModal(); };
$('#tunnel-form').elements.protocol.onchange = () => $('#domain-field').hidden = $('#tunnel-form').elements.protocol.value !== 'web';
$('#search').oninput = renderTunnels; $('#protocol-filter').onchange = renderTunnels;
$('#user-search').oninput = renderUsers; $('#site-search').oninput = renderSites;
$('#menu-toggle').onclick = () => setMenu(!$('#sidebar').classList.contains('is-open'));
$('#nav-backdrop').onclick = () => setMenu(false,true);
document.addEventListener('keydown', event => {
  if (!$('#sidebar').classList.contains('is-open')) return;
  if (event.key === 'Escape') { event.preventDefault(); setMenu(false,true); }
  if (event.key === 'Tab') {
    const focusable = [...$('#sidebar').querySelectorAll('a,button')].filter(element => element.getClientRects().length);
    const first = focusable[0], last = focusable.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
  }
});
window.addEventListener('resize', () => { setMenu($('#sidebar').classList.contains('is-open')); drawChart(); });
window.addEventListener('hashchange', () => {
  if (!actor) return;
  $('#notice').hidden = true; dirtyForms.clear(); $('#password-form').reset();
  $$('dialog[open]').forEach(dialog => dialog.close());
  refresh(true).then(() => { $('#page-title').focus({preventScroll:true}); window.scrollTo(0,0); }).catch(handleError);
});
$$('dialog').forEach(dialog => dialog.addEventListener('close', () => dialog.querySelector('.dialog-notice')?.remove()));
$('#device-dialog').addEventListener('close', () => { $('#enroll-code').value = ''; });
setMenu(false);
(async()=>{capabilities=await api('/capabilities','GET',undefined,'');authMode('login');await refresh(true);})().catch(error=>{if(error.status!==401)handleError(error);});
setInterval(() => {
  if (actor && !$('#panel').hidden && !loading && !mutating && !document.hidden && !$('dialog[open]') && !document.activeElement?.matches('input,select,textarea') && !dirtyForms.size && ['overview','clients','tunnels','users','sites'].includes(view)) refresh().catch(handleError);
},5000);

// Re-render translated application text without resetting open forms or their values.
document.addEventListener('hp:localechange', () => {
  for (const node of $$('#notice,#auth-notice,.dialog-notice')) node.textContent = HPI18n.translateExisting(node.textContent);
  $('#updated').textContent = tr('Updated at ') + new Date().toLocaleTimeString(HPI18n.locale);
  if (!actor || $('#panel').hidden) {
    $('#auth-title').textContent = $('#auth-submit').textContent = tr(({login:'Sign in',register:'Register',verify:'Verify email','reset-request':'Send reset email',reset:'Set new password'})[mode]);
    document.title = `${$('#auth-title').textContent} · HolePunch`;
    return;
  }
  renderShell();
  if (resourcePages.has(view)) {
    const draft = new Map($$('[data-allocation]').map(input => [input.dataset.allocation,input.value]));
    const dirty = dirtyForms.has('allocation-form');
    renderResources(true);
    if (dirty) {
      $$('[data-allocation]').forEach(input => {if(draft.has(input.dataset.allocation))input.value=draft.get(input.dataset.allocation);});
      dirtyForms.add('allocation-form'); updateAllocation();
    }
  }
  if (view === 'setup') renderSetup(false);
  if (view === 'users') renderUsers();
  if (view === 'sites') renderSites();
  if (view === 'account' && lastPageData) renderAccount(lastPageData);
  if (view === 'audit' && lastPageData) renderAudit(lastPageData);
  if (view === 'node' && lastPageData) {renderNode(...lastPageData); renderDiagnostics();}
  if (view === 'overview' && dashboard) renderHost();
  if ($('#tunnel-dialog').open) renderTunnelDialogText(tunnels.find(tunnel => tunnel.id === $('#tunnel-form').elements.id.value));
  if ($('#site-dialog').open) $('#site-title').textContent=tr($('#site-form').elements.id.value ? 'Edit website' : 'Add website');
});

function setupPasswordFields() {
  const enabled = $('#setup-change-password').checked;
  $('#setup-password-fields').hidden = !enabled;
  for (const input of $$('#setup-password-fields input')) {
    input.disabled = !enabled; input.required = enabled;
    if (!enabled) input.value = '';
  }
}
function renderSetup(hydrate = false) {
  if (!setupData) return;
  const f = $('#setup-form').elements;
  if (hydrate) {
    for (const key of ['email','display_name','public_addr','domain_suffix']) f[key].value = setupData[key] || '';
  }
  setupPasswordFields();
  $$('[data-setup-step]').forEach(section => section.hidden = Number(section.dataset.setupStep) !== setupStep);
  $$('[data-setup-progress]').forEach(item => {
    if (Number(item.dataset.setupProgress) === setupStep) item.setAttribute('aria-current','step');
    else item.removeAttribute('aria-current');
  });
  $('#setup-back').hidden = setupStep === 0;
  $('#setup-next').hidden = setupStep === 3;
  $('#setup-finish').hidden = setupStep !== 3;
  $('#setup-step-label').textContent = tr`Step ${setupStep + 1} of 4`;
  $('#setup-cloud-next').hidden = !cloud();
  $('#setup-domain-help').textContent = cloud() ? tr('Configure the platform domain suffix before creating web tunnels. Users can choose names within this suffix or leave the domain blank for random assignment.') : tr('For random tunnel domains, point *.t.example.com at this server and enter t.example.com here. Leave it blank to use individual custom domains or TCP/UDP tunnels.');
  $('#setup-panel-url').textContent = location.origin;
  $('#setup-panel-domain').textContent = setupData.panel_domain || tr('Not configured; use the local panel through SSH.');
  $('#setup-ports').textContent = tr`Control listener: ${setupData.control_bind}. HTTP: ${setupData.http_port}. HTTPS: ${setupData.https_listen}. Public tunnel range: ${setupData.port_start}–${setupData.port_end} (open only the TCP/UDP ports you use).`;
  $('#setup-connect-command').textContent = `holepunch-client connect --server ${location.origin}`;
  $('#setup-summary').textContent = tr`Administrator: ${f.email.value}. Public control address: ${f.public_addr.value}. Domain suffix: ${f.domain_suffix.value || tr('None')}.`;
  renderSetupChecks();
}
function renderSetupChecks() {
  const target = $('#setup-check-result');
  target.replaceChildren();
  if (!setupChecks) { target.textContent = tr('Checks have not run yet.'); return; }
  for (const [label, ok] of [[tr('Configuration synchronization'), setupChecks.config_synchronized], [tr('rathole control listener'), setupChecks.rathole_control_listener_reachable]]) {
    const row = document.createElement('p'); row.textContent = `${label}: ${ok ? tr('Ready') : tr('Not ready')}`; target.append(row);
  }
}
function validateSetupStep(step) {
  const fields = $$(`[data-setup-step="${step}"] input`).filter(input => !input.disabled);
  for (const field of fields) if (!field.reportValidity()) return false;
  if (step === 0 && $('#setup-change-password').checked) {
    const f = $('#setup-form').elements;
    if (f.password.value !== f.confirm_password.value) { notice(tr('The new passwords do not match.'), true); f.confirm_password.focus(); return false; }
  }
  return true;
}
function showSetupStep(step) {
  setupStep = step; renderSetup(false);
  $(`[data-setup-step="${step}"] h2`).focus();
}
$('#setup-change-password').onchange = setupPasswordFields;
$('#setup-back').onclick = () => showSetupStep(Math.max(0, setupStep - 1));
$('#setup-next').onclick = () => { if (validateSetupStep(setupStep)) showSetupStep(Math.min(3, setupStep + 1)); };
$('#setup-later').onclick = () => go('overview','');
action('#setup-check', async () => { setupChecks = await api('/diagnostics','GET',undefined,''); renderSetupChecks(); });
submit($('#setup-form'), async f => {
  if (setupStep !== 3) { $('#setup-next').click(); return; }
  for (let step = 0; step < 2; step++) {
    // Show invalid fields before reporting validity, so the browser can focus them.
    const invalid = $$(`[data-setup-step="${step}"] input`).some(input => !input.disabled && !input.checkValidity());
    if (invalid) { showSetupStep(step); validateSetupStep(step); return; }
  }
  if ($('#setup-change-password').checked && f.password.value !== f.confirm_password.value) { showSetupStep(0); validateSetupStep(0); return; }
  const result = await api('/setup','PUT',{display_name:f.display_name.value,public_addr:f.public_addr.value,domain_suffix:f.domain_suffix.value,current_password:f.current_password.value,password:f.password.value},'');
  f.current_password.value = f.password.value = f.confirm_password.value = '';
  dirtyForms.delete('setup-form');
  history.replaceState(null, '', '#clients');
  if (result.login_required) { resetSession(); notice(tr('Setup saved. Sign in with your new password to connect a client.')); }
  else { await refresh(true); notice(tr('Setup saved. Add a client to get its one-time enrollment code.')); }
});
