# v0.9.0: Power Query visual e round-trip de M (contrato de design)

> Documento de contrato entre o motor ETL (Rust) e o editor visual (frontend).
> Mudanças no schema devem atualizar este arquivo e os dois lados no mesmo PR.

## Problema atual

- Os passos são aplicados de forma **destrutiva** no TS (`EtlEngine.executePipeline` sobre as linhas atuais da tabela). Remover ou reordenar um passo não desfaz nada, e "Atualizar Dados" perde as transformações.
- O M de `.pbip` é interpretado com buscas de texto (`extract_quoted_arg_after`…) e só cobre Rename, Remove, Select e um subconjunto de SelectRows. Os outros passos são descartados sem aviso.

## Modelo novo (igual ao Power Query)

```
fonte bruta  ──►  DuckDB: tabela  raw__<tabela>   (nunca alterada por passos)
                 │
EtlQuery { source, steps[] }  ──►  SQL (cadeia de CTEs, 1 por passo)  ──►  tabela do modelo
                 │                                                         (CREATE OR REPLACE)
                 └─►  preview: o mesmo SQL cortado no passo i + LIMIT
M  ◄──────────────►  EtlQuery   (etl_to_m / etl_from_m; round-trip)
```

- Cada consulta é **reexecutada desde a fonte bruta**. Editar, remover ou reordenar um passo só muda a lista.
- Toda a execução fica no **Rust/DuckDB**. O motor TS de ETL vira legado, usado só no navegador sem Tauri.

## Schema (`src-tauri/src/etl/model.rs` ⇄ `src/types/etl.ts`)

JSON com `serde(tag = "op", rename_all = "camelCase")`; campos em camelCase.

```ts
type EtlType = 'text' | 'integer' | 'decimal' | 'currency' | 'date' | 'datetime' | 'boolean';
type Scalar = string | number | boolean | null;

type Cond =
  | { column: string; op: 'eq'|'neq'|'gt'|'gte'|'lt'|'lte'|'contains'|'notContains'|'startsWith'|'endsWith'; value: Scalar }
  | { column: string; op: 'isNull'|'isNotNull'|'isBlank'|'isNotBlank' }
  | { column: string; op: 'in'|'notIn'; values: Scalar[] }
  | { and: Cond[] } | { or: Cond[] } | { not: Cond };

type Agg = { name: string; op: 'sum'|'avg'|'min'|'max'|'count'|'countDistinct'|'countRows'|'median'; column?: string };

type EtlStep = { id: string; name: string } & (            // name = nome do passo no M (#"Changed Type")
  | { op: 'promoteHeaders' }
  | { op: 'changeType'; columns: { column: string; type: EtlType }[]; culture?: string }
  | { op: 'renameColumns'; renames: [string, string][] }
  | { op: 'removeColumns'; columns: string[] }
  | { op: 'selectColumns'; columns: string[] }                       // "Remover outras colunas"
  | { op: 'duplicateColumn'; column: string; newColumn: string }
  | { op: 'filterRows'; condition: Cond }
  | { op: 'removeBlankRows' }
  | { op: 'removeRowsWithErrors'; columns?: string[] }
  | { op: 'keepTopRows' | 'removeTopRows' | 'keepBottomRows' | 'removeBottomRows'; count: number }
  | { op: 'removeDuplicates'; columns?: string[] }
  | { op: 'sort'; by: { column: string; descending: boolean }[] }
  | { op: 'replaceValues'; columns: string[]; find: Scalar; replace: Scalar; matchEntireCell: boolean }
  | { op: 'fillDown' | 'fillUp'; columns: string[] }
  | { op: 'splitColumn'; column: string; mode: 'delimiterEach'|'delimiterLeftmost'|'delimiterRightmost'|'positions'|'fixedLength';
      delimiter?: string; positions?: number[]; length?: number; into: string[] }
  | { op: 'mergeColumns'; columns: string[]; separator: string; newColumn: string }
  | { op: 'textTransform'; columns: string[]; fn: 'upper'|'lower'|'proper'|'trim'|'clean' }
  | { op: 'unpivotOtherColumns'; keep: string[]; attributeColumn: string; valueColumn: string }
  | { op: 'unpivotColumns'; columns: string[]; attributeColumn: string; valueColumn: string }
  | { op: 'pivot'; column: string; valuesColumn: string; aggregation: Agg['op'] }
  | { op: 'groupBy'; keys: string[]; aggregations: Agg[] }
  | { op: 'addConditionalColumn'; newColumn: string; rules: { when: Cond; then: Scalar | { column: string } }[];
      otherwise: Scalar | { column: string }; type?: EtlType }
  | { op: 'addCustomColumn'; newColumn: string; expression: string; type?: EtlType }   // subconjunto de M, ver abaixo
  | { op: 'addIndexColumn'; newColumn: string; start: number; step: number }
  | { op: 'mergeQueries'; rightQuery: string; leftKeys: string[]; rightKeys: string[];
      joinKind: 'leftOuter'|'inner'|'rightOuter'|'fullOuter'|'leftAnti'|'rightAnti';
      expand: { column: string; as: string }[] }                     // NestedJoin + ExpandTableColumn num passo só
  | { op: 'appendQueries'; queries: string[] }
  | { op: 'opaqueM'; m: string }   // passo M não suportado: preservado no round-trip, não executável
);

interface EtlSource {                 // de onde vem raw__<tabela>
  kind: 'excel' | 'csv' | 'parquet' | 'json' | 'db' | 'table' | 'opaqueM';
  path?: string; sheet?: string;      // excel/csv/parquet/json (sheet = Item do Excel.Workbook)
  table?: string;                     // 'table': outra consulta do modelo (referência)
  m?: string;                         // 'opaqueM': fonte M não reconhecida (ex.: calendário via função)
}

interface EtlQuery { name: string; source: EtlSource; steps: EtlStep[] }
```

