use candidate::{Raster,TraceParams,TraceResult,TraceError,trace_within};
fn main(){
 let params=TraceParams{noise:0,..Default::default()};
 for (w,h) in [(0,0),(0,u32::MAX),(u32::MAX,0),(u32::MAX,1),(1,u32::MAX),(2,3)] {
  let image=Raster{width:w,height:h,rgba:Vec::new()};
  assert_eq!(trace_within(&image,&params,0).unwrap(),TraceResult::default());
 }
 let malformed=Raster{width:u32::MAX,height:u32::MAX,rgba:Vec::new()};
 let overflow=std::panic::catch_unwind(||trace_within(&malformed,&params,0));
 assert!(overflow.is_err(),"checked candidate panics on length arithmetic before validation returns");
 let original=baseline::Raster{width:u32::MAX,height:u32::MAX,rgba:Vec::new()};
 let old=std::panic::catch_unwind(||baseline::trace_within(&original,&baseline::TraceParams{noise:0,..Default::default()},0));
 assert!(old.is_err(),"unchecked entry multiplication predates the resource guard");
 let opaque=Raster::from_fn(1,1,|_,_|[0,0,0,255]);
 assert!(matches!(trace_within(&opaque,&params,0),Err(TraceError::TooComplex{max:0})));
 assert_eq!(trace_within(&opaque,&params,4).unwrap().anchor_count(),4);
 let empty=Raster::from_fn(1,1,|_,_|[0,0,0,0]);
 assert_eq!(trace_within(&empty,&params,0).unwrap(),TraceResult::default());
 println!("{{\"zero_or_invalid_length_cases_clean\":6,\"candidate_length_overflow_panics\":true,\"c26_baseline_same_panic\":true,\"tiny_zero_exact_empty_anchor_cases_clean\":3,\"giant_allocation_attempted\":false}}");
}
