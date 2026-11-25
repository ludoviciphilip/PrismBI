import React from 'react';
import { parseSimpleMarkdown, MdSpan } from '../../services/simpleMarkdown';

const Spans: React.FC<{ spans: MdSpan[] }> = ({ spans }) => (
  <>
    {spans.map((s, i) => {
      switch (s.kind) {
        case 'bold':
          return <strong key={i} className="font-semibold text-slate-800 dark:text-slate-100">{s.text}</strong>;
        case 'code':
          return <code key={i} className="font-mono text-[10px] px-1 rounded bg-slate-100 dark:bg-slate-800">{s.text}</code>;
        case 'link':
          return <a key={i} href={s.href} target="_blank" rel="noreferrer" className="text-amber-600 dark:text-amber-400 underline">{s.text}</a>;
        default:
          return <React.Fragment key={i}>{s.text}</React.Fragment>;
      }
    })}
  </>
);

/** Renderiza Markdown simples (notas de release) sem HTML cru. */
export const SimpleMarkdown: React.FC<{ source: string; className?: string }> = ({ source, className }) => (
  <div className={className}>
    {parseSimpleMarkdown(source).map((b, i) => {
      if (b.type === 'heading') {
        const cls = b.level === 1 ? 'text-[13px] font-bold mt-1' : b.level === 2 ? 'text-xs font-bold mt-2' : 'text-[11px] font-semibold mt-2';
        return <div key={i} className={`${cls} text-slate-900 dark:text-slate-100`}><Spans spans={b.spans} /></div>;
      }
      if (b.type === 'list') {
        return (
          <ul key={i} className="my-1 space-y-0.5">
            {b.items.map((it, j) => (
              <li key={j} className="flex gap-1.5" style={{ paddingLeft: it.depth * 12 }}>
                <span className="text-amber-500 shrink-0">•</span>
                <span><Spans spans={it.spans} /></span>
              </li>
            ))}
          </ul>
        );
      }
      return <p key={i} className="my-1"><Spans spans={b.spans} /></p>;
    })}
  </div>
);
