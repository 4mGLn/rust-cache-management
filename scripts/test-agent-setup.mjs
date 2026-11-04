// Native packaged-binary behavior tests. All agent destinations are private child state.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdtempSync,mkdirSync,readdirSync,readFileSync,writeFileSync,copyFileSync,existsSync,rmSync,lstatSync,realpathSync,unlinkSync,symlinkSync,readlinkSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,dirname,resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {setTimeout as wait} from 'node:timers/promises';
const binary=resolve(process.argv[2] || '');
assert(process.argv[2], 'Usage: node scripts/test-agent-setup.mjs PACKAGED_BINARY [MANAGEMENT_WRAPPER]');
const payload=dirname(dirname(binary));
const wrapper=process.argv[3]?resolve(process.argv[3]):null;
const installation=dirname(dirname(payload));
const wrapperState=[];
let primaryError;
const cleanupErrors=[];
const temp=realpathSync(mkdtempSync(join(tmpdir(),'rcm-native-agents-')));
const environmentDigest=()=>createHash('sha256').update(JSON.stringify(process.env)).digest('hex');
const parentEnv=environmentDigest();
const texts=['AGENTS-snippet.md','CHANGELOG.md','CONTRIBUTING.md','LICENSE','README.md','SKILL.md','THIRD-PARTY-NOTICES','VERSION','references/defaults.md','references/distribution.md','references/inspection.md','references/policy.md'];
function makeHome(name) {
  const home=join(temp,name); mkdirSync(home); return home;
}
function environment(home) {
  return {HOME:home,USERPROFILE:home,PATH:join(temp,'empty-path'),XDG_CONFIG_HOME:join(home,'.config'),XDG_DATA_HOME:join(home,'.local','share'),XDG_STATE_HOME:join(home,'.local','state'),APPDATA:join(home,'AppData','Roaming'),LOCALAPPDATA:join(home,'AppData','Local'),TEMP:temp,TMP:temp,...(process.platform==='win32'?{SystemRoot:process.env.SystemRoot,WINDIR:process.env.WINDIR,PATHEXT:process.env.PATHEXT || '.COM;.EXE;.BAT;.CMD',ComSpec:process.env.ComSpec || join(process.env.SystemRoot,'System32','cmd.exe')}:{} )};
}
function snapshot(root,allowLinks=false) {
  const entries=[];
  function walk(dir) {
    if(!existsSync(dir))return;
    for(const name of readdirSync(dir).sort()) {
      const path=join(dir,name),st=lstatSync(path);
      if(st.isSymbolicLink()) {
        assert(allowLinks,'Fixture should not create links');
        entries.push([path.slice(root.length),'link',readlinkSync(path)]);continue;
      }
      entries.push([path.slice(root.length),st.isDirectory()?'directory':createHash('sha256').update(readFileSync(path)).digest('hex')]);
      if(st.isDirectory())walk(path);
    }
  }
  walk(root); return JSON.stringify(entries);
}
function run(home,args,code=0) {
  const result=spawnSync(binary,['agents',...args,'--json'],{env:environment(home),cwd:temp,encoding:'utf8',timeout:30000,stdio:['ignore','pipe','pipe']});
  assert.ifError(result.error);
  assert.equal(result.status,code,JSON.stringify({args,status:result.status,stdout:result.stdout,stderr:result.stderr}));
  const parsed=JSON.parse(result.stdout); assert.equal(parsed.schema_version,1);assert.equal(parsed.command,args[0]);assert(Array.isArray(parsed.agents)&&Array.isArray(parsed.actions)&&Array.isArray(parsed.errors)&&Array.isArray(parsed.warnings));
  assert(['detected','planned','consent_required','complete','conflict','io_error','invalid','help'].includes(parsed.status));return parsed;
}
function runWrapped(home,args,code=0) {
  const env=environment(home);
  env.PATH=process.platform==='win32'?join(process.env.SystemRoot,'System32'):'/usr/bin:/bin';
  const command=process.platform==='win32'?join(process.env.SystemRoot,'System32','WindowsPowerShell','v1.0','powershell.exe'):wrapper;
  const prefix=process.platform==='win32'?['-NoLogo','-NoProfile','-NonInteractive','-File',wrapper]:[];
  const result=spawnSync(command,[...prefix,'agents',...args,'--json'],{env,cwd:temp,encoding:'utf8',timeout:30000,stdio:['ignore','pipe','pipe']});
  const diagnostic={command,args,parentCwd:process.cwd(),childCwd:temp,pid:result.pid,status:result.status,signal:result.signal,error:result.error?.message,stdout:result.stdout?.slice(-16384),stderr:result.stderr?.slice(-16384)};
  console.error('Management wrapper subprocess finished:',JSON.stringify({pid:result.pid,status:result.status,signal:result.signal,error:result.error?.message}));
  if(result.error)throw new Error('Management wrapper subprocess failed: '+JSON.stringify(diagnostic),{cause:result.error});
  assert.equal(result.status,code,JSON.stringify(diagnostic));
  let parsed;
  try { parsed=JSON.parse(result.stdout); }
  catch(error) { throw new Error('Management wrapper JSON failed: '+JSON.stringify(diagnostic),{cause:error}); }
  assert.equal(parsed.schema_version,1);return parsed;
}
function windowsHostControls(home) {
  const pe=readFileSync(binary),offset=pe.readUInt32LE(0x3c);
  assert.equal(pe.toString('ascii',offset,offset+4),'PE\0\0');
  const subsystem=pe.readUInt16LE(offset+24+68);
  assert.equal(subsystem,3,'Packaged binary must be a console executable');
  console.error('Packaged Windows PE subsystem:',subsystem);
  const target=Buffer.from(binary,'utf8').toString('base64');
  let properResult;
  for(const [mode,keepPathExt,keepComSpec,pipeline] of [
    ['legacy-incomplete',false,false,false],['pathext-only',true,false,false],
    ['comspec-only',false,true,false],['windows-essentials',true,true,false],
    ['legacy-forced-pipeline',false,false,true]]) {
    const script=join(temp,'host-control-'+mode+'.ps1');
    writeFileSync(script,"$ErrorActionPreference='Stop'\n$exe=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('"+target+"'))\n& $exe @args"+(pipeline?' | ForEach-Object { $_ }':'')+"\nexit $LASTEXITCODE\n");
    const env=environment(home);env.PATH=join(process.env.SystemRoot,'System32');
    if(!keepPathExt)delete env.PATHEXT;if(!keepComSpec)delete env.ComSpec;
    const command=join(process.env.SystemRoot,'System32','WindowsPowerShell','v1.0','powershell.exe');
    const result=spawnSync(command,['-NoLogo','-NoProfile','-NonInteractive','-File',script,'agents','--help'],{env,cwd:temp,encoding:'utf8',timeout:30000,stdio:['ignore','pipe','pipe']});
    let jsonStatus;try {jsonStatus=JSON.parse(result.stdout).status;}catch {jsonStatus='not-json';}
    // Deliberately incomplete environments are diagnostic controls, never evidence of a passing product test.
    console.error('Windows native host control:',JSON.stringify({mode,subsystem,pid:result.pid,status:result.status,signal:result.signal,error:result.error?.message,jsonStatus,stdoutBytes:Buffer.byteLength(result.stdout || ''),stderr:result.stderr?.slice(-4096),parentCwd:process.cwd(),childCwd:temp}));
    if(mode==='windows-essentials')properResult={status:result.status,jsonStatus,error:result.error?.message};
  }
  assert.equal(properResult.status,0,JSON.stringify(properResult));assert.equal(properResult.jsonStatus,'help',JSON.stringify(properResult));
}
function setup(home,extra=[],code=0) { return run(home,['setup','--agent','codex','--scope','user',...extra],code); }
function copyManual(destination) {
  for(const file of texts) {mkdirSync(dirname(join(destination,file)),{recursive:true});copyFileSync(join(payload,file),join(destination,file));}
}
try {
  mkdirSync(join(temp,'empty-path'));
  const home=makeHome("user home's unicode 한글");
  assert.equal(run(home,['detect']).status,'detected');
  const initial=snapshot(home);
  assert.equal(setup(home,['--dry-run']).status,'planned'); assert.equal(snapshot(home),initial,'Dry run changed user state');
  assert.equal(setup(home,[],4).status,'consent_required'); assert.equal(snapshot(home),initial,'Missing consent changed user state');
  setup(home,['--yes']);
  const skill=join(home,'.agents','skills','rust-cache-management');
  for(const file of texts) assert.deepEqual(readFileSync(join(skill,file)),readFileSync(join(payload,file)));
  const applied=snapshot(home);setup(home,['--yes']);assert.equal(snapshot(home),applied,'Repeat apply changed managed files');
  const project=join(temp,'project space');mkdirSync(project);
  mkdirSync(join(home,'.gemini')); // Private fixture signals Gemini usage and its cross-scope duplicate warning risk.
  run(home,['setup','--agent','codex','--scope','project','--project',project,'--dry-run'],3);
  assert(!existsSync(join(project,'.agents')),'Cross-scope conflict changed project');
  const projectHome=makeHome('project user');
  run(projectHome,['setup','--agent','codex','--scope','project','--project',project,'--dry-run']);
  assert(!existsSync(join(project,'.agents')),'Project preview created agent directory');
  run(projectHome,['setup','--agent','codex','--scope','project','--project',project,'--yes']);
  assert(existsSync(join(project,'.agents','skills','rust-cache-management','SKILL.md')));
  const sharedHome=makeHome('shared user');
  const shared=run(sharedHome,['setup','--agent','codex,gemini,opencode','--scope','user','--yes']);
  assert.equal(shared.actions.length,1,'Compatible providers should share one registration');
  assert(existsSync(join(sharedHome,'.agents','skills','rust-cache-management','SKILL.md')));
  for(const [agent,folder] of [['claude','.claude'],['hermes','.hermes']]) {
    const nativeHome=makeHome(agent+' user');
    run(nativeHome,['setup','--agent',agent,'--scope','user','--dry-run']);
    assert(!existsSync(join(nativeHome,folder)),'Native provider preview changed state');
    run(nativeHome,['setup','--agent',agent,'--scope','user','--yes']);
    assert(existsSync(join(nativeHome,folder,'skills','rust-cache-management','SKILL.md')));
  }
  const hermesProjectHome=makeHome('unsupported hermes project');
  const unsupportedBefore=snapshot(hermesProjectHome);
  run(hermesProjectHome,['setup','--agent','hermes','--scope','project','--project',project,'--yes'],3);
  assert.equal(snapshot(hermesProjectHome),unsupportedBefore);
  const manualHome=makeHome('manual user');
  const manual=join(manualHome,'.agents','skills','manual-name');copyManual(manual);
  const manualBefore=snapshot(manualHome);setup(manualHome,['--yes']);assert.equal(snapshot(manualHome),manualBefore,'Matching manual installation changed');
  copyManual(join(home,'.agents','skills','different-name'));
  const duplicateBefore=snapshot(home);setup(home,['--yes'],3);assert.equal(snapshot(home),duplicateBefore,'Duplicate registration changed');
  const modifiedHome=makeHome('modified user');setup(modifiedHome,['--yes']);
  const modified=join(modifiedHome,'.agents','skills','rust-cache-management','SKILL.md');writeFileSync(modified,readFileSync(modified,'utf8')+'\nLocal user edit\n');
  const modifiedBefore=snapshot(modifiedHome);setup(modifiedHome,['--yes'],3);assert.equal(snapshot(modifiedHome),modifiedBefore,'Local edit was overwritten');
  if(process.platform==='win32') {
    // Native directory junctions need no symlink privilege. Never point outside this owned fixture.
    const linkedHome=makeHome('junction write ancestor user');
    const writeOutside=join(temp,'junction outside write target');mkdirSync(writeOutside);
    writeFileSync(join(writeOutside,'user-file'),'preserve outside bytes');
    symlinkSync(writeOutside,join(linkedHome,'.agents'),'junction');
    const linkedBefore=snapshot(linkedHome,true),outsideBefore=snapshot(writeOutside);
    setup(linkedHome,['--yes'],3);
    assert.equal(snapshot(linkedHome,true),linkedBefore,'Linked write ancestor changed home state');
    assert.equal(snapshot(writeOutside),outsideBefore,'Linked write ancestor caused outside writes');
    assert(!existsSync(join(writeOutside,'skills')),'Setup followed a junction for writes');

    const unrelatedHome=makeHome('unrelated junction skill user');
    const unrelatedRoot=join(unrelatedHome,'.agents','skills');mkdirSync(unrelatedRoot,{recursive:true});
    const unrelatedOutside=join(temp,'unrelated junction skill target');mkdirSync(unrelatedOutside);
    writeFileSync(join(unrelatedOutside,'SKILL.md'),'---\nname: unrelated-fixture\ndescription: Read-only unrelated junction fixture\n---\nPreserve this manual skill.\n');
    writeFileSync(join(unrelatedOutside,'supporting-file'),'unchanged');
    const unrelatedLink=join(unrelatedRoot,'unrelated-link');symlinkSync(unrelatedOutside,unrelatedLink,'junction');
    const unrelatedBefore=snapshot(unrelatedOutside),linkBefore=readlinkSync(unrelatedLink);
    setup(unrelatedHome,['--dry-run']);setup(unrelatedHome,['--yes']);
    assert.equal(snapshot(unrelatedOutside),unrelatedBefore,'Reading unrelated junction changed its files');
    assert.equal(readlinkSync(unrelatedLink),linkBefore,'Unrelated junction was replaced');
    assert(existsSync(join(unrelatedRoot,'rust-cache-management','SKILL.md')),'Unrelated read-only junction blocked safe registration');

    const aliasHome=makeHome('duplicate junction aliases user');
    const aliasRoot=join(aliasHome,'.agents','skills');mkdirSync(aliasRoot,{recursive:true});
    const aliasOutside=join(temp,'rust skill junction target');copyManual(aliasOutside);
    symlinkSync(aliasOutside,join(aliasRoot,'first-alias'),'junction');
    symlinkSync(aliasOutside,join(aliasRoot,'second-alias'),'junction');
    const aliasesBefore=snapshot(aliasHome,true),aliasOutsideBefore=snapshot(aliasOutside);
    setup(aliasHome,['--yes'],3);
    assert.equal(snapshot(aliasHome,true),aliasesBefore,'Duplicate junction aliases changed registration state');
    assert.equal(snapshot(aliasOutside),aliasOutsideBefore,'Duplicate junction aliases changed source skill');
    console.log('Native Windows junction cases passed: linked write ancestor refused without outside writes; unrelated read-only skill preserved; two Rust aliases conflict3.');
  }
  if(wrapper) {
    const wrappedHome=makeHome('wrapper user');
    const wrappedProject=join(temp,"wrapper project ' unicode 한글");mkdirSync(wrappedProject);
    for(const name of ['auto-update','last-check']) {
      const path=join(installation,name);assert(!existsSync(path),'Wrapper bypass fixture requires initial auto-update off');
      writeFileSync(path,name==='last-check'?'0\n':'enabled\n');wrapperState.push(path);
    }
    if(process.platform==='win32')windowsHostControls(wrappedHome);
    const wrappedBefore=snapshot(wrappedHome);
    console.error('Agent setup fixture stage: management wrapper preview');
    const preview=runWrapped(wrappedHome,['setup','--agent','codex','--scope','project','--project',wrappedProject,'--dry-run']);
    assert.equal(preview.status,'planned');
    assert(preview.actions[0].target.endsWith(join("wrapper project ' unicode 한글",'.agents','skills','rust-cache-management')),'Wrapper JSON target lost Unicode/path arguments');
    console.error('Agent setup fixture stage: management wrapper missing consent');
    assert.equal(runWrapped(wrappedHome,['setup','--agent','codex','--scope','project','--project',wrappedProject],4).status,'consent_required');
    assert.equal(runWrapped(wrappedHome,['setup','--agent','','--scope','user','--dry-run'],2).status,'invalid','Empty selector must fail without writes');
    assert.equal(snapshot(wrappedHome),wrappedBefore);assert(!existsSync(join(wrappedProject,'.agents')));
    assert.equal(readFileSync(join(installation,'last-check'),'utf8'),'0\n','Agent forwarding performed an automatic update check');
    assert.equal(readFileSync(join(installation,'auto-update'),'utf8'),'enabled\n');
  }
  assert.equal(environmentDigest(),parentEnv,'Parent environment changed');
  console.error('Agent setup fixture stage: behavior checks complete; cleanup');
} catch(error) {
  primaryError=error;
  // Print first: Windows file locks must never hide the original assertion/process failure.
  console.error('Original agent setup fixture failure:',error?.stack ?? String(error));
} finally {
  for(const path of wrapperState) {
    try { unlinkSync(path); } catch(error) { cleanupErrors.push(error);console.error('Agent setup marker cleanup failure:',error?.stack ?? String(error)); }
  }
  const cleanupRetries=process.platform==='win32'?10:0;
  for(let attempt=0;attempt<=cleanupRetries;attempt++) {
    try { rmSync(temp,{recursive:true,force:true,maxRetries:0});break; }
    catch(error) {
      if(attempt===cleanupRetries) { cleanupErrors.push(error);console.error('Agent setup directory cleanup failure:',error?.stack ?? String(error)); }
      else { console.error('Agent setup cleanup retry:',JSON.stringify({attempt:attempt+1,code:error.code}));await wait(100*(attempt+1)); }
    }
  }
}
if(primaryError)throw primaryError;
if(cleanupErrors.length)throw new AggregateError(cleanupErrors,'Agent setup fixture cleanup failed');
console.log('Native packaged agent setup passed: JSON, preview, consent, user/project apply, shared/native5provider mappings, all12textfiles, repeat, manual reuse, duplicates, modified-file preservation, Unicode paths.'+(wrapper?' Management wrapper preview/exit4 and auto-check bypass passed.':''));
