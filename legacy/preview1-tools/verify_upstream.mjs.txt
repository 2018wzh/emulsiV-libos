#!/usr/bin/env node
// Run with Node 22.7+: node tools/verify_upstream.mjs
// This exercises unmodified upstream core/device classes, NOT the browser UI.
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync} from 'node:fs';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {resolve, dirname} from 'node:path';
import {execFileSync} from 'node:child_process';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const upstream = resolve(process.argv[2] || resolve(root, 'target/upstream-emulsiv'));
const PIN = '9e15421cd33511d4d2911fea1ae41cd65f33dae9';
const commit = execFileSync('git', ['-C', upstream, 'rev-parse', 'HEAD'], {encoding:'utf8'}).trim();
assert.equal(commit, PIN, 'Unexpected upstream revision; review before changing the pin');
assert.equal(execFileSync('git', ['-C', upstream, 'status', '--porcelain'], {encoding:'utf8'}).trim(), '', 'Upstream must be unmodified');
const load = p => import(pathToFileURL(resolve(upstream, p)).href);
const {Processor, Bus, Memory} = await load('src/virgule.js');
const {TextInput, TextOutput} = await load('src/devices/text.js');
const {GPIO} = await load('src/devices/gpio.js');
const {BitmapOutput, BitmapOutputView} = await load('src/devices/bitmap.js');
const {parse} = await load('src/hex.js');
const builds = JSON.parse(readFileSync(resolve(root, 'dist/build-report.json'), 'utf8'));

