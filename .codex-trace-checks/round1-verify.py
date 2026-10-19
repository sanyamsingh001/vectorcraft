import hashlib,json,os,pathlib,subprocess,time,traceback
root=pathlib.Path(__file__).resolve().parent.parent
out=root/'.codex-trace-checks'
os.chdir(root)
def guard(): return hashlib.sha256(subprocess.check_output(['git','diff','--binary','HEAD'])).hexdigest()
def write(name,value):
 p=out/name
 tmp=p.with_suffix('.tmp')
 tmp.write_text(json.dumps(value,indent=2)+'\n')
 tmp.replace(p)
env=os.environ.copy()
env.update(CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(root/'target'))
start_guard=guard()
results={'guard_before':start_guard,'measurements':[],'builds':[]}
try:
 checked=json.loads((out/'round1-finished.json').read_text())
 assert checked['sources_unchanged'] and checked['guard']==start_guard
 assert all(r['exit_code']==0 for r in checked['results'])
 deps=root/'target/debug/deps'
 extern=[]
 for name in ['vectorcraft_geom','vectorcraft_pathops','image','serde','thiserror','kurbo']:
  path=max(deps.glob('lib'+name+'-*.rlib'),key=lambda p:p.stat().st_mtime_ns)
  extern+=['--extern',name+'='+str(path)]
 runners={}
 for flavor,source in [('original',out/'round1-reference/lib.rs'),('upstream557',out/'round1-upstream/crates/trace/src/lib.rs'),('upstream557_optimized',out/'round1-upstream-optimized/lib.rs'),('candidate',None)]:
  lib=(out/('libvectorcraft_trace_round1_'+flavor+'.rlib')).resolve() if source else root/'target/debug/libvectorcraft_trace.rlib'
  commands=[]
  if source and not lib.exists():
   commands.append(['rustc','--edition=2024','--crate-name','vectorcraft_trace','--crate-type','rlib','-C','opt-level=1','-C','debug-assertions=yes','-C','debuginfo=0','-C','metadata='+flavor,'-L','dependency='+str(deps),*extern,str(source),'-o',str(lib)])
  runner=(out/('round1-'+flavor+'-probe')).resolve()
  commands.append(['rustc','--edition=2024','--crate-name','trace_round1_probe','-C','opt-level=1','-C','debug-assertions=yes','-C','debuginfo=0','-L','dependency='+str(deps),'--extern','vectorcraft_trace='+str(lib),str(out/'round1-probe.rs'),'-o',str(runner)])
  with (out/('round1-'+flavor+'-compile.log')).open('w') as log:
   for cmd in commands:
    write('round1-verify-running.json',{'controller_pid':os.getpid(),'step':'compile '+flavor,'guard':start_guard})
    started=time.time()
    completed=subprocess.run(cmd,env=env,stdout=log,stderr=subprocess.STDOUT)
    results['builds'].append({'flavor':flavor,'exit_code':completed.returncode,'elapsed_seconds':time.time()-started})
    if completed.returncode: raise RuntimeError('compile failed: '+flavor)
  runners[flavor]=runner
 for flavor,runner in runners.items():
  with (out/('round1-'+flavor+'-parity.txt')).open('w') as log:
   subprocess.run([str(runner),'parity'],env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
 original=(out/'round1-original-parity.txt').read_bytes()
 upstream=(out/'round1-upstream557-parity.txt').read_bytes()
 results['parity']={'fixture_preset_combinations':84,'original_equals_candidate':original==(out/'round1-candidate-parity.txt').read_bytes(),'upstream_equals_optimized':upstream==(out/'round1-upstream557_optimized-parity.txt').read_bytes(),'original_equals_upstream':original==upstream,'sha256':hashlib.sha256(original).hexdigest()}
 assert all(results['parity'][k] for k in ['original_equals_candidate','upstream_equals_optimized','original_equals_upstream'])
 for flavor in ['upstream557','upstream557_optimized']:
  runner=runners[flavor]
  for args in [['1024','4'],['4096','4'],['4096','4294967295'],['photo','2048']]:
   logpath=out/('round1-memory-'+flavor+'-'+'-'.join(args)+'.log')
   with logpath.open('w') as log:
    started=time.time()
    proc=subprocess.Popen([str(runner),*args],env=env,stdout=log,stderr=subprocess.STDOUT)
    write('round1-verify-running.json',{'controller_pid':os.getpid(),'child_pid':proc.pid,'step':flavor+' '+' '.join(args),'guard':start_guard})
    _,status,usage=os.wait4(proc.pid,0)
    code=os.waitstatus_to_exitcode(status)
    proc.returncode=code
   item={'flavor':flavor,'args':args,'exit_code':code,'elapsed_seconds':time.time()-started,'max_rss_kb':usage.ru_maxrss,'output':logpath.read_text().strip()}
   results['measurements'].append(item)
   write('round1-measurements.json',results)
   if code: raise RuntimeError('probe failed: '+str(item))
except Exception as e:
 results['error']=str(e)
 traceback.print_exc()
finally:
 results.update(guard_after=guard(),sources_unchanged=guard()==start_guard)
 write('round1-verify-finished.json',results)
