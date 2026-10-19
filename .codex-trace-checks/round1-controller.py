import hashlib,json,os,pathlib,subprocess,time
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
commands=[('round1-tests',['cargo','test','-p','vectorcraft-trace','--all-targets']),('round1-clippy',['cargo','clippy','-p','vectorcraft-trace','--all-targets','--','-D','warnings']),('round1-build',['cargo','build','-p','vectorcraft-trace']),('round1-fmt',['cargo','fmt','--all','--','--check'])]
results=[]
for name,cmd in commands:
 started=time.time()
 with (out/(name+'.log')).open('w') as log:
  proc=subprocess.Popen(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
  write('round1-running.json',{'controller_pid':os.getpid(),'child_pid':proc.pid,'command':cmd,'guard':start_guard,'started':started})
  _,status,usage=os.wait4(proc.pid,0)
  code=os.waitstatus_to_exitcode(status)
  proc.returncode=code
 result={'command':cmd,'exit_code':code,'elapsed_seconds':time.time()-started,'max_rss_kb':usage.ru_maxrss,'guard_before':start_guard,'guard_after':guard(),'sources_unchanged':start_guard==guard()}
 write(name+'.exit.json',result)
 results.append(result)
 if code or not result['sources_unchanged']: break
write('round1-finished.json',{'results':results,'guard':guard(),'sources_unchanged':guard()==start_guard,'finished':time.time()})
