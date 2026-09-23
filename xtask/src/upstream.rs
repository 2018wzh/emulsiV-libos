//! Rust bindings to the pinned upstream ES modules, executed by the Rust Boa engine.
//! All orchestration and assertions are Rust. The JavaScript files are upstream
//! test inputs, loaded without source rewriting. No Node process is started.
use crate::machine::{Rig, Snapshot};
use anyhow::{anyhow, ensure, Context as _, Result};
use boa_engine::{
    builtins::promise::PromiseState,
    module::SimpleModuleLoader,
    object::{FunctionObjectBuilder, JsObject},
    property::Attribute,
    Context, JsResult, JsString, JsValue, Module, NativeFunction, Source,
};
use std::{path::Path, rc::Rc};

fn js<T>(r: JsResult<T>) -> Result<T> {
    r.map_err(|e| anyhow!("upstream JavaScript: {e}"))
}
fn object(v: JsValue) -> Result<JsObject> {
    v.as_object().cloned().context("expected JavaScript object")
}
fn get(o: &JsObject, k: &str, c: &mut Context) -> Result<JsValue> {
    js(o.get(JsString::from(k), c))
}
fn number(o: &JsObject, k: &str, c: &mut Context) -> Result<u32> {
    Ok(js(get(o, k, c)?.to_i32(c))? as u32)
}
fn text(v: JsValue, c: &mut Context) -> Result<String> {
    Ok(js(v.to_string(c))?.to_std_string_escaped())
}
fn method(o: &JsObject, k: &str, a: &[JsValue], c: &mut Context) -> Result<JsValue> {
    let f = object(get(o, k, c)?)?;
    js(f.call(&o.clone().into(), a, c))
}
fn construct(ns: &JsObject, name: &str, a: &[JsValue], c: &mut Context) -> Result<JsObject> {
    js(object(get(ns, name, c)?)?.construct(a, None, c))
}
fn namespace(path: &Path, loader: &SimpleModuleLoader, c: &mut Context) -> Result<JsObject> {
    let module = js(Module::parse(Source::from_filepath(path)?, None, c))?;
    loader.insert(path.to_path_buf(), module.clone());
    let promise = module.load_link_evaluate(c);
    c.run_jobs();
    ensure!(
        matches!(promise.state(), PromiseState::Fulfilled(_)),
        "module evaluation failed: {:?}",
        promise.state()
    );
    Ok(module.namespace(c))
}

