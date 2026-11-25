/**
 * Parser mínimo de Markdown para as notas de release no UpdateModal.
 * Só cobre o que as notas usam (títulos, listas, **negrito**, `código`,
 * [links](https://...)). Gera uma estrutura de dados renderizada com
 * elementos React, nunca HTML cru: o texto vem da API do GitHub.
 */
export type MdSpan =
  | { kind: 'text'; text: string }
  | { kind: 'bold'; text: string }
  | { kind: 'code'; text: string }
  | { kind: 'link'; text: string; href: string };

export type MdBlock =
  | { type: 'heading'; level: 1 | 2 | 3; spans: MdSpan[] }
  | { type: 'paragraph'; spans: MdSpan[] }
  | { type: 'list'; items: { depth: number; spans: MdSpan[] }[] };

const INLINE = /\*\*(.+?)\*\*|`([^`]+)`|\[([^\]]+)\]\(([^)\s]+)\)/g;

export function parseInline(text: string): MdSpan[] {
  const spans: MdSpan[] = [];
  let last = 0;
  const push = (s: MdSpan) => {
    const prev = spans[spans.length - 1];
    if (s.kind === 'text' && prev?.kind === 'text') prev.text += s.text;
    else if (s.text) spans.push(s);
  };
  for (const m of text.matchAll(INLINE)) {
    const idx = m.index ?? 0;
    if (idx > last) push({ kind: 'text', text: text.slice(last, idx) });
    if (m[1] !== undefined) push({ kind: 'bold', text: m[1] });
    else if (m[2] !== undefined) push({ kind: 'code', text: m[2] });
    else if (/^https?:\/\//i.test(m[4])) push({ kind: 'link', text: m[3], href: m[4] });
    else push({ kind: 'text', text: m[3] });
    last = idx + m[0].length;
  }
  if (last < text.length) push({ kind: 'text', text: text.slice(last) });
  return spans;
}

export function parseSimpleMarkdown(src: string): MdBlock[] {
  const blocks: MdBlock[] = [];
  let para: string[] = [];
  const flushPara = () => {
    if (para.length) blocks.push({ type: 'paragraph', spans: parseInline(para.join(' ')) });
    para = [];
  };
  for (const raw of src.replace(/\r\n?/g, '\n').split('\n')) {
    const line = raw.replace(/\s+$/, '');
    if (!line.trim()) {
      flushPara();
      continue;
    }
    const heading = /^(#{1,3})\s+(.*)$/.exec(line);
    if (heading) {
      flushPara();
      blocks.push({ type: 'heading', level: heading[1].length as 1 | 2 | 3, spans: parseInline(heading[2]) });
      continue;
    }
    const item = /^(\s*)[-*]\s+(.*)$/.exec(line);
    if (item) {
      flushPara();
      const depth = Math.min(3, Math.floor(item[1].replace(/\t/g, '  ').length / 2));
      const prev = blocks[blocks.length - 1];
      const entry = { depth, spans: parseInline(item[2]) };
      if (prev?.type === 'list') prev.items.push(entry);
      else blocks.push({ type: 'list', items: [entry] });
      continue;
    }
    para.push(line.trim());
  }
  flushPara();
  return blocks;
}
