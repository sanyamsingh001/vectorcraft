use std::ops::{Deref,DerefMut};
use std::sync::{Barrier,OnceLock,atomic::{AtomicBool,AtomicUsize,Ordering}};
static LIVE:AtomicUsize=AtomicUsize::new(0);
static PEAK:AtomicUsize=AtomicUsize::new(0);
static OPENED:AtomicUsize=AtomicUsize::new(0);
static CLOSED:AtomicUsize=AtomicUsize::new(0);
static STAGES:AtomicUsize=AtomicUsize::new(0);
static GATE:OnceLock<(usize,Barrier)>=OnceLock::new();
static INJECT_PANIC:AtomicBool=AtomicBool::new(false);
pub struct Tracked<T>{inner:Vec<T>,bytes:usize}
impl<T> Tracked<T>{pub fn new(inner:Vec<T>)->Self{let bytes=inner.capacity()*std::mem::size_of::<T>();let now=LIVE.fetch_add(bytes,Ordering::SeqCst)+bytes;PEAK.fetch_max(now,Ordering::SeqCst);OPENED.fetch_add(1,Ordering::SeqCst);Self{inner,bytes}}}
impl<T> Deref for Tracked<T>{type Target=Vec<T>;fn deref(&self)->&Self::Target{&self.inner}}
impl<T> DerefMut for Tracked<T>{fn deref_mut(&mut self)->&mut Self::Target{&mut self.inner}}
impl<T> Drop for Tracked<T>{fn drop(&mut self){let inner=std::mem::take(&mut self.inner);drop(inner);LIVE.fetch_sub(self.bytes,Ordering::SeqCst);CLOSED.fetch_add(1,Ordering::SeqCst);}}
pub fn configure_gate(n:usize){let _=GATE.set((n,Barrier::new(n)));}
pub fn request_panic(){INJECT_PANIC.store(true,Ordering::SeqCst);}
pub fn stage(){let ticket=STAGES.fetch_add(1,Ordering::SeqCst);if INJECT_PANIC.swap(false,Ordering::SeqCst){panic!("intentional local review panic after all fixed buffers allocate");}if let Some((n,b))=GATE.get(){if ticket<*n{b.wait();}}}
pub fn stats()->(usize,usize,usize,usize,usize){(LIVE.load(Ordering::SeqCst),PEAK.load(Ordering::SeqCst),OPENED.load(Ordering::SeqCst),CLOSED.load(Ordering::SeqCst),STAGES.load(Ordering::SeqCst))}
