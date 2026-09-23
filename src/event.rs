//! Typed messages for a foreground event loop. Queue storage is caller-owned.
use crate::{gpio::Edges,queue::Queue};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Event { Text(u8), Gpio(Edges), Tick(u32), User{kind:u8,value:u32} }
pub type EventQueue<const N:usize> = Queue<Event,N>;
