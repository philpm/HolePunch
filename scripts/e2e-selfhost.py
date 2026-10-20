#!/usr/bin/env python3
"""Isolated Self-hosted + shared CLI + unlimited/optional-limit acceptance.
Uses Caddy internal CA; never contacts production or modifies system trust.
"""
import base64,hashlib,argparse,contextlib,http.cookiejar,json,os,secrets,socket,socketserver,ssl,subprocess,tempfile,threading,time,urllib.request,urllib.error
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--caddy',required=True);p.add_argument('--rathole',required=True);a=p.parse_args()
root=Path(__file__).resolve().parents[1];out=root/'output/selfhost';out.mkdir(parents=True,exist_ok=True);run=Path(tempfile.mkdtemp(dir=out));run.chmod(0o700)
BASE='http://127.0.0.1:13010';processes=[];handles=[];servers=[];checks=[]
def launch(cmd,name,env=None,stdin=None):
 log=open(run/(name+'.log'),'w');handles.append(log);proc=subprocess.Popen([str(x) for x in cmd],env=env,cwd=root,stdout=log,stderr=subprocess.STDOUT,stdin=subprocess.PIPE if stdin else subprocess.DEVNULL);processes.append(proc)
 if stdin:proc.stdin.write((stdin+'\n').encode());proc.stdin.close()
 return proc
class Session:
 def __init__(self):self.jar=http.cookiejar.CookieJar();self.http=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.jar));self.csrf=''
 def call(self,path,method='GET',data=None):
  req=urllib.request.Request(BASE+'/api/v1'+path,method=method,headers={'Content-Type':'application/json','X-CSRF-Token':self.csrf},data=None if data is None else json.dumps(data).encode())
  try:
   with self.http.open(req,timeout=10) as r:raw=r.read();v=json.loads(raw) if raw else {};return r.status,v
  except urllib.error.HTTPError as e:
   raw=e.read()
   try:value=json.loads(raw)
   except ValueError:value={}
   return e.code,value
 def ok(self,path,method='GET',data=None):
  code,v=self.call(path,method,data);assert code==200,(path,code,v);return v
 def login(self,email,password):self.csrf=self.ok('/auth/login','POST',{'email':email,'password':password})['csrf']
def wait(fn,label,timeout=30):
 end=time.monotonic()+timeout;last=None
 while time.monotonic()<end:
  try:
   v=fn()
   if v:return v
  except Exception as e:last=type(e).__name__
  time.sleep(.25)
 raise AssertionError(label+' timeout '+str(last))
def passed(label):checks.append(label);print('PASS',label,flush=True)
class TCP(socketserver.BaseRequestHandler):
 def handle(self):
  with contextlib.suppress(OSError):
   while data:=self.request.recv(4096):self.request.sendall(data)
class UDP(socketserver.BaseRequestHandler):
 def handle(self):self.request[1].sendto(self.request[0],self.client_address)