class Rig {
    constructor(name) {
        this.name = name;
        this.bus = new Bus();
        this.ram = new Memory(0, 4096);
        this.input = new TextInput(0xb0000000);
        this.output = new TextOutput(0xc0000000, 1);
        this.gpio = new GPIO(0xd0000000);
        this.bitmap = new BitmapOutput(0xc00, 32, 32);
        for (const d of [this.ram, this.input, this.output, this.gpio, this.bitmap]) this.bus.addDevice(d);
        this.bus.reset();
        parse(readFileSync(resolve(root, 'dist', `${name}.hex`), 'utf8'), this.bus);
        this.cpu = new Processor(32, this.bus);
        this.text = ''; this.steps = 0; this.irqs = 0;
        this.minSp = 0xc00; this.stackStarted = false;
        this.pixels = new Uint8Array(1024); this.pixelWrites = 0;
        this.ledLevels = new Set();
    }
    step() {
        const cpu = this.cpu;
        do {
            const state = cpu.state;
            cpu.step();
            assert(!cpu.fetchError && !cpu.loadStoreError && !this.bus.error, `${this.name}: bus fault at ${cpu.pc.toString(16)}`);
            if (state === 'decode') assert.notEqual(cpu.instr.name, 'invalid', `${this.name}: invalid opcode`);
            const sp = cpu.getX(2) >>> 0;
            // `la sp, __stack_top` first writes a temporary AUIPC value.
            // Start checking only once its ADDI has established the real top.
            if (!this.stackStarted && sp === 0xc00) this.stackStarted = true;
            if (this.stackStarted) {
                assert(sp >= 0xa00 && sp <= 0xc00, `${this.name}: stack escaped reservation: ${sp.toString(16)}`);
                this.minSp = Math.min(this.minSp, sp);
            }
            if (this.output.hasData()) this.text += this.output.getData();
            for (const p of this.bitmap.getData()) {
                this.pixels[p.y * 32 + p.x] = p.c; this.pixelWrites++;
            }
        } while (cpu.state !== 'fetch');
        if (cpu.acceptingIrq) this.irqs++;
        this.ledLevels.add(this.gpio.ioStatus & 1);
        this.steps++;
    }
    run(n = 100000) { for (let i=0; i<n; i++) this.step(); return this; }
    send(text) {
        for (const ch of text) {
            assert.equal(this.input.localRead(0, 1) & 0x40, 0);
            this.input.onKeyDown(ch.charCodeAt(0));
            let budget = 20000;
            while (this.input.localRead(0, 1) & 0x40) { assert(budget-- > 0, `${this.name}: input not consumed`); this.step(); }
            this.run(2000);
        }
        return this;
    }
    pixel(x, y) { return this.ram.localRead(0xc00 + y*32 + x, 1); }
}
const results = {};
function check(name, fn) {
    assert(builds[name] && !builds[name].error, `Missing audited build: ${name}`);
    const r = new Rig(name).run(); fn(r);
    assert(!r.text.includes('panic'), `${name}: panic output`);
    assert(r.stackStarted);
    // Writes to overlapping RAM and Bitmap device must agree.
    if (r.pixelWrites) assert.deepEqual(r.ram.data.slice(0xc00), r.pixels);
    results[name] = {instructions:r.steps, irq_entries:r.irqs, observed_stack_bytes:0xc00-r.minSp, pixel_writes:r.pixelWrites};
    console.log(`PASS ${name}: stack observed ${0xc00-r.minSp}/512 bytes`);
}
check('hello', r => assert.equal(r.text, 'Hello, emulsiV Rust!\n'));
check('text_echo', r => {r.send('Hello'); assert(r.text.endsWith('Hello'));});
check('line_console', r => {r.send('Rust;'); assert(r.text.includes('\n=Rust\n'));});
check('gpio_mirror', r => {r.gpio.toggleInput(16); r.gpio.toggleInput(31); r.run(1000); assert.equal(r.gpio.ioStatus & 0xffff, 0x8001);});
check('gpio_debounce', r => {
    r.gpio.toggleInput(31); r.run(5000); assert.equal(r.gpio.ioStatus & 1, 1);
    r.gpio.toggleInput(31); r.run(5000); r.gpio.toggleInput(31); r.run(5000); assert.equal(r.gpio.ioStatus & 1, 0);
});
check('gpio_pwm', r => assert.equal(r.ledLevels.size, 2));
check('bitmap_palette', r => {
    for (let y=0; y<32; y++) for (let x=0; x<32; x++) assert.equal(r.pixel(x,y), (y>>1)*16+(x>>1));
    assert.equal(new Set(r.pixels).size, 256);
});
check('bitmap_shapes', r => {assert.equal(r.pixel(0,0),3); assert.equal(r.pixel(4,4),0x1c); assert.equal(r.pixel(16,7),0xfc);});
check('bitmap_text', r => {assert(r.pixels.filter(v=>v===0x1f).length>50); assert(r.pixels.every(v=>v===0 || v===0x1f));});
check('mono_sprite', r => {assert.equal(r.pixel(0,0),3); assert.equal(r.pixel(14,12),0xfc);});
check('cooperative', r => {assert(r.text.includes('.')); assert.equal(r.ledLevels.size,2);});
check('event_loop', r => {r.send('A '); assert.equal(r.text, 'A '); assert.equal(r.gpio.ioStatus&1,1);});
check('irq_echo', r => {
    r.send('Z'.repeat(100)); assert.equal(r.irqs,100); assert.equal(r.text,'IRQ echo:\n'+'Z'.repeat(100)); assert(!r.cpu.irqState);
});
check('irq_gpio', r => {
    for (let i=0; i<100; i++) {r.gpio.toggleInput(31); r.run(500);}
    assert.equal(r.irqs,100); assert.equal(r.gpio.ioStatus&1,0); assert.equal(r.gpio.inputEvents,0); assert(!r.cpu.irqState);
});
check('crc_demo', r => assert.equal(r.text,'CRC32=0xcbf43926\nCRC16=0x000029b1\n'));
check('arena_demo', r => assert.equal(r.text,'arena used=16\n'));
check('shell', r => {
    r.send('?;w 0x55aa;r;'); assert(r.text.includes('0x000055aa')); assert.equal(r.gpio.ioStatus&0xffff,0x55aa);
    r.send('c 255;p 31 31 3;'); assert.equal(r.pixel(0,0),255); assert.equal(r.pixel(31,31),3);
    r.send('c 0;p 16 16 0xe0;'); assert.equal(r.pixel(16,16),0xe0); assert.equal(r.pixel(0,0),0);
    r.send('w 4294967296;p 32 0 1;'); assert(r.text.includes('error\n')); assert.equal(r.gpio.ioStatus&0xffff,0x55aa);
    r.send('w 0'+' '.repeat(40)+'1;'); assert(r.text.includes('overflow\n')); assert.equal(r.gpio.ioStatus&0xffff,0x55aa);
});
check('random_pixels', r => assert(r.pixels.filter(v=>v!==0).length>5));
check('diagnostics', r => {assert(r.text.includes('stack reserved=512\n')); assert(r.text.startsWith('image end=0x'));});
check('paint', r => {r.send('4d'); assert.equal(r.pixel(17,16),0xe0); r.send('2s'); assert.equal(r.pixel(17,17),0x1c);});
assert.deepEqual(Object.keys(results).sort(), Object.keys(builds).sort(), 'Every build must have an upstream test');

// Execute the real rendering method against a recording canvas adapter. No browser is launched.
const fills = [];
const context = {fillStyle:'', fillRect(){fills.push(this.fillStyle);}};
const previousDocument = globalThis.document;
globalThis.document = {getElementById(){return {width:32,height:32,getContext(){return context;}};}};
try {
    const bitmap = new BitmapOutput(0xc00,32,32);
    [0xe0,0x1c,3,255].forEach((c,i)=>bitmap.localWrite(i,1,c));
    BitmapOutputView.prototype.update.call({id:'test',device:bitmap});
    assert.deepEqual(fills,['rgb(255, 0, 0)','rgb(0, 255, 0)','rgb(0, 0, 255)','rgb(255, 255, 255)']);
} finally {globalThis.document = previousDocument;}
const report = {upstream_commit:commit, mode:'unmodified upstream CPU/devices under Node; recording canvas adapter', browser_ui_tested:false, native_rgb332_rendering:true, stack_note:'Observed test paths only, not a proof of worst-case stack usage', firmware:results};
writeFileSync(resolve(root,'dist/upstream-verification.json'),JSON.stringify(report,null,2)+'\n');
console.log(`PASS ${Object.keys(results).length} upstream firmware scenarios and RGB332 rendering`);
