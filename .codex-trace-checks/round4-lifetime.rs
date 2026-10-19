use instrumented::{Raster,TraceParams,Mode,Method,TraceError,TraceResult,trace_within,codex_review_probe as p};
fn image()->Raster{Raster::from_fn(64,64,|x,_|{let c=x/4;[(c*16)as u8,((15-c)*16)as u8,(c*8)as u8,255]})}
fn params()->TraceParams{TraceParams{mode:Mode::Color,colors:16,noise:1,paths:100.0,corners:100.0,method:Method::Abutting,..Default::default()}}
fn full()->TraceResult{let r=trace_within(&image(),&params(),64).unwrap();assert_eq!(r.paths.len(),16);assert_eq!(r.anchor_count(),64);r}
fn main(){let mode=std::env::args().nth(1).unwrap();
 let expected_limit=instrumented::codex_review_worker_limit(64,64);
 match mode.as_str(){
  "single"=>{assert_eq!(expected_limit,1);p::configure_gate(1);let _=full();let s=p::stats();assert_eq!(s,(0,24705,48,48,16));}
  "cancel-recovery"=>{assert_eq!(expected_limit,1);p::configure_gate(1);assert!(matches!(trace_within(&image(),&params(),0),Err(TraceError::TooComplex{max:0})));assert_eq!(p::stats(),(0,24705,3,3,1));let _=full();assert_eq!(p::stats(),(0,24705,51,51,17));}
  "global"=>{assert_eq!(expected_limit,1);p::configure_gate(2);std::thread::scope(|scope|{let a=scope.spawn(full);let b=scope.spawn(full);assert_eq!(a.join().unwrap(),b.join().unwrap());});let s=p::stats();assert_eq!(s.0,0);assert_eq!(s.2,96);assert_eq!(s.3,96);assert_eq!(s.4,32);assert!(s.1>=2*24705&&s.1>7*4096+2,"two independent invocations exceed one allowance");}
  "mixed"=>{assert_eq!(expected_limit,1);p::configure_gate(2);std::thread::scope(|scope|{let a=scope.spawn(||trace_within(&image(),&params(),0));let b=scope.spawn(full);assert!(matches!(a.join().unwrap(),Err(TraceError::TooComplex{max:0})));assert_eq!(b.join().unwrap().anchor_count(),64);});let s=p::stats();assert_eq!(s.0,0);assert_eq!(s.2,51);assert_eq!(s.3,51);assert_eq!(s.4,17);assert!(s.1>=2*24705);}
  "panic-recovery"=>{assert!(expected_limit>=16);p::request_panic();let caught=std::panic::catch_unwind(full);assert!(caught.is_err());let after=p::stats();assert_eq!(after.0,0);assert_eq!(after.2,after.3,"all scoped workers and fixed buffers drained on unwind");let _=full();let s=p::stats();assert_eq!(s.0,0);assert_eq!(s.2,s.3);}
  _=>panic!("unknown local probe")
 }
 let s=p::stats();println!("{{\"mode\":\"{mode}\",\"worker_limit\":{expected_limit},\"live_fixed_bytes_at_exit\":{},\"peak_live_fixed_bytes\":{},\"tracked_buffers_allocated\":{},\"tracked_buffers_freed\":{},\"nonuniform_layers_entered\":{},\"only_64x64_inputs\":true}}",s.0,s.1,s.2,s.3,s.4);
}
