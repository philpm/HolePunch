'use strict';
const {t: tr} = HPI18n;
const $=s=>document.querySelector(s),invoke=window.__TAURI__.core.invoke;let running=false,desired=false,busy=false,actionError='';
async function refresh(){if(busy)return;try{const s=await invoke('status');running=s.running;desired=s.desired_running;$('#status').textContent=s.connection==='revoked'?tr("Invalid token"):s.connection==='retrying'?tr("Reconnecting"):s.running?tr("Process running"):s.bound?tr("Bound"):tr("Not bound");$('#status').className='badge '+(s.running?'good':'');$('#toggle').textContent=desired||running?tr("Stop"):tr("Start");$('#toggle').disabled=!s.bound||s.connection==='revoked';$('#forget').disabled=false;$('#logs').disabled=false;$('#close-logs').disabled=false;$('#confirm-forget').disabled=false;$('#cancel-forget').disabled=false;$('#sync').disabled=!s.bound;$('#tunnels').replaceChildren();$('#tunnels').className=s.tunnels.length?'':'empty';if(!s.tunnels.length)$('#tunnels').textContent=s.bound?tr("No tunnels yet. Add one in the web console."):tr("Your tunnels will appear here after binding.");for(const t of s.tunnels){const row=document.createElement('div');row.className='tunnel-item';const title=document.createElement('strong');title.textContent=t.name;const detail=document.createElement('span');detail.textContent=`${t.protocol.toUpperCase()} · ${t.local_host}:${t.local_port} → ${t.domain||t.public_port}`;row.append(title,detail);$('#tunnels').append(row);}$('#error').textContent=tr(s.error||actionError);}catch(e){$('#error').textContent=String(e);}}
async function action(fn){busy=true;document.querySelectorAll('button').forEach(b=>b.disabled=true);actionError='';$('#error').textContent='';try{await fn();}catch(e){actionError=String(e);$('#error').textContent=actionError;}finally{busy=false;$('#bind-form button').disabled=false;await refresh();}}
$('#bind-form').onsubmit=e=>{e.preventDefault();action(async()=>{const f=e.target.elements;await invoke('bind',{apiUrl:f.api.value,token:f.token.value,remember:f.remember.checked});f.token.value='';});};
$('#sync').onclick=()=>action(()=>invoke('sync'));$('#toggle').onclick=()=>action(()=>invoke(desired||running?'stop':'start'));$('#autostart').onchange=async e=>{const enabled=e.target.checked;try{await invoke('autostart',{enabled});}catch(error){e.target.checked=!enabled;$('#error').textContent=String(error);}};
invoke('autostart',{enabled:null}).then(v=>$('#autostart').checked=v).catch(e=>$('#error').textContent=String(e));refresh();setInterval(refresh,3000);

$('#logs').onclick=()=>action(async()=>{$('#log-content').textContent=await invoke('logs');$('#log-dialog').showModal();});
$('#close-logs').onclick=()=>$('#log-dialog').close();
$('#forget').onclick=()=>$('#forget-dialog').showModal();
$('#cancel-forget').onclick=()=>$('#forget-dialog').close();
$('#confirm-forget').onclick=()=>action(async()=>{await invoke('forget');$('#forget-dialog').close();$('#bind-form').reset();});

async function updateLocale() {
  try { await invoke('set_locale', {locale:HPI18n.locale}); } catch (error) { $('#error').textContent = String(error); }
  await refresh();
}
document.addEventListener('hp:localechange', updateLocale);
updateLocale();
