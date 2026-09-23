use emulsiv_libos::{self as os,bus::{RegisterIo,TEXT_CTRL,TEXT_DATA,TEXT_OUT,GPIO_BASE,FRAMEBUFFER}};
use os::bitmap::{self,Bitmap,Color,PixelTarget};
use std::{collections::VecDeque,cell::RefCell};

struct MockBus {ctrl:u8,input:u8,output:Vec<u8>,gpio:[u32;5],inputs:u32,pixels:[u8;1024],writes:usize}
impl Default for MockBus {
    fn default()->Self {Self {ctrl:0,input:0,output:Vec::new(),gpio:[u32::MAX,0,0,0,0],inputs:0,pixels:[0;1024],writes:0}}
}
impl RegisterIo for MockBus {
    fn read8(&mut self,a:usize)->u8 {
        match a {TEXT_CTRL=>self.ctrl,TEXT_DATA=>self.input,
            FRAMEBUFFER..=0xfff=>self.pixels[a-FRAMEBUFFER],_=>panic!("bad read8 {a:x}")}
    }
    fn write8(&mut self,a:usize,v:u8) {
        match a {TEXT_CTRL=>self.ctrl=v&0xc0,TEXT_OUT=>self.output.push(v),
            FRAMEBUFFER..=0xfff=>{self.pixels[a-FRAMEBUFFER]=v;self.writes+=1;},_=>panic!("bad write8 {a:x}")}
    }
    fn read32(&mut self,a:usize)->u32 {
        assert!((GPIO_BASE..=GPIO_BASE+16).contains(&a)&&a&3==0);
        let i=(a-GPIO_BASE)/4;
        if i==4 {self.inputs&self.gpio[0]|self.gpio[4]&!self.gpio[0]} else {self.gpio[i]}
    }
    fn write32(&mut self,a:usize,v:u32) {
        assert!((GPIO_BASE..=GPIO_BASE+16).contains(&a)&&a&3==0);
        self.gpio[(a-GPIO_BASE)/4]=v;
    }
}
#[test] fn text_empty_poll() {
    let mut b=MockBus::default();assert_eq!(os::textio::TextIo::new(&mut b).try_read(),None);
}
#[test] fn text_ack_preserves_irq_enable() {
    let mut b=MockBus::default();b.ctrl=0xc0;b.input=b'X';
    assert_eq!(os::textio::TextIo::new(&mut b).try_read(),Some(b'X'));assert_eq!(b.ctrl,0x80);
}
#[test] fn text_enable_preserves_pending() {
    let mut b=MockBus::default();b.ctrl=0x40;
    os::textio::TextIo::new(&mut b).set_interrupts(true);assert_eq!(b.ctrl,0xc0);
    os::textio::TextIo::new(&mut b).set_interrupts(false);assert_eq!(b.ctrl,0x40);
}
#[test] fn text_poll_budget() {
    let mut b=MockBus::default();b.ctrl=0x40;b.input=b'A';
    assert_eq!(os::textio::TextIo::new(&mut b).read_with_budget(0),None);
    assert_eq!(os::textio::TextIo::new(&mut b).read_with_budget(1),Some(b'A'));
}
#[test] fn tiny_numeric_output() {
    let mut b=MockBus::default();let mut io=os::textio::TextIo::new(&mut b);
    io.write_u32(0);io.put_byte(b' ');io.write_u32(u32::MAX);io.put_byte(b' ');
    io.write_i32(i32::MIN);io.put_byte(b' ');io.write_hex32(0x1234abcd);
    assert_eq!(b.output,b"0 4294967295 -2147483648 0x1234abcd");
}
#[test] fn gpio_direction_polarity() {
    let mut b=MockBus::default();os::gpio::Gpio::new(&mut b).configure_outputs(1);
    assert_eq!(b.gpio[0],0xffff_fffe);
    os::gpio::Gpio::new(&mut b).configure_inputs(1);assert_eq!(b.gpio[0],u32::MAX);
}
#[test] fn gpio_mask_only_outputs() {
    let mut b=MockBus::default();b.gpio[0]=!3;b.gpio[4]=1;b.inputs=0x8000_0000;
    os::gpio::Gpio::new(&mut b).write_masked(u32::MAX,2);
    assert_eq!(b.gpio[4]&3,2);assert_eq!(os::gpio::Gpio::new(&mut b).read(),0x8000_0002);
}
#[test] fn gpio_events_not_write_one_clear() {
    let mut b=MockBus::default();b.gpio[2]=0b1011;b.gpio[3]=0b1100;
    os::gpio::Gpio::new(&mut b).clear_edges(0b0011);
    assert_eq!(b.gpio[2],0b1000);assert_eq!(b.gpio[3],0b1100);
    os::gpio::Gpio::new(&mut b).clear_all_edges();assert_eq!(&b.gpio[2..4],&[0,0]);
}
#[test] fn gpio_toggle_is_masked() {
    let mut b=MockBus::default();b.gpio[0]=!3;b.gpio[4]=1;
    os::gpio::Gpio::new(&mut b).toggle(3);assert_eq!(b.gpio[4]&3,2);
}
#[test] fn pin_boundaries() {
    assert_eq!(os::gpio::Pin::new(31).unwrap().mask(),0x8000_0000);
    assert!(os::gpio::Pin::new(32).is_none());assert!(os::gpio::Pin::new(255).is_none());
}
#[test] fn pixel_address_and_color() {
    let mut b=MockBus::default();Bitmap::new(&mut b).pixel(31,31,Color::RED);
    assert_eq!(b.pixels[1023],0x80);assert_eq!(Color::from_bits(0xff),Color::WHITE);
    assert_eq!(Color::from_rgb(false,true,true),Color::CYAN);
}
#[test] fn pixels_outside_are_ignored() {
    let mut b=MockBus::default();let mut p=Bitmap::new(&mut b);
    for (x,y) in [(-1,0),(0,-1),(32,0),(0,32),(i16::MIN,i16::MAX)] {p.pixel(x,y,Color::WHITE);}
    assert_eq!(p.get_pixel(-1,0),None);assert_eq!(b.writes,0);
}
#[test] fn framebuffer_clear() {
    let mut b=MockBus::default();Bitmap::new(&mut b).clear(Color::BLUE);
    assert!(b.pixels.iter().all(|&p|p==0x20));assert_eq!(b.writes,1024);
}
#[test] fn fill_rectangle_clips() {
    let mut b=MockBus::default();bitmap::fill_rect(&mut Bitmap::new(&mut b),-2,-2,4,4,Color::RED);
    assert_eq!(b.pixels.iter().filter(|&&p|p!=0).count(),4);
}
#[test] fn extreme_rectangle_does_not_overflow() {
    let mut b=MockBus::default();bitmap::rect(&mut Bitmap::new(&mut b),i16::MAX,i16::MAX,u16::MAX,u16::MAX,Color::RED);
    assert_eq!(b.writes,0);
}
#[test] fn line_diagonal() {
    let mut b=MockBus::default();bitmap::line(&mut Bitmap::new(&mut b),0,0,31,31,Color::WHITE);
    assert_eq!(b.pixels.iter().filter(|&&p|p!=0).count(),32);
    for i in 0..32 {assert_eq!(b.pixels[i*33],0xe0);}
}
#[test] fn line_reverse_and_negative() {
    let mut b=MockBus::default();bitmap::line(&mut Bitmap::new(&mut b),4,0,-4,0,Color::GREEN);
    assert_eq!(b.pixels.iter().filter(|&&p|p!=0).count(),5);
}
#[test] fn circle_zero_radius() {
    let mut b=MockBus::default();bitmap::circle(&mut Bitmap::new(&mut b),8,8,0,Color::CYAN);
    assert_eq!(b.pixels.iter().filter(|&&p|p!=0).count(),1);
}
#[test] fn scroll_retains_order() {
    let mut b=MockBus::default();for y in 0..32 {b.pixels[y*32..y*32+32].fill(((y%8) as u8)<<5);}
    Bitmap::new(&mut b).scroll_up(1,Color::WHITE);assert_eq!(b.pixels[0],0x20);assert_eq!(b.pixels[1023],0xe0);
    Bitmap::new(&mut b).scroll_up(255,Color::BLACK);assert!(b.pixels.iter().all(|&v|v==0));
}
#[test] fn sprite_truncated_is_transactional() {
    let mut b=MockBus::default();
    assert!(bitmap::blit_mono(&mut Bitmap::new(&mut b),0,0,9,1,&[0xff],Color::RED,None).is_err());
    assert_eq!(b.writes,0);
}
#[test] fn mono_buffer_is_small_and_correct() {
    assert_eq!(core::mem::size_of::<bitmap::MonoBuffer>(),128);
    let mut m=bitmap::MonoBuffer::new();m.pixel(0,0,Color::WHITE);m.pixel(31,31,Color::RED);
    assert_eq!(m.bytes()[0],0x80);assert_eq!(m.bytes()[127],1);
    let mut b=MockBus::default();m.present(&mut Bitmap::new(&mut b),Color::CYAN,Color::BLACK);
    assert_eq!(b.pixels[0],0x60);assert_eq!(b.pixels[1023],0x60);
}
#[test] fn font_space_is_empty() {
    let mut b=MockBus::default();bitmap::draw_char(&mut Bitmap::new(&mut b),0,0,b' ',Color::WHITE);
    assert_eq!(b.writes,0);
}
#[test] fn zero_queue() {
    let mut q=os::queue::Queue::<u8,0>::new();assert_eq!(q.push(1),Err(1));assert_eq!(q.pop(),None);
}
#[test] fn queue_wrap_and_overflow() {
    let mut q=os::queue::Queue::<u8,2>::new();assert_eq!(q.push(1),Ok(()));assert_eq!(q.push(2),Ok(()));
    assert_eq!(q.push(3),Err(3));assert_eq!(q.pop(),Some(1));assert_eq!(q.push(3),Ok(()));
    assert_eq!(q.pop(),Some(2));assert_eq!(q.pop(),Some(3));assert!(q.is_empty());
}
#[test] fn queue_matches_reference_over_many_operations() {
    let mut q=os::queue::Queue::<u32,7>::new();let mut reference=VecDeque::new();let mut rng=os::rng::XorShift32::new(1);
    for _ in 0..5000 {
        let n=rng.next_u32();
        if n&1==0 {let result=q.push(n);if reference.len()<7 {reference.push_back(n);assert!(result.is_ok());} else {assert_eq!(result,Err(n));}}
        else {assert_eq!(q.pop(),reference.pop_front());}
        assert_eq!(q.len(),reference.len());assert_eq!(q.front(),reference.front().copied());
    }
}
#[test] fn fixed_string_utf8_and_atomic_failure() {
    let mut s=os::fixed::FixedString::<8>::new();s.push_str("你好").unwrap();s.push('a').unwrap();
    assert!(s.push('🦀').is_err());assert_eq!(s.as_str(),"你好a");assert_eq!(s.pop(),Some('a'));
    assert_eq!(s.pop(),Some('好'));assert_eq!(s.as_str(),"你");
}
#[test] fn fixed_zero_capacity() {
    let mut s=os::fixed::FixedString::<0>::new();assert!(s.push('a').is_err());assert!(s.push_str("").is_ok());assert_eq!(s.pop(),None);
}
#[test] fn line_editing() {
    use os::console::{LineEditor,LineEvent};let mut e=LineEditor::<8>::new();
    e.feed(b'a');e.feed(b'b');assert_eq!(e.feed(8),LineEvent::Erase);e.feed(b'c');
    assert_eq!(e.feed(b'\r'),LineEvent::Complete);assert_eq!(e.line(),"ac");
    assert_eq!(e.feed(b'\n'),LineEvent::None);e.feed(b'z');assert_eq!(e.line(),"z");
}
#[test] fn overflow_is_not_a_truncated_command() {
    use os::console::{LineEditor,LineEvent};let mut e=LineEditor::<2>::new();
    for b in b"abc" {e.feed(*b);}assert_eq!(e.feed(b'\n'),LineEvent::Overflow);
    e.feed(b'X');assert_eq!(e.feed(b'\n'),LineEvent::Complete);assert_eq!(e.line(),"X");
}
#[test] fn control_clear_and_cancel() {
    use os::console::{LineEditor,LineEvent};let mut e=LineEditor::<8>::new();e.feed(b'a');
    assert_eq!(e.feed(21),LineEvent::Cleared);assert_eq!(e.line(),"");e.feed(b'b');
    assert_eq!(e.feed(3),LineEvent::Cancelled);assert_eq!(e.line(),"");
}
#[test] fn debounce_ignores_bounce() {
    let mut d=os::input::Debouncer::new(false,3);
    for v in [true,false,true,true] {assert_eq!(d.sample(v),None);}
    assert_eq!(d.sample(true),Some(os::input::Edge::Rising));assert!(d.value());
    assert_eq!(d.sample(true),None);
}
#[test] fn pwm_boundaries() {
    assert!(os::input::Pwm::new(0,0).is_none());assert!(os::input::Pwm::new(4,5).is_none());
    let off=os::input::Pwm::new(4,0).unwrap();let on=os::input::Pwm::new(4,4).unwrap();
    assert!((0..20).all(|t|!off.level(t)&&on.level(t)));
    let half=os::input::Pwm::new(4,2).unwrap();assert_eq!((0..100).filter(|&t|half.level(t)).count(),50);
}
#[test] fn deadline_wrap() {
    let d=os::time::Deadline::after(u32::MAX-2,5).unwrap();assert_eq!(d.tick(),2);
    assert!(!d.expired(1));assert!(d.expired(2));assert!(os::time::Deadline::after(0,0x8000_0000).is_err());
}
std::thread_local! {static LOG:RefCell<Vec<(u8,u32)>>=const {RefCell::new(Vec::new())};}
fn task_a(t:u32)->os::scheduler::TaskControl {LOG.with(|v|v.borrow_mut().push((0,t)));os::scheduler::TaskControl::Continue}
fn task_b(t:u32)->os::scheduler::TaskControl {LOG.with(|v|v.borrow_mut().push((1,t)));os::scheduler::TaskControl::Finished}
#[test] fn scheduler_budget_and_cancellation() {
    LOG.with(|v|v.borrow_mut().clear());let mut s=os::scheduler::Scheduler::<2>::new();
    let a=s.add(0,10,task_a).unwrap();let b=s.add(0,20,task_b).unwrap();
    assert_eq!(s.run_ready(0,1),1);assert_eq!(s.run_ready(0,1),1);assert_eq!(s.len(),1);
    assert!(!s.cancel(b));assert_eq!(s.run_ready(9,8),0);assert_eq!(s.run_ready(10,8),1);
    assert!(s.cancel(a));assert!(s.is_empty());
    LOG.with(|v|assert_eq!(*v.borrow(),vec![(0,0),(1,0),(0,10)]));
}
#[test] fn scheduler_wrap_and_stale_id() {
    let mut s=os::scheduler::Scheduler::<1>::new();let old=s.add(u32::MAX-2,5,task_a).unwrap();
    assert_eq!(s.run_ready(u32::MAX-2,1),1);assert_eq!(s.run_ready(1,1),0);assert_eq!(s.run_ready(2,1),1);
    assert!(s.cancel(old));let _new=s.add(2,1,task_a).unwrap();assert!(!s.cancel(old));
}
#[test] fn scheduler_rejects_bad_period_and_capacity() {
    use os::scheduler::{Scheduler,ScheduleError};let mut s=Scheduler::<0>::new();
    assert_eq!(s.add(0,0,task_a),Err(ScheduleError::InvalidPeriod));assert_eq!(s.add(0,1,task_a),Err(ScheduleError::Full));
    assert_eq!(s.run_ready(0,1),0);
}
#[test] fn arena_alignment_disjoint_and_zeroed() {
    let mut data=[0xffu8;64];let mut arena=os::arena::Arena::new(&mut data);
    let a=arena.allocate(7,8).unwrap();let b=arena.allocate(9,16).unwrap();
    assert_eq!(a.as_ptr() as usize%8,0);assert_eq!(b.as_ptr() as usize%16,0);
    a.fill(1);assert!(b.iter().all(|&v|v==0));assert!(b.as_ptr() as usize>=a.as_ptr() as usize+a.len());
}
#[test] fn arena_failures_preserve_cursor() {
    let mut data=[0u8;8];let mut a=os::arena::Arena::new(&mut data);
    assert!(a.allocate(1,3).is_none());assert!(a.allocate(0,1).is_none());assert!(a.allocate(9,1).is_none());assert_eq!(a.used(),0);
}
#[test] fn known_checksum_vectors() {
    assert_eq!(os::crc::crc16_ccitt(b"123456789"),0x29b1);assert_eq!(os::crc::crc32(b"123456789"),0xcbf43926);
    assert_eq!(os::crc::crc32(b""),0);assert_eq!(os::crc::crc16_ccitt(b""),0xffff);
}
#[test] fn rng_known_and_nonzero_seed() {
    let mut r=os::rng::XorShift32::new(1);assert_eq!(r.next_u32(),270369);assert_eq!(r.next_u32(),67634689);
    assert_ne!(os::rng::XorShift32::new(0).next_u32(),0);
}
#[test] fn numeric_parser_bases_and_overflow() {
    use os::shell::number;assert_eq!(number("0xff"),Ok(255));assert_eq!(number("0b101"),Ok(5));
    assert_eq!(number("4294967295"),Ok(u32::MAX));
    for s in ["","0x","-1","0b2","4294967296","0x100000000","1_0"] {assert!(number(s).is_err(),"{s}");}
}
#[test] fn command_parser_is_strict() {
    use os::shell::{parse,Command};assert_eq!(parse("p 31 0 0x80"),Ok(Command::Pixel{x:31,y:0,color:128}));
    for s in ["r 1","p 32 0 0","w","w 1 2","c 256","unknown"] {assert!(parse(s).is_err(),"{s}");}
}
