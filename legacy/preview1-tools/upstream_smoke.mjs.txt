#!/usr/bin/env node
// Run unmodified upstream CPU/device classes, not a replacement instruction model.
import assert from 'node:assert/strict';
import {readFileSync, readdirSync, writeFileSync} from 'node:fs';
import {resolve, dirname} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {execFileSync} from 'node:child_process';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const upstream = resolve(process.argv[2] || resolve(root, '..', '.emulsiv-upstream'));
const pinned = '9e15421cd33511d4d2911fea1ae41cd65f33dae9';
assert.equal(execFileSync('git', ['-C', upstream, 'rev-parse', 'HEAD'], {encoding:'utf8'}).trim(), pinned, 'unexpected upstream revision');
assert.equal(execFileSync('git', ['-C', upstream, 'status', '--porcelain'], {encoding:'utf8'}).trim(), '', 'upstream checkout must be unmodified');
const load = path => import(pathToFileURL(resolve(upstream, 'src', path)).href);
const {Processor, Bus, Memory} = await load('virgule.js');
const {TextInput, TextOutput} = await load('devices/text.js');
const {GPIO} = await load('devices/gpio.js');
const {BitmapOutput, BitmapOutputView} = await load('devices/bitmap.js');
const hex = await load('hex.js');

function boot(name) {
    const bus = new Bus(), mem = new Memory(0, 4096);
    const input = new TextInput(0xb0000000), output = new TextOutput(0xc0000000, 4);
    const gpio = new GPIO(0xd0000000), bitmap = new BitmapOutput(0xc00, 32, 32);
    for (const device of [mem, input, output, gpio, bitmap]) bus.addDevice(device);
    bus.reset();
    hex.parse(readFileSync(resolve(root, 'dist', `${name}.hex`), 'utf8'), bus);
    assert.equal(bus.error, false, `${name}: HEX load failure`);
    const cpu = new Processor(32, bus);
    const machine = {name, bus, mem, input, output, gpio, bitmap, cpu, text:'', instructions:0, minSp:3072, initialized:false, irqs:0};
    run(machine, 30000);
    return machine;
}

function step(m) {
    const {cpu} = m;
    assert(cpu.pc < 0xc00 && cpu.pc % 4 === 0, `${m.name}: invalid PC`);
    const wasIrq = !!cpu.irqState;
    do {
        const state = cpu.state;
        cpu.step();
        if (state === 'fetch') assert.equal(cpu.fetchError, false, `${m.name}: fetch error`);
        if (state === 'decode') assert.notEqual(cpu.instr.name, 'invalid', `${m.name}: illegal instruction`);
        if (state === 'loadStoreWriteBack') assert.equal(cpu.loadStoreError, false, `${m.name}: MMIO error`);
        if (m.output.hasData()) m.text += m.output.getData();
    } while (cpu.state !== 'fetch');
    if (!wasIrq && cpu.irqState) m.irqs++;
    const sp = cpu.getX(2) >>> 0;
    if (sp === 0xc00) m.initialized = true;
    if (m.initialized) {
        m.minSp = Math.min(m.minSp, sp);
        assert(sp >= 0xa00 && sp <= 0xc00 && sp % 16 === 0, `${m.name}: stack outside reservation or misaligned`);
    }
    m.instructions++;
}
function run(m, count) {
    for (let i = 0; i < count; i++) step(m);
    assert(!m.text.includes('panic'), `${m.name}: Rust panic`);
}
function send(m, text) {
    for (const byte of Buffer.from(text, 'ascii')) {
        assert.equal(m.input.localRead(0, 1) & 0x40, 0, 'input overwrite');
        m.input.onKeyDown(byte);
        let budget = 100000;
        while (m.input.localRead(0, 1) & 0x40) { assert(budget-- > 0, 'input consumption timeout'); step(m); }
        run(m, 10000);
    }
}

