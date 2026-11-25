# Corpus de conformidade DAX

Casos DAX com **resultado esperado conhecido**, executados nos dois motores:

| Motor | Harness | Comando |
| :--- | :--- | :--- |
| TypeScript (legado, InsightHub) | `src/engine/__tests__/dax_corpus.test.ts` | `pnpm vitest run dax_corpus` |
| Rust (principal) | `src-tauri/tests/dax_corpus.rs` | `cargo test --test dax_corpus -- --nocapture` |

Cada execução grava `.report-ts.json` / `.report-rust.json` (ignorados pelo git) e
`node scripts/dax-conformance-report.mjs` junta os dois em `docs/DAX_CONFORMANCE.md`.
**Esse é o número de conformidade que vale**: casos com resultado conferido, não
entradas marcadas como "complete" no catálogo.

## Catraca (`baseline.json`)

`baseline.json` lista, por motor, os ids que já passam. O teste falha se algum
deles **regredir**. Quando um caso novo passar, registre-o:

```bash
UPDATE_DAX_BASELINE=1 pnpm vitest run dax_corpus
(cd src-tauri && UPDATE_DAX_BASELINE=1 cargo test --test dax_corpus)
```

Migrar uma função para Rust = fazer os casos dela passarem em `rust` e
atualizar a baseline no mesmo commit.

## Formato

`models/<nome>.json` — modelo compartilhado:

```jsonc
{
  "tables": [
    { "name": "d_Produto",
      "columns": [{ "name": "ProdutoID", "type": "number" }, ...],   // type: text|number|date|boolean|currency|percentage
      "rows": [[1, "Caneta", ...], ...] },                             // linhas posicionais; null = BLANK
    { "name": "d_Calendario", "calendar": { "start": "2024-01-01", "end": "2025-12-31" },
      "columns": [/* Data, Ano, MesNum, Trimestre, AnoMes — gerados */] }
  ],
  "relationships": [{ "from": "f_Vendas[ProdutoID]", "to": "d_Produto[ProdutoID]", "crossFilter": "single", "active": true }],
  "measures": [{ "table": "f_Vendas", "name": "Total Vendas", "formula": "SUM(f_Vendas[Valor])" }]
}
```

Datas são strings ISO `YYYY-MM-DD`. O calendário gerado é idêntico nos dois harnesses.

`cases/<categoria>.json`:

```jsonc
{
  "model": "vendas",
  "category": "core",
  "cases": [
    { "id": "core.sum.all",                 // único em todo o corpus: <categoria>.<função>.<cenário>
      "expr": "SUM(f_Vendas[Valor])",
      "filters": [{ "table": "d_Produto", "column": "Categoria", "values": ["Papelaria"] }], // opcional (filtros de visual/slicer)
      "expected": 1135,                     // número | texto | true/false | null (=BLANK) | {"date":"2024-01-31"} | {"error": true}
      "tolerance": 1e-9,                    // opcional, relativa
      "source": "dax.guide/SUM",            // opcional: de onde veio o valor esperado
      "notes": "..." }
  ]
}
```

## Regras para valores esperados

- O esperado precisa ser **independente dos motores**: calcule com um oráculo
  separado (Python/planilha), copie de um exemplo documentado (dax.guide,
  Microsoft Learn) ou, idealmente, do Power BI Desktop real. Nunca copie a saída
  de um dos motores.
- Prefira modelos pequenos o bastante para conferir à mão.
- Um caso = um comportamento. Casos de borda (BLANK, divisão por zero, texto vs.
  número, datas no limite do mês) valem mais que casos felizes repetidos.
