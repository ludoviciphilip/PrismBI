import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { parseSimpleMarkdown } from '../simpleMarkdown';
import conformance from '../../data/daxConformance.json';

describe('parseSimpleMarkdown (notas de release no UpdateModal)', () => {
  it('reconhece títulos, listas e parágrafos', () => {
    const blocks = parseSimpleMarkdown('# Título\n\n## Seção\nTexto livre\n- item um\n- item dois\n  - sub item');
    expect(blocks.map(b => b.type)).toEqual(['heading', 'heading', 'paragraph', 'list']);
    expect(blocks[0]).toMatchObject({ type: 'heading', level: 1 });
    const list = blocks[3];
    expect(list.type === 'list' && list.items.map(i => i.depth)).toEqual([0, 0, 1]);
  });

  it('converte negrito, código e links http(s) em trechos inline', () => {
    const [p] = parseSimpleMarkdown('Veja **isto**, `codigo` e [docs](https://example.com/x).');
    expect(p.type).toBe('paragraph');
    const spans = p.type === 'paragraph' ? p.spans : [];
    expect(spans).toEqual([
      { kind: 'text', text: 'Veja ' },
      { kind: 'bold', text: 'isto' },
      { kind: 'text', text: ', ' },
      { kind: 'code', text: 'codigo' },
      { kind: 'text', text: ' e ' },
      { kind: 'link', text: 'docs', href: 'https://example.com/x' },
      { kind: 'text', text: '.' },
    ]);
  });

  it('nunca gera link para esquemas que não sejam http(s)', () => {
    const [p] = parseSimpleMarkdown('[x](javascript:alert(1)) e [y](file:///etc/passwd)');
    const spans = p.type === 'paragraph' ? p.spans : [];
    expect(spans.some(s => s.kind === 'link')).toBe(false);
    expect(spans.map(s => s.text).join('')).toContain('x');
  });

  it('trata HTML como texto puro', () => {
    const [p] = parseSimpleMarkdown('<img src=x onerror=alert(1)>');
    expect(p).toEqual({ type: 'paragraph', spans: [{ kind: 'text', text: '<img src=x onerror=alert(1)>' }] });
  });
});

describe('daxConformance.json (número exibido na tela inicial)', () => {
  it('está em sincronia com a baseline do corpus (rode `pnpm dax:report` após mudar a baseline)', () => {
    const baseline = JSON.parse(
      fs.readFileSync(path.resolve(__dirname, '../../../tests/dax-corpus/baseline.json'), 'utf8')
    );
    expect(conformance.engine).toBe('rust');
    expect(conformance.passing).toBe(baseline.rust.length);
    expect(conformance.total).toBeGreaterThanOrEqual(conformance.passing);
  });
});
