import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';

const tokens = /[A-Za-z_.$][A-Za-z0-9_.$]*/g;
const callers = ['count', 'spans', 'values'];
const hash = text => createHash('sha256').update(text).digest('hex');

function extract(source, caller) {
  const blocks = source.split(/(?=^\t\.def\t)/m);
  const matches = blocks.filter(block => new RegExp(`^\\t\\.def\\t_R[^;]*placement_${caller};`, 'm').test(block));
  if (matches.length !== 1) throw Error(`Expected one placement_${caller} function`);
  const symbol = matches[0].match(/^\t\.def\t([^;]+);/m)[1];
  let body = matches[0].slice(matches[0].indexOf(`${symbol}:`));
  body = body.split('.seh_handlerdata')[0].split('.seh_endproc')[0];
  body = body.split('\n').filter(line => !/^\s*\.(?:seh_|section|def|scl|type|endef)/.test(line)).join('\n');
  if (/^\s*callq?\s+\*/m.test(body)) throw Error('Review the indirect call before extraction');
  const locals = new Set([...body.matchAll(/^([\w.$]+):/gm)].map(match => match[1]));
  const definitions = new Map();
  const lines = source.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const match = lines[i].match(/^([\w.$]+):/);
    if (!match) continue;
    const data = [];
    for (let j = i + 1; j < lines.length; j++) {
      if (/^\s*\.section\b/.test(lines[j]) || /^[\w.$]+:/.test(lines[j])) break;
      if (/^\s*\.(?:ascii|asciz|byte|short|long|quad|zero|fill)\b/.test(lines[j])) data.push(lines[j]);
    }
    if (data.length) definitions.set(match[1], data.join('\n'));
  }
  const tables = new Map();
  function visit(text) {
    for (const token of text.match(tokens) ?? []) {
      if (!locals.has(token) && definitions.has(token) && !tables.has(token)) {
        tables.set(token, definitions.get(token));
        visit(definitions.get(token));
      }
    }
  }
  visit(body);
  const external = new Set();
  for (const text of [body, ...tables.values()]) {
    for (const token of text.match(tokens) ?? []) {
      if (token.startsWith('_R') && !locals.has(token) && !tables.has(token)) external.add(token);
    }
  }
  // A valid punctuation scan must not call a retained helper.
  for (const line of body.split('\n').filter(line => /^\s*(?:call|jmp)q?\s+_R/.test(line))) {
    if (!/panic|unwrap_failed|slice_error|slice_index_fail|unrecognized/.test(line)) {
      throw Error(`Review the external call before extraction: ${line.trim()}`);
    }
  }
  return { symbol, body, locals, tables, external };
}

export function generate(configPath, directory) {
  const configRoot = dirname(resolve(configPath));
  const config = JSON.parse(readFileSync(configPath, 'utf8'));
  const out = resolve(directory);
  mkdirSync(out, { recursive: true });
  const input = readFileSync(resolve(configRoot, config.input), 'utf8');
  if (!input.length) throw Error('The input is empty');
  writeFileSync(resolve(out, 'input.txt'), input);
  const names = new Set();
  const variants = [];
  const audit = [];
  let referenceOracle;
  let assembly = '';
  for (const revision of config.revisions) {
    if (!/^[a-z][a-z0-9_]*$/.test(revision.name) || names.has(revision.name)) throw Error('Use unique revision identifiers');
    names.add(revision.name);
    const source = readFileSync(resolve(configRoot, revision.assembly), 'utf8');
    const oracle = JSON.parse(readFileSync(resolve(configRoot, revision.oracle), 'utf8'));
    const bytes = Buffer.byteLength(input.repeat(Math.ceil(512 * 1024 / Buffer.byteLength(input))));
    if (oracle.bytes !== bytes) throw Error('The oracle input size differs');
    referenceOracle ??= oracle;
    if (callers.some(caller => oracle[caller] !== referenceOracle[caller])) throw Error('The revision results differ');
    for (const caller of callers) {
      if (!Number.isSafeInteger(oracle[caller]) || oracle[caller] <= 0) throw Error('Missing oracle result');
      const scan = extract(source, caller);
      audit.push({ revision: revision.name, caller, assemblyHash: hash(source), body: scan.body,
        tables: Object.fromEntries(scan.tables), abortedSymbols: [...scan.external] });
      for (const offset of [0, 16, 32, 48]) {
        const name = `rw_${revision.name}_${caller}_${offset}`;
        const symbols = new Map([[scan.symbol, name]]);
        let id = 0;
        for (const label of scan.locals) if (label !== scan.symbol) symbols.set(label, `${name}_L${id++}`);
        for (const label of scan.tables.keys()) symbols.set(label, `${name}_D${id++}`);
        const rename = text => text.replace(tokens, token => symbols.get(token) ?? (scan.external.has(token) ? 'rw_abort' : token));
        assembly += `\n.text\n.p2align 6, 0x90\n.space ${offset}, 0x90\n.globl ${name}\n${rename(scan.body)}\n`;
        assembly += '\n.section .rdata,"dr"\n';
        for (const [label, data] of scan.tables) assembly += `.p2align 6\n${symbols.get(label)}:\n${rename(data)}\n`;
        variants.push({ name, offset, expected: oracle[caller] });
      }
    }
  }
  if (variants.length < 24) throw Error('Compare at least two revisions, including an unchanged control');
  writeFileSync(resolve(out, 'placement.s'), assembly);
  writeFileSync(resolve(out, 'audit.json'), JSON.stringify({ inputHash: hash(input), functions: audit }, null, 2));
  const declarations = variants.map(v => `fn ${v.name}(input: *const u8, len: usize) -> usize;`).join('\n');
  const entries = variants.map(v => `("${v.name}", ${v.name} as Scan, ${v.offset}usize, ${v.expected}usize)`).join(',\n');
  writeFileSync(resolve(out, 'placement.rs'), `#[cfg(not(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc")))]
compile_error!("The placement experiment requires Windows x64 MSVC");
use std::{hint::black_box, time::Instant};
core::arch::global_asm!(include_str!("placement.s"), options(att_syntax));
type Scan = unsafe extern "C" fn(*const u8, usize) -> usize;
unsafe extern "C" { ${declarations} }
#[unsafe(no_mangle)] pub extern "C" fn rw_abort() -> ! { std::process::abort() }
fn main() {
    let source = include_str!("input.txt");
    let input = source.repeat((512 * 1024usize).div_ceil(source.len()));
    let variants = [${entries}];
    let iterations: usize = std::env::args().nth(1).unwrap_or("200".into()).parse().unwrap();
    let reverse = std::env::args().nth(2).as_deref() == Some("reverse");
    assert!(iterations > 0);
    println!("manifest,{}", r#"${JSON.stringify({ names: variants.map(v => v.name), rounds: 8, inputHash: hash(input) })}"#);
    println!("iterations,{iterations}");
    println!("reverse,{reverse}");
    for &(name, scan, offset, expected) in &variants {
        assert_eq!((scan as usize) % 64, offset, "{name}");
        assert_eq!(unsafe { scan(input.as_ptr(), input.len()) }, expected, "{name}");
        println!("address,{name},{:#x}", scan as usize);
    }
    println!("round,variant,ns");
    for round in 0..8 {
        for j in 0..variants.len() {
            let index = if (round % 2 == 0) != reverse { j } else { variants.len() - 1 - j };
            let (name, scan, _, _) = variants[(index + (round / 2) * 17) % variants.len()];
            for _ in 0..10 { black_box(unsafe { scan(black_box(input.as_ptr()), input.len()) }); }
            let start = Instant::now();
            for _ in 0..iterations { black_box(unsafe { scan(black_box(input.as_ptr()), input.len()) }); }
            println!("{round},{name},{}", start.elapsed().as_nanos() as f64 / iterations as f64);
        }
    }
    println!("complete");
}
`);
  console.log(`Generated ${variants.length} placements. Review audit.json before compilation.`);
}

