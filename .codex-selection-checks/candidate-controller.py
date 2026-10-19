import hashlib,json,os,pathlib,subprocess,time,traceback
own=pathlib.Path(__file__).resolve().parent.parent;out=own/'.codex-selection-checks';snapshot=out/'current-candidate';freeze=json.loads((out/'source-freeze.json').read_text());current=pathlib.Path(freeze['source_target']);os.chdir(own)
def sha(b):return hashlib.sha256(b).hexdigest()
def guard(p):return sha(subprocess.check_output(['git','diff','--binary','HEAD'],cwd=p))
def inputs():return {p:sha((current/p).read_bytes()) for p in freeze['inputs']}
def write(name,obj):
 p=out/name;t=p.with_suffix('.tmp');t.write_text(json.dumps(obj,indent=2)+'\n');t.replace(p)
def side_inputs():
 paths=sorted(snapshot.rglob('*.rs'))+sorted(snapshot.rglob('Cargo.toml'))+[snapshot/'Cargo.lock',out/'candidate-controller.py',out/'issue593-8d0cede.patch']
 paths += sorted((out/'current-baseline').rglob('*.rs'))+sorted((out/'current-baseline').rglob('Cargo.toml'))
 return {str(p.relative_to(own)):sha(p.read_bytes()) for p in paths}
side_start=side_inputs();own_start=guard(own);review_start=guard(current);result={'own_guard_before':own_start,'review_guard_before':review_start,'side_input_hashes':side_start,'steps':[]};code=1
def check():
 assert guard(own)==own_start,'own source changed'
 assert inputs()==freeze['inputs'],'review tools/dependency source changed'
 assert side_inputs()==side_start,'snapshot source changed'
env=os.environ.copy();env.update(CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(own/'target'))
for n in ['CRAFT_FONTS_DIR','CRAFT_FONTS_REQUIRED']:env.pop(n,None)
def run(name,args):
 check();started=time.time()
 with (out/(name+'.log')).open('w') as log:
  p=subprocess.Popen(args,cwd=snapshot,env=env,stdout=log,stderr=subprocess.STDOUT)
  write('candidate-running.json',{'controller_pid':os.getpid(),'child_pid':p.pid,'step':name})
  status=p.wait()
 check();step={'step':name,'exit_code':status,'elapsed_seconds':time.time()-started};result['steps'].append(step);write(name+'.exit.json',step);assert status==0,step
try:
 run('candidate-build',['cargo','test','--offline','--locked','-p','vectorcraft-tools','--lib','--no-run','--message-format=json'])
 binary=None
 for line in (out/'candidate-build.log').read_text().splitlines():
  try:event=json.loads(line)
  except ValueError:continue
  if event.get('reason')=='compiler-artifact' and event.get('executable') and event.get('target',{}).get('name')=='vectorcraft_tools':binary=event['executable']
 assert binary,'test executable missing';result['test_executable']=binary
 run('candidate-direct',[binary,'direct::','--test-threads=1'])
 run('candidate-selection',[binary,'select::tests::','--test-threads=1'])
 run('candidate-fmt',['rustfmt','--edition','2024','--check',str(snapshot/'crates/tools/src/direct.rs'),str(own/'crates/tools/src/direct.rs')])
 run('candidate-clippy',['cargo','clippy','--offline','--locked','-p','vectorcraft-tools','--lib','--tests','--','-D','warnings'])
 run('candidate-apply-check',['git','-C',str(current),'apply','--check',str(out/'issue593-8d0cede.patch')])
 run('candidate-diff-check',['git','-C',str(own),'diff','--check','--','crates/tools/src/direct.rs'])
 code=0
except Exception as e:result['error']=str(e);traceback.print_exc()
finally:
 trace=sha(subprocess.check_output(['git','diff','--binary','HEAD','--','ROADMAP.md','crates/trace'],cwd=own))
 result.update(exit_code=code,own_guard_after=guard(own),review_guard_after=guard(current),current_inputs_unchanged=inputs()==freeze['inputs'],side_inputs_unchanged=side_inputs()==side_start,trace_guard_after=trace,trace_unchanged=trace==freeze['own_guard'],finished=time.time());write('candidate-finished.json',result)
raise SystemExit(code)