### `addCustomColumn`: subconjunto de M aceito
Expressões `each …` com: `[Coluna]`, literais, `+ - * /`, `&`, comparações, `and/or/not`, `if … then … else …`, `null`, e as funções `Text.Upper/Lower/Proper/Trim/Start/End/Middle/Length/Contains`, `Number.Round/Abs`, `Date.Year/Month/Day/Quarter/DayOfWeek/WeekOfMonth/MonthName/DayOfWeekName` (com cultura opcional `"pt-BR"`), `Date.AddDays/AddMonths`, `Text.From`, `Number.From`, `Date.From`. Isso é compilado para SQL do DuckDB. Qualquer coisa fora do subconjunto gera um erro claro e o passo fica marcado como não executável.

## Comandos Tauri (Rust)

| Comando | Entrada | Saída |
| :--- | :--- | :--- |
| `etl_load_raw` | `{ query: string, source: EtlSource, pbipRoot?: string }` | `{ rawTable, columns: {name,type}[], rowCount }`: (re)carrega a fonte bruta em `raw__<query>` |
| `etl_preview` | `{ query: EtlQuery, uptoStep?: number, limit?: number }` | `{ columns: {name,type}[], rows: object[], rowCount: number, warnings: string[], failedStep?: { index, message } }` |
| `etl_apply` | `{ query: EtlQuery, targetTable: string }` | `ImportedTablePayload` (mesmo formato dos importadores atuais) |
| `etl_to_m` | `{ query: EtlQuery }` | `string` (script `let … in` completo) |
| `etl_from_m` | `{ name: string, m: string }` | `{ query: EtlQuery, warnings: string[] }` |

- `uptoStep` é inclusivo (o índice do passo selecionado no painel "Etapas Aplicadas"). Omitido, aplica todos.
- Num passo que falha, o preview mostra os dados até o passo anterior e preenche `failedStep`, como o Power Query.
- Nomes de colunas e tabelas são sempre citados com `quote_ident`. Valores entram como literais escapados ou parâmetros, nunca interpolados crus.

## Round-trip M
- `etl_from_m(etl_to_m(q)) == q`, sem contar `id`. Isso vale para **todo** passo do schema e é testado por propriedade em cada variante.
- Os scripts reais de `tests/m-fixtures/*.m` (Rhino, sanitizados) precisam virar consultas sem `opaqueM` nos passos suportados. A exceção esperada é a fonte de `dCalendario`, que chama a função `Query1`. `etl_to_m(etl_from_m(m))` deve preservar a semântica (comparação por re-parse, não por texto).
- Na exportação `.pbip`, o M gerado vai para a partição da tabela. Na importação, `etl_from_m` substitui o interpretador por busca de texto em `m_loader.rs`. As funções antigas ficam só como fallback, até que os testes do Rhino passem pelo caminho novo.

## Frontend
- `appliedSteps[tableId]: EtlStep[]` (substitui `PowerQueryStep`), com migração dos tipos antigos que têm equivalente. Os que não têm são descartados com aviso no console e em `migrations.ts`.
- O painel "Etapas Aplicadas" permite selecionar, renomear, excluir, mover para cima/baixo e editar com duplo clique. A grade de preview mostra o resultado até o passo selecionado (`etl_preview`), e um selo marca o passo com erro.
- A barra de fórmulas mostra o M do passo; a aba "Editor Avançado" mostra o M completo (`etl_to_m`), que pode ser editado e aplicado (`etl_from_m`).
- "Fechar e Aplicar" chama `etl_apply`. "Atualizar Dados" chama `etl_load_raw` e depois `etl_apply` para tabelas com consulta.
- Sem Tauri (navegador), cai para o `EtlEngine` TS, com aviso de recursos limitados.
