//! Wrapping logical ticks. No wall clock, timer interrupt or millisecond promise.
//! Comparisons require times to differ by less than 2^31 ticks.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct IntervalTooLong;
pub const MAX_INTERVAL:u32=0x7fff_ffff;
pub const fn reached(now:u32,deadline:u32)->bool { now.wrapping_sub(deadline) as i32>=0 }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Deadline(u32);
impl Deadline {
    pub const fn after(now:u32,delay:u32)->Result<Self,IntervalTooLong> {
        if delay>MAX_INTERVAL {Err(IntervalTooLong)} else {Ok(Self(now.wrapping_add(delay)))}
    }
    pub const fn expired(self,now:u32)->bool { reached(now,self.0) }
    pub const fn tick(self)->u32 {self.0}
}
#[derive(Default)]
pub struct TickCounter(u32);
impl TickCounter {
    pub const fn new()->Self {Self(0)}
    pub const fn now(&self)->u32 {self.0}
    pub fn advance(&mut self,by:u32)->u32 {self.0=self.0.wrapping_add(by);self.0}
}