export function summarize(path) {
  const groups = new Map();
  const lines = readFileSync(path, 'utf8').split(/\r?\n/);
  const manifests = lines.filter(line => line.startsWith('manifest,'));
  if (manifests.length !== 1) throw Error('Expected one measurement manifest');
  const manifest = JSON.parse(manifests[0].slice('manifest,'.length));
  if (lines.filter(line => line === 'complete').length !== 1) throw Error('The measurement did not complete');
  const expected = new Set(manifest.names);
  const seen = new Set();
  const addresses = new Map();
  for (const line of lines.filter(line => line.startsWith('address,'))) {
    const [, name, value] = line.split(',');
    if (!expected.has(name) || addresses.has(name) || BigInt(value) % 64n !== BigInt(name.match(/_(\d+)$/)[1])) {
      throw Error('Invalid linked placement');
    }
    addresses.set(name, value);
  }
  if (addresses.size !== expected.size) throw Error('Missing linked addresses');
  for (const line of lines) {
    const match = line.match(/^(\d+),(rw_\w+),(\d+(?:\.\d+)?)$/);
    if (!match) continue;
    const [, round, name, value] = match;
    if (!expected.has(name) || Number(round) >= manifest.rounds || seen.has(`${round},${name}`) || Number(value) <= 0) {
      throw Error('Invalid timing sample');
    }
    seen.add(`${round},${name}`);
    if (!groups.has(name)) groups.set(name, []);
    groups.get(name).push(Number(value));
  }
  if (seen.size !== expected.size * manifest.rounds) throw Error('Missing timing samples');
  const median = values => { const x = [...values].sort((a, b) => a - b); return (x[Math.floor((x.length - 1) / 2)] + x[Math.floor(x.length / 2)]) / 2; };
  const rows = [...groups].map(([name, samples]) => ({ name, samples: samples.length, medianNs: median(samples) }));
  for (const row of rows) console.log(`${row.name},${row.samples},${row.medianNs.toFixed(2)}`);
  console.log('revision_caller,best_ns,median_ns,worst_ns,span_percent');
  const families = new Map();
  for (const row of rows) {
    const family = row.name.replace(/_\d+$/, '');
    if (!families.has(family)) families.set(family, []);
    families.get(family).push(row.medianNs);
  }
  for (const [family, values] of families) {
    if (values.length !== 4) throw Error(`Missing placements for ${family}`);
    const best = Math.min(...values), worst = Math.max(...values);
    console.log(`${family},${best.toFixed(2)},${median(values).toFixed(2)},${worst.toFixed(2)},${(100 * (worst / best - 1)).toFixed(2)}`);
  }
  return rows;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [command, ...args] = process.argv.slice(2);
  if (command === 'generate' && args.length === 2) generate(...args);
  else if (command === 'summarize' && args.length === 1) summarize(...args);
  else throw Error('Use: node benchmarks/placement.mjs generate CONFIG OUT | summarize LOG');
}
