import hashlib,json,os,pathlib,subprocess,time
root=pathlib.Path(__file__).resolve().parent.parent
out=root/'.codex-trace-checks'
os.chdir(root)
def guard(): return hashlib.sha256(subprocess.check_output(['git','diff','--binary','HEAD'])).hexdigest()
env=os.environ.copy()
env.update(CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(root/'target'))
start_guard=guard()
deps=root/'target/debug/deps'
extern=[]
for name in ['vectorcraft_geom','vectorcraft_pathops','image','serde','thiserror','kurbo','serde_json']:
 path=(deps/'libserde_json-160e40cebf7c5b27.rlib') if name=='serde_json' else max(deps.glob('lib'+name+'-*.rlib'),key=lambda p:p.stat().st_mtime_ns)
 extern+=['--extern',name+'='+str(path)]
runner=out/'round1-upstream-tests'
commands=[['rustc','--test','--edition=2024','--crate-name','vectorcraft_trace','-C','opt-level=1','-C','debug-assertions=yes','-C','debuginfo=0','-L','dependency='+str(deps),*extern,str(out/'round1-upstream-tested/lib.rs'),'-o',str(runner)],[str(runner),'--test-threads=1']]
results=[]
for i,cmd in enumerate(commands):
 with (out/('round1-upstream-errors-'+str(i)+'.log')).open('w') as log:
  proc=subprocess.Popen(cmd,env=env,stdout=log,stderr=subprocess.STDOUT)
  (out/'round1-errors-running.json').write_text(json.dumps({'controller_pid':os.getpid(),'child_pid':proc.pid,'step':i,'guard':start_guard},indent=2))
  code=proc.wait()
 results.append({'exit_code':code,'step':i})
 if code:break
(out/'round1-errors-finished.json').write_text(json.dumps({'results':results,'guard_before':start_guard,'guard_after':guard(),'sources_unchanged':guard()==start_guard},indent=2)+'\n')