pub struct Upstream {
    context: Context,
    cpu: JsObject,
    bus: JsObject,
    ram: JsObject,
    input: JsObject,
    output: JsObject,
    gpio: JsObject,
    bitmap: JsObject,
    bitmap_namespace: JsObject,
    step_fn: JsObject,
    x: JsObject,
    text: String,
    steps: u64,
    irqs: u64,
    min_sp: u32,
    stack_started: bool,
    led_levels: u8,
}
impl Upstream {
    pub fn new(root: &Path, hex: &str) -> Result<Self> {
        let root = root.canonicalize()?;
        let loader = Rc::new(js(SimpleModuleLoader::new(&root))?);
        let mut c = js(Context::builder().module_loader(loader.clone()).build())?;
        let cpu_ns = namespace(&root.join("src/virgule.js"), &loader, &mut c)?;
        let text_ns = namespace(&root.join("src/devices/text.js"), &loader, &mut c)?;
        let gpio_ns = namespace(&root.join("src/devices/gpio.js"), &loader, &mut c)?;
        let bitmap_ns = namespace(&root.join("src/devices/bitmap.js"), &loader, &mut c)?;
        let hex_ns = namespace(&root.join("src/hex.js"), &loader, &mut c)?;
        let bus = construct(&cpu_ns, "Bus", &[], &mut c)?;
        let ram = construct(&cpu_ns, "Memory", &[0.into(), 4096.into()], &mut c)?;
        let input = construct(
            &text_ns,
            "TextInput",
            &[JsValue::new(0xb0000000u32)],
            &mut c,
        )?;
        let output = construct(
            &text_ns,
            "TextOutput",
            &[JsValue::new(0xc0000000u32), 4.into()],
            &mut c,
        )?;
        let gpio = construct(&gpio_ns, "GPIO", &[JsValue::new(0xd0000000u32)], &mut c)?;
        let bitmap = construct(
            &bitmap_ns,
            "BitmapOutput",
            &[3072.into(), 32.into(), 32.into()],
            &mut c,
        )?;
        for d in [&ram, &input, &output, &gpio, &bitmap] {
            method(&bus, "addDevice", &[d.clone().into()], &mut c)?;
        }
        method(&bus, "reset", &[], &mut c)?;
        let parse = object(get(&hex_ns, "parse", &mut c)?)?;
        js(parse.call(
            &JsValue::undefined(),
            &[JsString::from(hex).into(), bus.clone().into()],
            &mut c,
        ))?;
        ensure!(
            !get(&bus, "error", &mut c)?.to_boolean(),
            "upstream HEX load failed"
        );
        let cpu = construct(
            &cpu_ns,
            "Processor",
            &[32.into(), bus.clone().into()],
            &mut c,
        )?;
        let step_fn = object(get(&cpu, "step", &mut c)?)?;
        let x = object(get(&cpu, "x", &mut c)?)?;
        Ok(Self {
            context: c,
            cpu,
            bus,
            ram,
            input,
            output,
            gpio,
            bitmap,
            bitmap_namespace: bitmap_ns,
            step_fn,
            x,
            text: String::new(),
            steps: 0,
            irqs: 0,
            min_sp: 3072,
            stack_started: false,
            led_levels: 0,
        })
    }
    fn step(&mut self) -> Result<()> {
        let c = &mut self.context;
        let pc = number(&self.cpu, "pc", c)?;
        ensure!(pc < 0xc00 && pc % 4 == 0, "upstream invalid PC {pc:x}");
        loop {
            let state = text(get(&self.cpu, "state", c)?, c)?;
            js(self.step_fn.call(&self.cpu.clone().into(), &[], c))?;
            ensure!(
                !get(&self.cpu, "fetchError", c)?.to_boolean()
                    && !get(&self.cpu, "loadStoreError", c)?.to_boolean()
                    && !get(&self.bus, "error", c)?.to_boolean(),
                "upstream bus fault at {pc:x}"
            );
            if state == "decode" {
                let instr = object(get(&self.cpu, "instr", c)?)?;
                ensure!(
                    text(get(&instr, "name", c)?, c)? != "invalid",
                    "upstream invalid instruction at {pc:x}"
                );
            }
            let sp = js(js(self.x.get(2u32, c))?.to_i32(c))? as u32;
            if sp == 0xc00 {
                self.stack_started = true;
            }
            if self.stack_started {
                ensure!(
                    (0xa00..=0xc00).contains(&sp) && sp % 16 == 0,
                    "upstream stack outside reservation: {sp:x}"
                );
                self.min_sp = self.min_sp.min(sp);
            }
            if method(&self.output, "hasData", &[], c)?.to_boolean() {
                let data = method(&self.output, "getData", &[], c)?;
                self.text.push_str(&text(data, c)?);
            }
            if text(get(&self.cpu, "state", c)?, c)? == "fetch" {
                break;
            }
        }
        if get(&self.cpu, "acceptingIrq", c)?.to_boolean() {
            self.irqs += 1;
        }
        self.led_levels |= 1 << (number(&self.gpio, "ioStatus", c)? & 1);
        self.steps += 1;
        // The headless consumer drains the display queue like the browser view.
        // RAM retains all pixel values for the Rust behavioral assertions.
        if self.steps % 1024 == 0 {
            method(&self.bitmap, "getData", &[], c)?;
        }
        Ok(())
    }
    /// Invoke the upstream rendering method with Rust-native canvas callbacks.
    /// This checks the color protocol, not a real browser DOM or screen.
    pub fn check_colors(&mut self) -> Result<()> {
        fn canvas(_: &JsValue, _: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
            c.global_object().get(JsString::from("__canvas"), c)
        }
        fn paint(_: &JsValue, _: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
            c.global_object().get(JsString::from("__paint"), c)
        }
        fn fill(this: &JsValue, _: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
            let style = this
                .as_object()
                .unwrap()
                .get(JsString::from("fillStyle"), c)?;
            let fills = c.global_object().get(JsString::from("__fills"), c)?;
            let o = fills.as_object().unwrap();
            let push = o.get(JsString::from("push"), c)?;
            push.as_object().unwrap().call(&fills, &[style], c)
        }
        let c = &mut self.context;
        method(&self.bitmap, "getData", &[], c)?;
        let context = JsObject::with_object_proto(c.intrinsics());
        let f = FunctionObjectBuilder::new(c.realm(), NativeFunction::from_fn_ptr(fill)).build();
        js(context.set(JsString::from("fillRect"), f, true, c))?;
        let canvas_object = JsObject::with_object_proto(c.intrinsics());
        js(canvas_object.set(JsString::from("width"), 32, true, c))?;
        js(canvas_object.set(JsString::from("height"), 32, true, c))?;
        let f = FunctionObjectBuilder::new(c.realm(), NativeFunction::from_fn_ptr(paint)).build();
        js(canvas_object.set(JsString::from("getContext"), f, true, c))?;
        let document = JsObject::with_object_proto(c.intrinsics());
        let f = FunctionObjectBuilder::new(c.realm(), NativeFunction::from_fn_ptr(canvas)).build();
        js(document.set(JsString::from("getElementById"), f, true, c))?;
        let fills = boa_engine::object::builtins::JsArray::new(c);
        for (name, o) in [
            ("__paint", context),
            ("__canvas", canvas_object),
            ("document", document),
            ("__fills", fills.clone().into()),
        ] {
            js(c.register_global_property(JsString::from(name), o, Attribute::all()))?;
        }
        for color in 0..256 {
            method(
                &self.bitmap,
                "localWrite",
                &[color.into(), 1.into(), color.into()],
                c,
            )?;
        }
        let ctor = object(get(&self.bitmap_namespace, "BitmapOutputView", c)?)?;
        let prototype = object(get(&ctor, "prototype", c)?)?;
        let view = JsObject::with_object_proto(c.intrinsics());
        js(view.set(JsString::from("device"), self.bitmap.clone(), true, c))?;
        js(view.set(JsString::from("id"), JsString::from("canvas"), true, c))?;
        js(object(get(&prototype, "update", c)?)?.call(&view.into(), &[], c))?;
        ensure!(js(fills.length(c))? == 256, "upstream color draw count");
        for color in 0..256u32 {
            let expected = format!(
                "rgb({}, {}, {})",
                (color >> 5) * 255 / 7,
                ((color >> 2) & 7) * 255 / 7,
                (color & 3) * 85
            );
            ensure!(
                text(js(fills.get(color, c))?, c)? == expected,
                "RGB332 color {color}"
            );
        }
        Ok(())
    }
}
impl Rig for Upstream {
    fn run(&mut self, n: usize) -> Result<()> {
        for _ in 0..n {
            self.step()?;
        }
        Ok(())
    }
    fn receive(&mut self, b: u8) -> Result<()> {
        let c = &mut self.context;
        let status = method(&self.input, "localRead", &[0.into(), 1.into()], c)?;
        ensure!(
            js(status.to_i32(c))? & 0x40 == 0,
            "upstream input latch overwrite"
        );
        method(&self.input, "onKeyDown", &[u32::from(b).into()], c)?;
        Ok(())
    }
    fn toggle(&mut self, pin: u32) -> Result<()> {
        ensure!(pin < 32, "invalid pin");
        method(&self.gpio, "toggleInput", &[pin.into()], &mut self.context)?;
        Ok(())
    }
    fn snapshot(&mut self) -> Result<Snapshot> {
        let c = &mut self.context;
        let data = object(get(&self.ram, "data", c)?)?;
        let mut ram = Vec::with_capacity(4096);
        for i in 0..4096u32 {
            ram.push(js(js(data.get(i, c))?.to_i32(c))? as u8);
        }
        let ctrl = method(&self.input, "localRead", &[0.into(), 1.into()], c)?;
        Ok(Snapshot {
            text: self.text.clone(),
            ram,
            gpio_value: number(&self.gpio, "ioStatus", c)?,
            gpio_events: number(&self.gpio, "inputEvents", c)?,
            text_ctrl: js(ctrl.to_i32(c))? as u8,
            irq_entries: self.irqs,
            in_irq: get(&self.cpu, "irqState", c)?.to_boolean(),
            steps: self.steps,
            observed_stack_bytes: 3072 - self.min_sp,
            stack_started: self.stack_started,
            led_levels: self.led_levels,
        })
    }
}
