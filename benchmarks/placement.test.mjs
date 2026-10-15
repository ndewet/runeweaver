import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { generate, summarize } from './placement.mjs';

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'runeweaver-placement-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const assembly = ['count', 'spans', 'values'].map(caller => `
\t.def\t_Rtest_placement_${caller};
\t.scl\t2;
\t.endef
_Rtest_placement_${caller}:
.seh_proc _Rtest_placement_${caller}
\tmovq\t.Ltable(%rip), %rax
\t.p2align\t4
.Lloop_${caller}:
\tjne\t.Lloop_${caller}
\tretq
.seh_endproc
`).join('') + '\n.section .rdata,"dr"\n.Ltable:\n\t.quad\t.Lother\n.Lother:\n\t.byte\t7\n';
  writeFileSync(join(root, 'scan.s'), assembly);
  writeFileSync(join(root, 'input.txt'), '+');
  writeFileSync(join(root, 'oracle.json'), JSON.stringify({ bytes: 524288, count: 1, spans: 1, values: 1 }));
  const config = { input: 'input.txt', revisions: ['base', 'tip'].map(name => ({ name, assembly: 'scan.s', oracle: 'oracle.json' })) };
  writeFileSync(join(root, 'config.json'), JSON.stringify(config));
  return { root, assembly, config };
}

test('extraction preserves branches, internal alignment, and transitive tables', t => {
  const { root } = fixture(t);
  generate(join(root, 'config.json'), join(root, 'out'));
  const assembly = readFileSync(join(root, 'out', 'placement.s'), 'utf8');
  assert.equal((assembly.match(/\.globl /g) ?? []).length, 24);
  assert.equal((assembly.match(/\t\.p2align\t4/g) ?? []).length, 24);
  assert.equal((assembly.match(/\tjne\trw_/g) ?? []).length, 24);
  assert.equal((assembly.match(/\t\.byte\t7/g) ?? []).length, 24);
  assert.doesNotMatch(assembly, /\.seh_|\.Ltable|\.Lother|\.Lloop/);
  const rust = readFileSync(join(root, 'out', 'placement.rs'), 'utf8');
  assert.match(rust, /assert_eq!\(\(scan as usize\) % 64, offset/);
  assert.match(rust, /scan\(input.as_ptr\(\), input.len\(\)\) }, expected/);
});

test('extraction rejects a retained helper', t => {
  const { root, assembly } = fixture(t);
  writeFileSync(join(root, 'scan.s'), assembly.replace('\tretq', '\tcallq\t_Rretained_helper\n\tretq'));
  assert.throws(() => generate(join(root, 'config.json'), join(root, 'out')), /Review the external call/);
});

test('generation rejects differing results', t => {
  const { root, config } = fixture(t);
  writeFileSync(join(root, 'different.json'), JSON.stringify({ bytes: 524288, count: 2, spans: 1, values: 1 }));
  config.revisions[1].oracle = 'different.json';
  writeFileSync(join(root, 'config.json'), JSON.stringify(config));
  assert.throws(() => generate(join(root, 'config.json'), join(root, 'out')), /revision results differ/);
});

function logFixture(t) {
  const { root } = fixture(t);
  const names = [0, 16, 32, 48].map(offset => `rw_base_count_${offset}`);
  const lines = [`manifest,${JSON.stringify({ names, rounds: 2 })}`];
  for (let i = 0; i < names.length; i++) lines.push(`address,${names[i]},${4096 + i * 16}`);
  for (let round = 0; round < 2; round++) for (const name of names) lines.push(`${round},${name},${100 + round * 20}`);
  lines.push('complete');
  const path = join(root, 'results.log');
  writeFileSync(path, lines.join('\n'));
  return { path, lines };
}

test('summary uses the median of complete measurements', t => {
  const { path } = logFixture(t);
  assert.deepEqual(summarize(path).map(row => row.medianNs), [110, 110, 110, 110]);
});

test('summary rejects missing and duplicate samples', t => {
  const { path, lines } = logFixture(t);
  writeFileSync(path, lines.slice(0, -1).join('\n'));
  assert.throws(() => summarize(path), /did not complete/);
  writeFileSync(path, lines.filter((_, index) => index !== lines.length - 2).join('\n'));
  assert.throws(() => summarize(path), /Missing timing samples/);
  writeFileSync(path, [...lines, lines.at(-2)].join('\n'));
  assert.throws(() => summarize(path), /Invalid timing sample/);
});

test('summary rejects incorrect linked addresses', t => {
  const { path, lines } = logFixture(t);
  writeFileSync(path, lines.join('\n').replace('4096', '4097'));
  assert.throws(() => summarize(path), /Invalid linked placement/);
});
