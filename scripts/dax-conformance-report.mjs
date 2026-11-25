#!/usr/bin/env node
// Junta tests/dax-corpus/.report-{ts,rust}.json em docs/DAX_CONFORMANCE.md.
// Uso: rode os dois harnesses e depois `node scripts/dax-conformance-report.mjs`.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const corpus = path.join(root, 'tests/dax-corpus');

const load = engine => {
  const p = path.join(corpus, `.report-${engine}.json`);
  if (!fs.existsSync(p)) throw new Error(`Relatório ausente: ${p} (rode o harness do motor ${engine})`);
  return JSON.parse(fs.readFileSync(p, 'utf8'));
};

const ts = load('ts');
const rust = load('rust');
const byId = engine => new Map(engine.results.map(r => [r.id, r]));
const tsMap = byId(ts);
const rustMap = byId(rust);
const ids = [...new Set([...tsMap.keys(), ...rustMap.keys()])].sort();

const cats = new Map();
for (const id of ids) {
  const cat = (tsMap.get(id) ?? rustMap.get(id)).category;
  const c = cats.get(cat) ?? { total: 0, ts: 0, rust: 0 };
  c.total += 1;
  if (tsMap.get(id)?.pass) c.ts += 1;
  if (rustMap.get(id)?.pass) c.rust += 1;
  cats.set(cat, c);
}

const pct = (a, b) => (b ? `${((100 * a) / b).toFixed(1)}%` : '—');
const fmt = v => (v === undefined ? '—' : JSON.stringify(v));
const lines = [];
lines.push('# Conformidade DAX medida (corpus)');
lines.push('');
lines.push('> Gerado por `scripts/dax-conformance-report.mjs` a partir de `tests/dax-corpus/`. Não edite à mão.');
lines.push('> Cada caso tem resultado esperado conferido de forma independente dos motores.');
lines.push('');
lines.push(`**Casos:** ${ids.length} · **Rust (principal):** ${rust.passing}/${rust.total} (${pct(rust.passing, rust.total)}) · **TypeScript (legado):** ${ts.passing}/${ts.total} (${pct(ts.passing, ts.total)})`);
lines.push('');
lines.push('| Categoria | Casos | Rust | TS |');
lines.push('| :--- | ---: | ---: | ---: |');
for (const [cat, c] of [...cats.entries()].sort()) {
  lines.push(`| ${cat} | ${c.total} | ${c.rust} (${pct(c.rust, c.total)}) | ${c.ts} (${pct(c.ts, c.total)}) |`);
}
lines.push('');
const failing = ids.filter(id => !rustMap.get(id)?.pass || !tsMap.get(id)?.pass);
lines.push(`## Casos com falha (${failing.length})`);
lines.push('');
if (failing.length) {
  lines.push('| Caso | Rust | TS |');
  lines.push('| :--- | :--- | :--- |');
  for (const id of failing) {
    const cell = r => (!r ? '—' : r.pass ? '✅' : `❌ ${r.error ? `erro: ${String(r.error).slice(0, 60)}` : fmt(r.actual)}`);
    lines.push(`| \`${id}\` | ${cell(rustMap.get(id))} | ${cell(tsMap.get(id))} |`);
  }
} else {
  lines.push('Nenhuma. 🎉');
}
lines.push('');

const out = path.join(root, 'docs/DAX_CONFORMANCE.md');
fs.writeFileSync(out, lines.join('\n'));
console.log(`Escrito ${path.relative(root, out)} — Rust ${rust.passing}/${rust.total}, TS ${ts.passing}/${ts.total}`);
