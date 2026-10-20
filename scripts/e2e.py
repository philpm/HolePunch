#!/usr/bin/env python3
"""Real TCP/UDP/HTTPS + config reload + revocation test. No external certificate requests.
Usage: python3 scripts/e2e.py --caddy /path/to/caddy --rathole /path/to/rathole
All subprocesses and sockets are closed on success or failure.
"""
import argparse, http.cookiejar, contextlib, json, os, secrets, signal, socket, socketserver, ssl, subprocess, threading, time, urllib.request, urllib.error
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--caddy',required=True);p.add_argument('--rathole',required=True);args=p.parse_args()
root=Path(__file__).resolve().parent.parent
out=root/'output'/'e2e';out.mkdir(parents=True,exist_ok=True)
# Unique state per run; logs retained for diagnosing failure, permissions protect service keys.
import tempfile
run=Path(tempfile.mkdtemp(prefix='run-',dir=out));os.chmod(run,0o700)
admin=secrets.token_hex(32);procs=[];logs=[];servers=[];results=[]
def launch(cmd,name,env=None):
    log=open(run/f'{name}.log','w');logs.append(log)
    proc=subprocess.Popen([str(x) for x in cmd],cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
    procs.append(proc);return proc

session=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
csrf=''
def call(path,method='GET',data=None,token=admin,base='http://127.0.0.1:13000'):
    headers={'Content-Type':'application/json'}
    if token==admin:
        headers['X-CSRF-Token']=csrf
    elif token:headers['Authorization']='Bearer '+token
    request=urllib.request.Request(base+path,data=None if data is None else json.dumps(data).encode(),headers=headers,method=method)
    try:
        with (session.open(request,timeout=8) if token==admin else urllib.request.urlopen(request,timeout=8)) as res:
            raw=res.read();return res.status,json.loads(raw) if raw else None
    except urllib.error.HTTPError as e:return e.code,e.read().decode()

def wait(fn,label,timeout=25):
    end=time.monotonic()+timeout;last=None
    while time.monotonic()<end:
        try:
            result=fn()
            if result:return result
        except Exception as e:last=e
        time.sleep(.25)
    raise AssertionError(f'{label} timed out: {last}')
def check(label,fn):
    assert fn(),label;results.append(label);print('PASS',label,flush=True)
class Echo(socketserver.BaseRequestHandler):
    def handle(self):
        while data:=self.request.recv(4096):self.request.sendall(data)
class UDP(socketserver.BaseRequestHandler):
    def handle(self):data,sock=self.request;sock.sendto(data,self.client_address)
class Web(socketserver.BaseRequestHandler):
    def handle(self):
        self.request.recv(8192);self.request.sendall(b'HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\nholepunch-works')
class LocalWeb(socketserver.BaseRequestHandler):
    def handle(self):
        self.request.recv(8192);self.request.sendall(b'HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\nholepunch-local')
class TCPServer(socketserver.ThreadingTCPServer):allow_reuse_address=True;daemon_threads=True
class UDPServer(socketserver.ThreadingUDPServer):allow_reuse_address=True;daemon_threads=True

def echo(port,udp=False):
    with socket.socket(socket.AF_INET,socket.SOCK_DGRAM if udp else socket.SOCK_STREAM) as s:
        s.settimeout(1);s.connect(('127.0.0.1',port));s.sendall(b'holepunch-roundtrip');return s.recv(128)==b'holepunch-roundtrip'
def closed(port):
    try:
        with socket.create_connection(('127.0.0.1',port),timeout=.5):return False
    except OSError:return True

def sync():
    status,body=call('/api/v1/sync','POST',{});assert status==200,body

def create(client,protocol,local,public,domain=None):
    body={'name':'E2E '+protocol,'client_id':client['id'],'protocol':protocol,'local_host':'127.0.0.1','local_port':local,'public_port':public,'domain':domain,'enabled':True}
    status,t=call('/api/v1/portal/tunnels','POST',body);assert status==200,t;return t
try:
    for cls,handler,port in [(TCPServer,Echo,19101),(UDPServer,UDP,19102),(TCPServer,Web,19103),(TCPServer,LocalWeb,19104)]:
        server=cls(('127.0.0.1',port),handler);servers.append(server);threading.Thread(target=server.serve_forever,daemon=True).start()
    caddy_config=run/'caddy.json';caddy_config.write_text('{"admin":{"listen":"127.0.0.1:12019"},"apps":{}}')
    env=os.environ.copy();env.update({'XDG_DATA_HOME':str(run/'caddy-data'),'XDG_CONFIG_HOME':str(run/'caddy-config')})
    caddy=launch([Path(args.caddy).resolve(),'run','--config',caddy_config],'caddy',env)
    wait(lambda:call('/config/',token=None,base='http://127.0.0.1:12019')[0]==200,'Caddy')
    env.update({'HP_ADMIN_TOKEN':admin,'HP_ALLOW_HTTP':'1','HP_BOOTSTRAP_EMAIL':'admin@example.test','HP_BOOTSTRAP_PASSWORD':secrets.token_urlsafe(24),'HP_DATA_DIR':str(run/'center'),'HP_BIND':'127.0.0.1:13000','HP_CADDY_URL':'http://127.0.0.1:12019','HP_CADDY_LISTEN':'127.0.0.1:12019','HP_TLS_ASK_URL':'http://127.0.0.1:13000/api/v1/tls/check','HP_PUBLIC_ADDR':'127.0.0.1:12333','HP_RATHOLE_BIND':'127.0.0.1:12333','HP_HTTPS_LISTEN':'127.0.0.1:18443','HP_HTTP_PORT':'18080','HP_PORT_START':'21000','HP_PORT_END':'21020','HP_PANEL_DOMAIN':'','HP_TLS_INTERNAL':'1'})
    center=launch([root/'target/debug/holepunch-server'],'center',env)
    wait(lambda:call('/healthz')[0]==200,'Center')
    status,login=call('/api/v1/auth/login','POST',{'email':env['HP_BOOTSTRAP_EMAIL'],'password':env['HP_BOOTSTRAP_PASSWORD']});assert status==200;csrf=login['csrf']
    status,client=call('/api/v1/portal/clients','POST',{'name':'E2E device'});assert status==200
    status,binding=call('/api/v1/enroll','POST',{'token':client['enroll_token']},token=None);assert status==200;client['token']=binding['token']
    tcp=create(client,'tcp',19101,21000);udp=create(client,'udp',19102,21001);web=create(client,'web',19103,21002,'e2e.example.test');sync()
    status,site=call('/api/v1/sites','POST',{'name':'Local app','domain':'local.example.test','upstream_host':'127.0.0.1','upstream_port':19104,'enabled':True});assert status==200,site
    sync()
    server=launch([Path(args.rathole).resolve(),'--server',run/'center/server.toml'],'rathole-server')
    client_env=os.environ.copy();client_env.update({'HP_API_URL':'http://127.0.0.1:13000','HP_CLIENT_TOKEN':client['token'],'HP_CLIENT_DATA':str(run/'client'),'HP_RATHOLE_BIN':str(Path(args.rathole).resolve())})
    local=launch([root/'target/debug/holepunch-client'],'client',client_env)
    check('TCP roundtrip over Noise',lambda:wait(lambda:echo(21000),'TCP'))
    check('UDP roundtrip over Noise',lambda:wait(lambda:echo(21001,True),'UDP'))
    check('Client heartbeat',lambda:wait(lambda:call('/api/v1/dashboard')[1]['online_clients']==1,'heartbeat'))
    check('Readiness verifies edge listener',lambda:wait(lambda:call('/readyz')[0]==200,'readiness'))
    backup=run/'online-backup.db'
    subprocess.run([str(root/'target/debug/holepunch-server'),'backup',str(backup)],env=env,check=True,stdout=subprocess.DEVNULL)
    import sqlite3
    with sqlite3.connect(backup) as db:
        check('Online backup includes committed configuration',lambda:db.execute('SELECT COUNT(*) FROM tunnels').fetchone()[0]==3)
    blocked=subprocess.run([str(root/'target/debug/holepunch-server'),'restore',str(backup)],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    check('Restore refuses a running Center',lambda:blocked.returncode!=0)
    # The server-owned gateway exits with the server process. Validate automatic
    # recovery of new data connections, not survival of sockets owned by it.
    center.terminate();center.wait(timeout=8)
    center=launch([root/'target/debug/holepunch-server'],'center-restarted',env)
    wait(lambda:call('/healthz')[0]==200,'Center restart')
    check('TCP automatically reconnects after gateway/server restart',lambda:wait(lambda:echo(21000),'TCP recovery'))
    check('Client automatically resynchronizes after Center restart',lambda:wait(lambda:call('/api/v1/dashboard')[1]['online_clients']==1,'client recovery'))
    # Test an independent restore, preserving the live installation.
    restore_env=env.copy();restore_env['HP_DATA_DIR']=str(run/'restored')
    subprocess.run([str(root/'target/debug/holepunch-server'),'restore',str(backup)],env=restore_env,check=True,stdout=subprocess.DEVNULL)
    with sqlite3.connect(run/'restored/holepunch.db') as db:
        check('Backup restores into an independent installation',lambda:db.execute('SELECT COUNT(*) FROM tunnels').fetchone()[0]==3)

    # Use Caddy local issuer for this test only; never contact public ACME.
    def https_response(host='e2e.example.test'):
        context=ssl._create_unverified_context()
        with socket.create_connection(('127.0.0.1',18443),timeout=3) as sock:
            with context.wrap_socket(sock,server_hostname=host) as tls:
                tls.sendall(f'GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n'.encode());chunks=[]
                while chunk:=tls.recv(8192):chunks.append(chunk)
                return b''.join(chunks)
    def https():return b'holepunch-works' in https_response()
    def local_site():return b'holepunch-local' in https_response('local.example.test')
    check('Same-host HTTPS survives Center restart',lambda:wait(local_site,'same-host HTTPS'))
    check('HTTPS -> Caddy -> rathole -> local HTTP (local CA)',lambda:wait(https,'HTTPS'))
    # Keep the same server and client process IDs while both watcher configurations change.
    tcp2=create(client,'tcp',19101,21003);sync()
    check('Hot add without process restart',lambda:wait(lambda:echo(21003),'hot add',40) and server.poll() is None and local.poll() is None)
    call('/api/v1/portal/tunnels/'+tcp['id'],'DELETE');sync()
    check('Hot delete closes public listener',lambda:wait(lambda:closed(21000),'hot delete'))
    check('Tunnel hot changes preserve same-host HTTPS',local_site)
    # Verify unknown names and missing Ask keys cannot issue certificates.
    config=call('/config/',token=None,base='http://127.0.0.1:12019')[1]
    ask=config['apps']['tls']['automation']['on_demand']['permission']['endpoint']
    from urllib.parse import urlsplit,parse_qs
    key=parse_qs(urlsplit(ask).query)['key'][0]
    check('Ask accepts enrolled active domain',lambda:call('/api/v1/tls/check?domain=e2e.example.test&key='+key,token=None)[0]==200)
    check('Ask permits enabled same-host website',lambda:call('/api/v1/tls/check?domain=local.example.test&key='+key,token=None)[0]==200)
    check('Ask rejects unknown domain',lambda:call('/api/v1/tls/check?domain=unknown.example.test&key='+key,token=None)[0]==403)
    call('/api/v1/portal/clients/'+client['id']+'/revoke','POST',{});sync()
    check('Revocation blocks client authentication',lambda:call('/api/v1/client/config',token=client['token'])[0]==401)
    check('Revocation denies TLS Ask',lambda:call('/api/v1/tls/check?domain=e2e.example.test&key='+key,token=None)[0]==403)
    check('Revocation removes TCP listener',lambda:wait(lambda:closed(21003),'revoked listener'))
    check('Revoked client removes cached credentials',lambda:wait(lambda:not (run/'client/client.toml').exists(),'client revocation'))
    check('Client revocation preserves same-host HTTPS',local_site)
    # A Caddy-only restart must restore routes even without a new database revision.
    revision=call('/api/v1/dashboard')[1]['sync']['revision']
    caddy.terminate();caddy.wait(timeout=5)
    caddy=launch([Path(args.caddy).resolve(),'run','--config',caddy_config],'caddy-drift',env)
    check('Caddy restart restores configuration without mutation',lambda:wait(lambda: 'http' in call('/config/',token=None,base='http://127.0.0.1:12019')[1].get('apps',{}),'drift recovery') and call('/api/v1/dashboard')[1]['sync']['revision']==revision)
    # Retry state is exposed, then converges after Caddy returns.
    caddy.terminate();caddy.wait(timeout=5)
    check('Offline Caddy produces explicit 502',lambda:call('/api/v1/sync','POST',{})[0]==502)
    caddy=launch([Path(args.caddy).resolve(),'run','--config',caddy_config],'caddy-restarted',env)
    check('Automatic recovery after Caddy restart',lambda:wait(lambda:call('/api/v1/dashboard')[1]['sync']['status']=='applied','retry recovery'))
    check('Caddy restart restores same-host website',lambda:wait(local_site,'site restart'))
    server.terminate();server.wait(timeout=5)
    check('Same-host HTTPS works with rathole stopped',local_site)
    site_path='/api/v1/sites/'+site['id']
    changed={**site,'upstream_port':19103}
    assert call(site_path,'PUT',changed)[0]==200;sync()
    check('Site edit changes actual upstream',lambda:b'holepunch-works' in https_response('local.example.test'))
    def site_blocked():
        config=call('/config/',token=None,base='http://127.0.0.1:12019')[1]
        routes=config['apps']['http']['servers']['holepunch']['routes']
        if any(route.get('@id')=='site-'+site['id'] for route in routes):return False
        try:return b' 404 ' in https_response('local.example.test')
        except ssl.SSLError:return True  # permission denial may reject the handshake before HTTP
    changed['enabled']=False;assert call(site_path,'PUT',changed)[0]==200;sync()
    check('Disabled site denies new certificates',lambda:call('/api/v1/tls/check?domain=local.example.test&key='+key,token=None)[0]==403)
    check('Disabled site removes route and blocks HTTPS',site_blocked)
    changed['enabled']=True;changed['upstream_port']=19104;assert call(site_path,'PUT',changed)[0]==200;sync()
    check('Site can be re-enabled',local_site)
    assert call(site_path,'DELETE')[0]==200;sync()
    check('Deleted site stops HTTPS routing',site_blocked)
    report={'passed':len(results),'checks':results,'run_directory':str(run)};(out/'latest.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
finally:
    for proc in reversed(procs):
        if proc.poll() is None:
            proc.send_signal(signal.SIGINT)
            try:proc.wait(timeout=5)
            except subprocess.TimeoutExpired:proc.kill();proc.wait()
    for s in servers:s.shutdown();s.server_close()
    for f in logs:f.close()
