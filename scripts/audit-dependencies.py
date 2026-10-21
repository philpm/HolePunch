#!/usr/bin/env python3
"""Query public crate names/versions in lockfiles against OSV. Network failure is not a pass.
By default any advisory fails; --allow-informational tolerates upstream RustSec warnings
(unmaintained/unsound) but keeps them visible in the report for release review.
"""
import argparse,json,re,time,urllib.error,urllib.request,concurrent.futures
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent

def request(url,data=None):
    req=urllib.request.Request(url,data=None if data is None else json.dumps(data).encode(),headers={'Content-Type':'application/json','User-Agent':'HolePunch-dependency-check'})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req,timeout=40) as response:return json.load(response)
        except urllib.error.HTTPError as error:
            if attempt == 2 or (error.code != 429 and error.code < 500):raise
        except (urllib.error.URLError, TimeoutError, ConnectionError):
            if attempt == 2:raise
        time.sleep(2 ** attempt)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--allow-informational',action='store_true');parser.add_argument('--output',default='output/dependency-audit-current.json')
    parser.add_argument('--root',type=Path,default=ROOT,help='Repository containing the lockfiles')
    parser.add_argument('--lockfile',action='append',help='Relative lockfile path; repeat to audit multiple workspaces')
    args=parser.parse_args();root=args.root.resolve()
    mapping=[]
    for file in args.lockfile or ['Cargo.lock','apps/desktop/src-tauri/Cargo.lock','deploy/rathole.Cargo.lock']:
        for block in (root/file).read_text(encoding='utf-8').split('[[package]]')[1:]:
            if 'registry+' not in block:continue
            name=re.search(r'^name = "(.*?)"',block,re.M).group(1);version=re.search(r'^version = "(.*?)"',block,re.M).group(1)
            mapping.append((file,name,version))
    hits=[]
    for start in range(0,len(mapping),150):
        batch=mapping[start:start+150]
        result=request('https://api.osv.dev/v1/querybatch',{'queries':[{'package':{'name':name,'ecosystem':'crates.io'},'version':v} for _,name,v in batch]})
        for item,answer in zip(batch,result['results']):
            if answer.get('next_page_token'):raise RuntimeError('OSV returned paginated results; full review required')
            if answer.get('vulns'):hits.append((item,[v['id'] for v in answer['vulns']]))
    ids={id for _,advisories in hits for id in advisories};details={}
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        for id,detail in zip(sorted(ids),pool.map(lambda id:request('https://api.osv.dev/v1/vulns/'+id),sorted(ids))):details[id]=detail
    findings=[];blocking=0
    for (file,name,version),ids in hits:
        # Prefer the original RustSec record when GHSA lists the same issue.
        rustsec={id for id in ids if id.startswith('RUSTSEC-')}
        aliases={alias for id in rustsec for alias in details[id].get('aliases',[])}
        for id in ids:
            if id in aliases or details[id].get('withdrawn'):continue
            detail=details[id];kind=next((a.get('database_specific',{}).get('informational') for a in detail.get('affected',[]) if a.get('database_specific',{}).get('informational')),None)
            blocked=not(kind and args.allow_informational);blocking+=int(blocked)
            findings.append({'lock':file,'crate':name,'version':version,'id':id,'summary':detail.get('summary'),'informational':kind,'blocking':blocked,'source':'https://osv.dev/vulnerability/'+id})
    report={'queries':len(mapping),'blocking':blocking,'findings':findings};out=root/args.output;out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    for f in findings:print(('BLOCK' if f['blocking'] else 'REVIEW'),f['lock'],f['crate'],f['version'],f['id'],f['summary'])
    print(f'{len(mapping)} package records; {blocking} blocking advisories; {len(findings)-blocking} upstream review notices')
    return 1 if blocking else 0
if __name__=='__main__':raise SystemExit(main())
