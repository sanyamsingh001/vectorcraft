use std::sync::{Barrier, Mutex, OnceLock, atomic::{AtomicUsize, Ordering}};
static FIRST: AtomicUsize=AtomicUsize::new(0);
static SCHEDULE: OnceLock<(usize,Barrier)>=OnceLock::new();
static RECORDS: Mutex<Vec<(usize,usize,usize)>>=Mutex::new(Vec::new());
pub fn configure(workers:usize) {let _=SCHEDULE.set((workers,Barrier::new(workers)));}
pub fn record(mask:usize,components:usize,edges:usize) {
    let ticket=FIRST.fetch_add(1,Ordering::SeqCst);
    if let Ok(mut r)=RECORDS.lock() {r.push((mask,components,edges));}
    // Only the first worker wave waits. All three scratch buffers remain live at this point.
    if let Some((workers,barrier))=SCHEDULE.get() {if ticket<*workers {barrier.wait();}}
}
pub fn records()->Vec<(usize,usize,usize)> {RECORDS.lock().map(|r|r.clone()).unwrap_or_default()}
