//! Fixed-capacity cooperative periodic tasks, no stacks or preemption.
use crate::time::{reached,MAX_INTERVAL};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum TaskControl { Continue, Finished }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct TaskId {slot:usize,generation:u32}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ScheduleError { Full, InvalidPeriod }
#[derive(Clone,Copy)]
struct Task { next:u32,period:u32,run:fn(u32)->TaskControl,generation:u32 }
pub struct Scheduler<const N:usize> { tasks:[Option<Task>;N],generation:u32,cursor:usize }
impl<const N:usize> Default for Scheduler<N> {fn default()->Self {Self::new()}}
impl<const N:usize> Scheduler<N> {
    pub const fn new()->Self {Self {tasks:[None;N],generation:0,cursor:0}}
    /// First invocation is due at `now`. Generation wraps after 2^32 registrations.
    pub fn add(&mut self,now:u32,period:u32,run:fn(u32)->TaskControl)->Result<TaskId,ScheduleError> {
        if period==0 || period>MAX_INTERVAL {return Err(ScheduleError::InvalidPeriod);}
        for i in 0..N {
            if self.tasks[i].is_none() {
                self.generation=self.generation.wrapping_add(1);
                self.tasks[i]=Some(Task {next:now,period,run,generation:self.generation});
                return Ok(TaskId {slot:i,generation:self.generation});
            }
        }
        Err(ScheduleError::Full)
    }
    pub fn cancel(&mut self,id:TaskId)->bool {
        if let Some(slot)=self.tasks.get_mut(id.slot) {
            if slot.as_ref().map(|t|t.generation)==Some(id.generation) {*slot=None;return true;}
        }
        false
    }
    pub fn len(&self)->usize {self.tasks.iter().filter(|t|t.is_some()).count()}
    pub fn is_empty(&self)->bool {self.len()==0}
    /// At most `budget` callbacks; each task at most once. Missed periods are
    /// coalesced, rescheduling at now+period, not replayed in an unbounded burst.
    pub fn run_ready(&mut self,now:u32,budget:usize)->usize {
        if N==0 || budget==0 {return 0;}
        let start=self.cursor;let mut calls=0;
        for offset in 0..N {
            let i=(start+offset)%N;
            if let Some(mut task)=self.tasks[i] {
                if reached(now,task.next) {
                    let result=(task.run)(now);calls+=1;
                    if result==TaskControl::Finished {self.tasks[i]=None;}
                    else {task.next=now.wrapping_add(task.period);self.tasks[i]=Some(task);}
                    self.cursor=(i+1)%N;
                    if calls==budget {break;}
                }
            }
        }
        calls
    }
}
