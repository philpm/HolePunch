#!/usr/bin/env python3
"""Create a portable native release archive and checksums; never publishes anything."""
import functools, argparse, hashlib, json, shutil, tarfile, tempfile, zipfile, subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent

@functools.lru_cache(maxsize=1)
def version():
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--no-deps','--format-version','1'],cwd=ROOT))
    return next(p['version'] for p in metadata['packages'] if p['name']=='holepunch-client')

def main(edition="selfhost", shared_root=None, extra_files=None):
    shared=shared_root or ROOT
    p=argparse.ArgumentParser();p.add_argument('--edition',choices=[edition],default=edition);p.add_argument('--target',required=True);p.add_argument('--bin-dir',type=Path,required=True);p.add_argument('--rathole',type=Path,required=True);p.add_argument('--out',type=Path,default=ROOT/'dist');a=p.parse_args()
    import re
    if not re.fullmatch(r'[A-Za-z0-9_-]+',a.target):p.error('invalid target triple')
    ext='.exe' if 'windows' in a.target else ''
    server='holepunch-server' if a.edition=='selfhost' else 'holepunch-cloud'
    required=[a.bin_dir/(server+ext),a.bin_dir/('holepunch-client'+ext),a.rathole]
    if any(not f.is_file() for f in required):p.error('build Center, Client and rathole before packaging')
    a.out.mkdir(parents=True,exist_ok=True);name=f'holepunch-{a.edition}-{version()}-{a.target}'
    with tempfile.TemporaryDirectory() as tmp:
        stage=Path(tmp)/name;stage.mkdir();(stage/'bin').mkdir()
        for src,name_ in zip(required,['holepunch-center','holepunch-client','rathole']):shutil.copy2(src,stage/'bin'/(name_+ext))
        for file in ['LICENSE','NOTICE.md','README.md','README-zh.md','SECURITY.md','CHANGELOG.md','CONTRIBUTING.md','CODE_OF_CONDUCT.md','SUPPORT.md']:
            shutil.copy2((ROOT/file if (ROOT/file).exists() else shared/file),stage/file)
        shutil.copytree(ROOT/'docs',stage/'docs')
        # Cloud guides link to the pinned core's documentation. Ship those texts
        # at the same relative paths, without copying source or runtime data.
        if shared != ROOT:
            shared_docs = stage/'vendor/holepunch'
            shared_docs.mkdir(parents=True)
            shutil.copytree(shared/'docs',shared_docs/'docs')
            for file in ['LICENSE','NOTICE.md','README.md','README-zh.md','SECURITY.md','CHANGELOG.md','CONTRIBUTING.md','CODE_OF_CONDUCT.md','SUPPORT.md']:
                shutil.copy2(shared/file,shared_docs/file)
        else:
            shared_docs = stage
        for directory in ['apps/desktop','web']:
            destination = shared_docs/directory
            destination.mkdir(parents=True,exist_ok=True)
            shutil.copy2(shared/directory/'README.md',destination/'README.md')
        shutil.copytree(shared/'deploy/systemd',stage/'systemd')
        source=next((p for p in [shared/'output/sidecar-source/rathole-0.5.0', shared/'output/rathole-source/rathole-0.5.0'] if p.is_dir()),None)
        if source is None:raise RuntimeError('prepare-sidecar source is required for license inventory')
        inventories={}
        notices=stage/'licenses';notices.mkdir()
        for scope,manifest in [('core',ROOT/'Cargo.toml'),('client',shared/'Cargo.toml'),('rathole',source/'Cargo.toml')]:
            metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1','--manifest-path',str(manifest)],cwd=ROOT))
            inventories[scope]=[]
            nodes={n['id']:n for n in metadata['resolve']['nodes']}
            roots=[p['id'] for p in metadata['packages'] if p['name'] in [server,'holepunch-cli']] if scope in ['core','client'] else [metadata['resolve']['root']]
            included=set();pending=[r for r in roots if r]
            while pending:
                pkgid=pending.pop()
                if pkgid in included:continue
                included.add(pkgid);pending.extend(nodes[pkgid]['dependencies'])
            for pkg in metadata['packages']:
                if pkg['id'] not in included:continue
                inventories[scope].append({'name':pkg['name'],'version':pkg['version'],'license':pkg.get('license'),'source':pkg.get('source')})
                package_dir=Path(pkg['manifest_path']).parent
                for file in package_dir.iterdir():
                    if file.is_file() and file.name.upper().startswith(('LICENSE','COPYING','NOTICE','COPYRIGHT')):
                        dest=notices/(pkg['name']+'-'+pkg['version']);dest.mkdir(exist_ok=True)
                        shutil.copy2(file,dest/file.name)
        (stage/'DEPENDENCIES.json').write_text(json.dumps(inventories,indent=2)+'\n')
        core_revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=shared,text=True).strip()
        (stage/'BUILD.json').write_text(json.dumps({'version':version(),'target':a.target,'rathole':'0.5.0-holepunch.2','schema':4,'edition':a.edition,'server_binary':server,'core_revision':core_revision},indent=2)+'\n')
        repro=stage/'build-inputs';repro.mkdir()
        for src,name_ in [(ROOT/'Cargo.lock','core.Cargo.lock'),(shared/'deploy/rathole.Cargo.lock','rathole.Cargo.lock'),(shared/'scripts/patch-rathole.py','patch-rathole.py'),(shared/'scripts/prepare-sidecar.sh','prepare-sidecar.sh'),(shared/'scripts/build-rathole.py','build-rathole.py'),(shared/'scripts/deploy.py','deploy.py'),(shared/'scripts/access.py','access.py')]:
            shutil.copy2(src,repro/name_)
        if shared != ROOT:shutil.copy2(shared/'Cargo.lock',repro/'public.Cargo.lock')
        shutil.copy2(shared/'deploy/caddy.json',stage/'caddy.json')
        shutil.copy2(shared/'deploy/bootstrap.Caddyfile',stage/'bootstrap.Caddyfile')
        if a.edition == 'selfhost':
            for script in ['deploy.py', 'access.py', 'setup.py', 'compose.py', 'install.sh', 'install.ps1']:
                shutil.copy2(shared/'scripts'/script,stage/script)
            shutil.copy2(shared/'deploy/compose.release.yml',stage/'compose.release.yml')
        for destination, source_file in (extra_files or {}).items():
            destination = Path(destination)
            if destination.is_absolute() or '..' in destination.parts:
                raise ValueError('Extra package files must stay within the release directory')
            (stage/destination).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source_file, stage/destination)
        if 'windows' in a.target:
            dest=a.out/(name+'.zip')
            with zipfile.ZipFile(dest,'w',zipfile.ZIP_DEFLATED) as archive:
                for file in sorted(stage.rglob('*')):
                    if file.is_file():archive.write(file,file.relative_to(stage.parent))
        else:
            dest=a.out/(name+'.tar.gz')
            with tarfile.open(dest,'w:gz') as archive:archive.add(stage,arcname=name)
    digest=hashlib.sha256(dest.read_bytes()).hexdigest()
    dest.with_name(dest.name+'.sha256').write_text(f'{digest}  {dest.name}\n')
    print(dest)
if __name__=='__main__':main()
