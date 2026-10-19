import hashlib,json,os,pathlib,resource,subprocess,time,traceback
root=pathlib.Path(__file__).resolve().parent.parent;out=root/'.codex-trace-checks';os.chdir(root)
def sha(data):return hashlib.sha256(data).hexdigest()
def guard():return sha(subprocess.check_output(['git','diff','--binary','HEAD']))
def write(name,value):
 p=out/name;t=p.with_suffix('.tmp');t.write_text(json.dumps(value,indent=2)+'\n');t.replace(p)
def hashes():
 files=[*sorted((out/'round3-frozen').rglob('*.rs')),out/'round3-frozen/ROADMAP.md',out/'round3-cumulative-c26.patch',out/'round4-controller.py',out/'round4-dimensions.rs',out/'round4-lifetime.rs']
 for flavor in ['native','scaled']:files+=sorted((out/('round4-'+flavor+'-source')).glob('*.rs'))
 return {str(p.relative_to(root)):sha(p.read_bytes()) for p in files}
start=guard();before=hashes();result={'guard_before':start,'input_hashes':before,'steps':[]};code=1
try:
 freeze=json.loads((out/'round3-freeze.json').read_text());assert freeze['guard']==start
 for f,h in freeze['frozen_source_sha256'].items():assert sha((out/'round3-frozen'/f).read_bytes())==h
 c=json.loads((out/'round2-current2-finished.json').read_text());assert c['exit_code']==0
 external={n:root/v['path'] for n,v in c['external_rlibs'].items()}
 for n,v in c['external_rlibs'].items():assert sha(external[n].read_bytes())==v['sha256']
 for n in ['vectorcraft_geom','vectorcraft_pathops','baseline']:
  external[n]=root/c['built_rlibs'][n]['path'];assert sha(external[n].read_bytes())==c['built_rlibs'][n]['sha256']
 external['candidate']=out/'libvectorcraft_trace_round3_guarded.rlib'
 deps=root/'target/debug/deps'
 def build(crate,source,needed,target,library=False):
  args=['rustc','--edition=2024','--crate-name',crate,'-C','opt-level=1','-C','debug-assertions=yes','-C','debuginfo=0','-C','metadata=round4_'+target.name,'-L','dependency='+str(deps),'-L','dependency='+str(out)]
  if library:args+=['--crate-type','rlib']
  for n in needed:args+=['--extern',n+'='+str(external[n])]
  return args+[str(source),'-o',str(target)]
 commands=[('dimensions-build',build('trace_round4_dimensions',out/'round4-dimensions.rs',['baseline','candidate'],out/'round4-dimensions')),('dimensions',[str(out/'round4-dimensions')])]
 needs=['vectorcraft_geom','vectorcraft_pathops','image','serde','thiserror','kurbo']
 for flavor in ['scaled','native']:
  library=out/('libvectorcraft_trace_round4_'+flavor+'.rlib');external['instrumented']=library
  runner=out/('round4-'+flavor+'-probe')
  commands.append((flavor+'-library',build('vectorcraft_trace',out/('round4-'+flavor+'-source/lib.rs'),needs,library,True)))
  commands.append((flavor+'-build',build('trace_round4_'+flavor,out/'round4-lifetime.rs',['instrumented'],runner)))
  modes=['single','cancel-recovery','global','mixed'] if flavor=='scaled' else ['panic-recovery']
  commands.extend((flavor+'-'+mode,[str(runner),mode]) for mode in modes)
 env=os.environ.copy();env.update(CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(root/'target'),MALLOC_ARENA_MAX='1')
 def limits():resource.setrlimit(resource.RLIMIT_AS,(4*(64<<20),4*(64<<20)));resource.setrlimit(resource.RLIMIT_CPU,(30,30))
 for name,cmd in commands:
  assert guard()==start and hashes()==before
  begin=time.time()
  running_probe=not name.endswith(('build','library'))
  with (out/('round4-'+name+'.log')).open('w') as log:
   child=subprocess.Popen(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT,preexec_fn=limits if running_probe else None)
   write('round4-running.json',{'controller_pid':os.getpid(),'child_pid':child.pid,'step':name,'guard':start})
   try:status=child.wait(timeout=45)
   except subprocess.TimeoutExpired:child.kill();child.wait();raise RuntimeError('bounded timeout '+name)
  item={'step':name,'exit_code':status,'elapsed_seconds':time.time()-begin,'guard_after':guard(),'inputs_unchanged':hashes()==before};result['steps'].append(item);write('round4-'+name+'.exit.json',item)
  assert status==0,name
  assert guard()==start and hashes()==before
 code=0
except Exception as e:result['error']=str(e);traceback.print_exc()
finally:result.update(exit_code=code,guard_after=guard(),sources_unchanged=guard()==start,inputs_unchanged=hashes()==before);write('round4-finished.json',result)
raise SystemExit(code)