const boots = {};
const machines = {};
for (const filename of readdirSync(resolve(root, 'examples')).filter(f=>f.endsWith('.rs')).sort()) {
    const name = filename.slice(0,-3), m = boot(name);
    machines[name] = m;
    boots[name] = {instructions:m.instructions, observed_stack_bytes:3072-m.minSp};
}
const scenarios = [];
assert.equal(machines.hello.text, 'Hello, emulsiV Rust!\n'); scenarios.push('hello');
send(machines.text_echo, 'Az09'); assert(machines.text_echo.text.endsWith('Az09')); scenarios.push('text_echo');
const gm = machines.gpio_mirror;
for (let i=0;i<16;i++) if (0xa55a & (1<<i)) gm.gpio.toggleInput(i+16);
run(gm, 1000); assert.equal(gm.gpio.ioStatus & 0xffff, 0xa55a); scenarios.push('gpio_mirror');
const palette = machines.bitmap_palette;
for (let y=0;y<32;y++) for(let x=0;x<32;x++) assert.equal(palette.mem.data[0xc00+y*32+x], (y>>1)*16+(x>>1));
scenarios.push('all_256_framebuffer_colors');
// Exercise upstream RGB332 canvas conversion with a headless canvas adapter.
// This verifies view protocol, but is NOT a browser DOM/UI end-to-end test.
const draws = [];
const context = {fillStyle:'', fillRect(x,y,w,h) {draws.push({x,y,w,h,color:this.fillStyle});}};
globalThis.document = {getElementById() {return {width:32,height:32,getContext() {return context;}};}};
BitmapOutputView.prototype.update.call({device:palette.bitmap,id:'headless-canvas'});
assert.equal(draws.length, 1024);
for (const d of draws) {
    const c=(d.y>>1)*16+(d.x>>1);
    assert.equal(d.color, `rgb(${Math.floor((c>>5)*255/7)}, ${Math.floor(((c>>2)&7)*255/7)}, ${(c&3)*85})`);
}
delete globalThis.document;
scenarios.push('upstream_rgb332_canvas_conversion');
const ie = machines.irq_echo;
send(ie, 'IRQ'); assert(ie.text.endsWith('IRQ')); assert.equal(ie.irqs,3); assert.equal(ie.cpu.irqState,false); scenarios.push('irq_echo');
const ig = machines.irq_gpio;
ig.gpio.toggleInput(31); run(ig,10000); assert.equal(ig.gpio.ioStatus&1,1); assert.equal(ig.irqs,1); assert.equal(ig.gpio.inputEvents,0); scenarios.push('irq_gpio');
const paint=machines.paint;
send(paint,'4d'); assert.equal(paint.mem.data[0xc00+16*32+17],0xe0); scenarios.push('paint');
const lc=machines.line_console;
send(lc,'Rust;'); assert(lc.text.includes('=Rust\n')); scenarios.push('semicolon_line_console');
const shell=machines.shell;
send(shell,'?;w 0x1234;r;c 224;p 31 31 3;');
assert(shell.text.includes('r | w N | c N | p X Y N'));
assert(shell.text.includes('0x00001234\n'));
assert.equal(shell.gpio.ioStatus&0xffff,0x1234);
for(let i=0;i<1024;i++) assert.equal(shell.mem.data[0xc00+i],i===1023?3:224);
send(shell,'p 32 0 1;w 4294967296;w 2 3;');
assert.equal(shell.gpio.ioStatus&0xffff,0x1234);
assert(shell.text.endsWith('error\nerror\nerror\n'));
send(shell,'w 1'+' '.repeat(30)+';');
assert(shell.text.endsWith('overflow\n')); assert.equal(shell.gpio.ioStatus&0xffff,0x1234);
send(shell,'w 7;r;'); assert(shell.text.endsWith('0x00000007\n'));
scenarios.push('shell_gpio_bitmap_bounds_overflow_recovery');
const report = {
    runner:'unmodified upstream emulsiV JavaScript core and devices under Node',
    upstream_commit:pinned, booted_examples:boots, scenarios,
    shell_observed_stack_bytes:3072-shell.minSp,
    browser_ui_end_to_end:'not performed; bitmap view used a headless canvas adapter',
};
writeFileSync(resolve(root,'dist','upstream-smoke.json'),JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify(report,null,2));