class HTTP(socketserver.BaseRequestHandler):
 def handle(self):
  data=self.request.recv(8192)
  if b'Upgrade: websocket' not in data:
   self.request.sendall(b'HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\nfood-api-dev');return
  key=data.split(b'Sec-WebSocket-Key: ')[1].split(b'\r\n')[0]
  accept=base64.b64encode(hashlib.sha1(key+b'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest())
  self.request.sendall(b'HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: '+accept+b'\r\n\r\n')
  with contextlib.suppress(OSError):
   while frame:=self.request.recv(131):
    if len(frame)<6:return
    n=frame[1]&127;mask=frame[2:6];payload=bytes(b^mask[i%4] for i,b in enumerate(frame[6:6+n]))
    self.request.sendall(bytes([0x81,len(payload)])+payload)
class TS(socketserver.ThreadingTCPServer):allow_reuse_address=True;daemon_threads=True
class US(socketserver.ThreadingUDPServer):allow_reuse_address=True;daemon_threads=True
def echo(port,n=32,udp=False):
 with socket.socket(socket.AF_INET,socket.SOCK_DGRAM if udp else socket.SOCK_STREAM) as s:
  s.settimeout(12);s.connect(('127.0.0.1',port));data=b'a'*n;s.sendall(data);received=b''
  while len(received)<n:received+=s.recv(n-len(received))
  return received==data
def closed(port):
 try:
  with socket.create_connection(('127.0.0.1',port),timeout=.5):return False
 except OSError:return True

try:
 # Fail early on occupied ports, including specific-loopback listeners that can
 # coexist with wildcard listeners on macOS.
 for port in [13010,12029,12343,18453,18090,19201,19202,19203,*range(24000,24021),30000,30001]:
  for kind in [socket.SOCK_STREAM,socket.SOCK_DGRAM]:
   with socket.socket(socket.AF_INET,kind) as probe:
    if kind==socket.SOCK_STREAM:probe.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
    probe.bind(('127.0.0.1',port))
    if kind==socket.SOCK_STREAM:probe.listen(1)
 for cls,handler,port in [(TS,TCP,19201),(US,UDP,19202),(TS,HTTP,19203)]:
  server=cls(('127.0.0.1',port),handler);servers.append(server);threading.Thread(target=server.serve_forever,daemon=True).start()
 env=os.environ.copy();password=secrets.token_urlsafe(24)
 env.update({'RUST_LOG':'info,holepunch_server_core::bandwidth=debug','HP_ADMIN_TOKEN':secrets.token_hex(32),'HP_DATA_DIR':str(run/'center'),'HP_BIND':'127.0.0.1:13010','HP_BOOTSTRAP_EMAIL':'admin@example.test','HP_BOOTSTRAP_PASSWORD':password,'HP_ALLOW_HTTP':'1','HP_MAIL_OUTBOX':str(run/'mail'),'HP_CADDY_URL':'http://127.0.0.1:12029','HP_CADDY_LISTEN':'127.0.0.1:12029','HP_TLS_ASK_URL':BASE+'/api/v1/tls/check','HP_PUBLIC_ADDR':'127.0.0.1:12343','HP_RATHOLE_BIND':'127.0.0.1:12343','HP_HTTPS_LISTEN':'127.0.0.1:18453','HP_HTTP_PORT':'18090','HP_PORT_START':'24000','HP_PORT_END':'24020','HP_PANEL_DOMAIN':'','HP_TLS_INTERNAL':'1'})
 for key in list(env):
  if key.startswith('HP_SMTP_') or key=='HP_MAIL_OUTBOX':env.pop(key,None)
 config=run/'caddy.json';config.write_text(json.dumps({'admin':{'listen':'127.0.0.1:12029'}}));env['XDG_DATA_HOME']=str(run/'caddy-data');env['XDG_CONFIG_HOME']=str(run/'caddy-config')
 launch([a.caddy,'run','--config',config],'caddy',env);center=launch([root/'target/debug/holepunch-server'],'center',env)
 admin=Session();wait(lambda:admin.call('/auth/login','POST',{'email':'admin@example.test','password':password})[0]==200,'bootstrap login');admin.login('admin@example.test',password);passed('Explicit administrator bootstrap and password session')
 assert admin.ok('/capabilities')['edition']=='self_hosted'
 assert admin.ok('/me')['limits']=={'clients':None,'tunnels':None,'bps':None}
 for path,method in [('/admin/users','GET'),('/admin/policy','GET'),('/auth/register','POST'),('/auth/request-reset','POST')]:assert admin.call(path,method,{})[0]==404
 passed('Single workspace, no SMTP dependency, no SaaS endpoints, unlimited account quotas')
 for i in range(3):device=admin.ok('/portal/clients','POST',{'name':'selfhost-'+str(i)})
 passed('Create clients beyond Cloud default quota')
 wait(lambda:(run/'center/server.toml').exists(),'server config');launch([a.rathole,'--server',run/'center/server.toml'],'rathole',env)
 client_env=os.environ.copy();client_env.update({'HP_CLIENT_DATA':str(run/'client'),'HP_RATHOLE_BIN':str(Path(a.rathole).resolve())})
 launch([root/'target/debug/holepunch-client','connect','--server',BASE,'--token-stdin'],'client',client_env,device['enroll_token'])
 wait(lambda:(run/'client/status.json').exists(),'CLI status');passed('Same CLI enrolls into Self-hosted')
 def new(protocol,local,domain=None):return {'name':'selfhost-'+protocol,'client_id':device['id'],'protocol':protocol,'local_host':'127.0.0.1','local_port':local,'domain':domain,'enabled':True,'bandwidth_limit_bps':None}
 tcp=admin.ok('/portal/tunnels','POST',new('tcp',19201));udp=admin.ok('/portal/tunnels','POST',new('udp',19202));web=admin.ok('/portal/tunnels','POST',new('web',19203,'my-service.owned.example.test'))
 sync=lambda:admin.ok('/sync','POST',{})
 sync();wait(lambda:echo(tcp['public_port']),'TCP');wait(lambda:echo(udp['public_port'],8192,udp=True),'UDP')
 passed('Unlimited TCP and 8 KB UDP through gateway, Noise and actual CLI')
 start=time.monotonic();assert echo(tcp['public_port'],131072);elapsed=time.monotonic()-start;assert elapsed<1.5,elapsed
 passed('256 KB bidirectional TCP completes without the Cloud 1 Mbps default cap')
 def https():
  result=subprocess.run(['curl','-sk','--max-time','3','--resolve','my-service.owned.example.test:18453:127.0.0.1','https://my-service.owned.example.test:18453/'],capture_output=True)
  return result.returncode==0 and result.stdout==b'food-api-dev'
 wait(https,'HTTPS own domain');passed('Own domain HTTPS without platform suffix, using isolated internal CA')
 admin.ok('/domains','PUT',{'domain_suffix':'my.owned.example.test'})
 random_web=admin.ok('/portal/tunnels','POST',new('web',19203));assert random_web['domain'].endswith('.my.owned.example.test')
 passed('Optional private domain suffix and random subdomain beyond Cloud two-tunnel default')
 admin.ok('/portal/bandwidth','POST',{'allocations':[{'tunnel_id':tcp['id'],'bps':128000}]});sync();wait(lambda:echo(tcp['public_port']),'rate ready')
 start=time.monotonic();assert echo(tcp['public_port'],8000);limited=time.monotonic()-start;assert .9<=limited<4,limited
 admin.ok('/portal/bandwidth','POST',{'allocations':[{'tunnel_id':tcp['id'],'bps':None}]});sync();wait(lambda:echo(tcp['public_port']),'unlimited restored')
 passed('Optional finite limit can be enabled and removed through the same API')
 admin.ok('/portal/bandwidth','POST',{'allocations':[{'tunnel_id':tcp['id'],'bps':0}]});sync();wait(lambda:closed(tcp['public_port']),'zero stopped')
 passed('Zero still disables, null means unlimited')
 admin.ok('/portal/clients/'+device['id']+'/revoke','POST',{});sync()
 assert admin.ok('/portal/tunnels')==[];passed('Revocation deletes owned tunnels and unlimited policies')
 result={'edition':'self_hosted','passed':len(checks),'checks':checks,'unlimited_roundtrip_seconds':elapsed,'limited_roundtrip_seconds':limited,'run':str(run)};(out/'latest.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
finally:
 for proc in reversed(processes):
  if proc.poll() is None:
   proc.terminate()
   try:proc.wait(timeout=8)
   except subprocess.TimeoutExpired:proc.kill();proc.wait()
 for s in servers:s.shutdown();s.server_close()
 for h in handles:h.close()
