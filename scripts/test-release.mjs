// Offline regression checks for signature gating and no-clobber release publication.
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
const verify = pathToFileURL(resolve('scripts/verify-release.mjs')).href;
const publish = pathToFileURL(resolve('scripts/publish-release.mjs')).href;
const temp = mkdtempSync(join(tmpdir(), 'rcm-release-test-'));
const env = { ...process.env, GITHUB_REPOSITORY: 'fixture/cache', GITHUB_REF_NAME: 'v0.2.0', GITHUB_TOKEN: 'offline-test-token' };
function test(name, module, setup, pass, cwd = process.cwd()) {
  const r = spawnSync(process.execPath, ['--input-type=module', '-e', setup + `\nawait import(${JSON.stringify(module)});`], { cwd, env, encoding: 'utf8' });
  if ((r.status === 0) !== pass) throw new Error(`${name} failed: ${r.stderr}`);
  console.log(`PASS ${name}`);
}
const verifySetup = `
import cp from 'node:child_process';
import { syncBuiltinESMExports } from 'node:module';
cp.execFileSync = (_, args) => args[0] === 'rev-list' ? 'head\\nparent\\n' : 'head\\n';
syncBuiltinESMExports();
globalThis.fetch = async url => {
 let data;
 if (url.includes('/git/ref/')) data={object:{type:LIGHTWEIGHT?'commit':'tag',sha:'tagsha'}};
 else if (url.includes('/git/tags/')) data={verification:{verified:true},object:{type:'commit',sha:'head'}};
 else if (url.includes('/commits/')) data={commit:{verification:{verified:!(UNSIGNED_PARENT && url.endsWith('/parent'))}}};
 else throw Error('Unexpected offline request');
 return new Response(JSON.stringify(data),{status:200});
};`;
try {
  test('all signed history accepted', verify, 'const LIGHTWEIGHT=false, UNSIGNED_PARENT=false;' + verifySetup, true);
  test('unsigned ancestor rejected', verify, 'const LIGHTWEIGHT=false, UNSIGNED_PARENT=true;' + verifySetup, false);
  test('lightweight tag rejected', verify, 'const LIGHTWEIGHT=true, UNSIGNED_PARENT=false;' + verifySetup, false);
  mkdirSync(join(temp, 'dist'));
  for (let i = 0; i < 5; i++) writeFileSync(join(temp, 'dist', `fixture-${i}.tar.gz`), 'offline fixture');
  for (const n of ['SHA256SUMS', 'install.sh', 'install.ps1']) writeFileSync(join(temp, 'dist', n), 'offline fixture');
  const publishSetup = `
let posts=0, uploads=0, published=false;
globalThis.fetch=async (url,opts={}) => {
 url=String(url); let data={}; let status=200;
 if(url.includes('/releases?')) {
   if(DRAFT && url.endsWith('page=1')) data=Array.from({length:100},(_,i)=>({tag_name:'v0.0.'+i}));
   else data=DRAFT?[{tag_name:'v0.2.0',draft:true}]:[];
 } else if(url.endsWith('/releases') && opts.method==='POST') {
   posts++; if(!JSON.parse(opts.body).draft) throw Error('Release must begin as draft');
   data={id:123,upload_url:'https://uploads.github.com/repos/fixture/cache/releases/123/assets{?name,label}'};
 } else if(url.startsWith('https://uploads.github.com/')) { uploads++; if(FAIL_UPLOAD) status=500; }
 else if(url.endsWith('/releases/123') && opts.method==='PATCH') {
   if(uploads!==8 || posts!==1) throw Error('Published before every asset uploaded'); published=true;
 } else throw Error('Unexpected offline request');
 return new Response(JSON.stringify(data),{status});
};
process.on('exit', code=>{if(code===0 && !published) process.exitCode=9;});`;
  test('complete assets publish after draft staging', publish, 'const DRAFT=false, FAIL_UPLOAD=false;' + publishSetup, true, temp);
  test('existing draft on second inventory page rejected', publish, 'const DRAFT=true, FAIL_UPLOAD=false;' + publishSetup, false, temp);
  test('failed asset upload cannot publish', publish, 'const DRAFT=false, FAIL_UPLOAD=true;' + publishSetup, false, temp);
} finally { rmSync(temp, { recursive: true, force: true }); }
