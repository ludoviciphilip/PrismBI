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
  kind: 'excel' | 'csv' | 'parquet' | 'json' | 'db' | 'table' | 'inline' | 'listDates' | 'opaqueM';
  path?: string; sheet?: string;      // excel/csv/parquet/json (sheet = Item do Excel.Workbook)
  table?: string;                     // 'table': outra consulta do modelo (referência)
                                      // 'db': relação DuckDB, ex. catálogo ATTACH "pg.public.pedidos"
  m?: string;                         // 'opaqueM': fonte M não reconhecida
  columns?: string[]; rows?: Scalar[][];   // 'inline': #table({..}, {{..}}) literal (ex.: tabela de medidas)
  start?: string; count?: number; stepDays?: number;  // 'listDates': List.Dates(#date, count, #duration(stepDays,0,0,0))
  function?: string;                  // 'listDates': função chamada no M se não for List.Dates (ex.: "Query1")
}

interface EtlQuery { name: string; source: EtlSource; steps: EtlStep[] }
```

### `addCustomColumn`: subconjunto de M aceito
Expressões `each …` com: `[Coluna]`, literais, `+ - * /`, `&`, comparações, `and/or/not`, `if … then … else …`, `null`, e as funções `Text.Upper/Lower/Proper/Trim/Start/End/Middle/Length/Contains`, `Number.Round/Abs`, `Date.Year/Month/Day/Quarter/DayOfWeek/WeekOfMonth/MonthName/DayOfWeekName` (com cultura opcional `"pt-BR"`), `Date.AddDays/AddMonths`, `Text.From`, `Number.From`, `Date.From`. Isso é compilado para SQL do DuckDB. Qualquer coisa fora do subconjunto gera um erro claro e o passo fica marcado como não executável.

## Comandos Tauri (Rust)

| Comando | Entrada | Saída |
| :--- | :--- | :--- |
| `etl_load_raw` | `{ query: string, source: EtlSource, pbipRoot?: string }` | `{ rawTable, columns: {name,type}[], rowCount, sourcePath? }`: (re)carrega a fonte bruta em `raw__<query>` |
| `etl_preview` | `{ query: EtlQuery, uptoStep?: number, limit?: number }` | `{ columns: {name,type}[], rows: object[], rowCount: number, warnings: string[], failedStep?: { index, message } }` |
| `etl_apply` | `{ query: EtlQuery, targetTable: string }` | `ImportedTablePayload \| null` (mesmo formato dos importadores atuais; `null` quando a consulta não produz colunas, como a tabela de medidas, que conta como aplicada) |
| `etl_to_m` | `{ query: EtlQuery }` | `string` (script `let … in` completo) |
| `etl_from_m` | `{ name: string, m: string }` | `{ query: EtlQuery, warnings: string[] }` |

- `uptoStep` é inclusivo (o índice do passo selecionado no painel "Etapas Aplicadas"). Omitido, aplica todos. Negativo (ex. `-1`) mostra só a fonte (etapa "Source").
- `columns[].type` usa o vocabulário `EtlType` (`text`, `integer`, `decimal`, `currency` = `DECIMAL(19,4)`, `date`, `datetime`, `boolean`). Colunas brutas de Excel/CSV são `text` (as células tipadas vêm em forma canônica: números com `.`, datas ISO).
- `etl_preview`/`etl_apply` carregam `raw__<query>` sozinhos se ele ainda não existir (sem `pbipRoot`, então caminhos relativos exigem `etl_load_raw` antes).
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

## Notas da implementação v0.9.0 (motor Rust)

### Desvios do rascunho original
- **Fontes `inline` e `listDates`** (novas em `EtlSource.kind`). Sem elas, duas consultas reais do Rhino só caberiam como `opaqueM`:
  - `Medidas`: `Source = "Medidas="` + `#table(1, {{Source}})` vira `inline` (`columns: ["Column1"]`, `rows: [["Medidas="]]`); o M gerado é `#table({"Column1"}, {{"Medidas="}})`. Uma consulta que termina sem colunas (tabela de medidas) não cria tabela no modelo.
  - `dCalendario`: `Query1(#date(2026,4,1), 275, #duration(1,0,0,0))` + `Table.FromList(Source, Splitter.SplitByNothing(), null, null, ExtraValues.Error)` vira `listDates` (`start`, `count`, `stepDays`, `function: "Query1"`), gerado no DuckDB como uma coluna `Column1` de datas. No Rhino, `Query1` é `List.Dates` (expressions.tmdl); como o M da partição não traz essa definição, uma função com a mesma assinatura (data, número, `#duration` de dias inteiros) é tratada como `List.Dates`, e `etl_from_m` emite um aviso dizendo isso.
- `etl_load_raw` também devolve `sourcePath` (caminho resolvido do arquivo).

### Detalhes do contrato (sem mudar o schema)
- `addCustomColumn.expression` guarda a fórmula **sem** o `each` (como no diálogo do Power Query); `each …` também é aceito na entrada.
- `mergeQueries` é emitido como uma única etapa `Table.ExpandTableColumn(Table.NestedJoin(...), ...)`; na leitura também é aceita a forma em duas etapas (NestedJoin + ExpandTableColumn), e o passo recebe o nome da etapa de expansão.
- Chamadas aninhadas viram etapas separadas: `Table.CombineColumns(Table.TransformColumnTypes(prev, .., "it-IT"), ..)` vira `changeType` (nome `"<etapa> - Changed Type"`) + `mergeColumns`. Sem cultura, o `TransformColumnTypes` para texto das colunas mescladas é absorvido no `mergeColumns` (é o que o `to_m` sempre emite).
- Fonte `db`: `table` é o caminho da relação no DuckDB; no M ela aparece como `VetroBI.DbTable("...")`, que só o VetroBI entende (com fallback de leitura para `PrismBI.DbTable`).
- Normalizações do round-trip (equivalências semânticas): `eq/neq` com `null` viram `isNull/isNotNull`; `or[isNull c, eq c ""]` vira `isBlank`; `and[isNotNull c, neq c ""]` vira `isNotBlank`; `and/or` de um item só vira o próprio item; `count` sem coluna vira `countRows`; uma fórmula personalizada no formato de coluna condicional vira `addConditionalColumn`.
- Condições com literais de data (`#date(..)`) não cabem em `Scalar` e ficam como `opaqueM`.

### Semântica no DuckDB
- Colunas ocultas em toda a cadeia de CTEs: `__row_idx` (ordem das linhas; toda etapa a preserva ou recalcula, e o resultado final é ordenado por ela) e `__err__<coluna>` (célula com erro). Nomes que começam com `__row_idx`, `__err__` e `__etl_s` são reservados.
- **Erro x nulo** (`changeType` + `removeRowsWithErrors`): as conversões usam `TRY_CAST`/`try_strptime` (dão NULL na falha). `__err__<col>` fica TRUE quando o valor de origem **não era nulo, não era texto vazio/branco e a conversão deu NULL**. Um NULL ou `""` de origem vira NULL com a flag FALSE. Células de erro do Excel (`#N/A`…) entram como NULL com a flag TRUE. As flags seguem renomeações, duplicações e mesclagens, e são descartadas por etapas que remodelam a tabela (agrupar, dinamizar, transpor, acrescentar).
- Culturas vêm de uma tabela fixa no código (pt, es, it, de, fr, en-US, en-GB…), nunca do SO/ICU. Sem cultura: separador decimal `.`, datas ISO. Excel/inline: números canônicos (`1234.5`) não são relidos com a cultura (senão `pt-BR` leria 12345).
- Nomes de mês e dia (`Date.MonthName`/`Date.DayOfWeekName`) vêm de tabelas estáticas (pt, en, es). Sem cultura, inglês.
- Nulos no filtro como no M: `=`/`<>` tratam `null` como valor (`null <> 5` é verdadeiro), operadores relacionais e funções de texto com `null` dão `null`, que descarta a linha (inclusive dentro de `not`).
- `mergeQueries` casa `null` com `null` (como o M) e lê a tabela **já materializada** da outra consulta; na abertura de `.pbip` as consultas são processadas em ordem de dependência.

## Rodada 2 (feedback do teste no desktop): novas etapas e comandos

Adições ao schema. São compatíveis: nenhuma etapa existente muda.

```ts
type EtlStep = /* …todas as anteriores… */
  | { op: 'transformColumns'; columns: string[]; expression: string; type?: EtlType }
      // Table.TransformColumns(prev, {{"c", each <expr>, type}}). `expression` usa `_` como o valor da célula
      // (ex.: "Text.Start(_, 3)", "_ * 1.1", "Date.Year(_)", "\"R$ \" & _"). A mesma expressão vale para todas as colunas.
  | { op: 'reorderColumns'; columns: string[] }                 // Table.ReorderColumns (ordem completa)
  | { op: 'demoteHeaders' }                                     // Table.DemoteHeaders ("Usar cabeçalhos como 1ª linha")
  | { op: 'transpose' }                                         // Table.Transpose (colunas viram Column1..N)
  | { op: 'reverseRows' }                                       // Table.ReverseRows
  | { op: 'keepRange'; offset: number; count: number }          // Table.Range (offset 0-based)
  | { op: 'removeAlternateRows'; offset: number; remove: number; keep: number }   // Table.AlternateRows
  | { op: 'keepDuplicates'; columns?: string[] }                // mantém linhas cuja chave aparece mais de uma vez
  | { op: 'replaceErrors'; replacements: { column: string; value: Scalar }[] }   // Table.ReplaceErrorValues
  | { op: 'keepRowsWithErrors'; columns?: string[] }            // Table.SelectRowsWithErrors
```

Funções novas no subconjunto de expressões (`addCustomColumn` e `transformColumns`):
- **Texto:** `Text.BeforeDelimiter`, `Text.AfterDelimiter`, `Text.BetweenDelimiters` (só as formas com 2 e 3 argumentos de texto), `Text.PadStart`, `Text.PadEnd`, `Text.Replace`, `Text.Clean`, `Text.Reverse`, `Text.Repeat`.
- **Número:** `Number.Power`, `Number.Sqrt`, `Number.Mod`, `Number.IntegerDivide`, `Number.RoundUp`, `Number.RoundDown`, `Number.Sign`, `Number.Ln`, `Number.Log10`, `Number.Exp`, `Number.IsEven`, `Number.IsOdd`.
- **Data:** `Date.QuarterOfYear`, `Date.WeekOfYear`, `Date.DayOfYear`, `Date.DaysInMonth`, `Date.StartOfMonth`, `Date.EndOfMonth`, `Date.StartOfYear`, `Date.EndOfYear`, `Date.StartOfQuarter`, `Date.EndOfQuarter`, `Date.StartOfWeek`, `Date.EndOfWeek`, `Date.AddYears`, `Date.AddQuarters`, `Date.AddWeeks`, `Date.IsInCurrentYear`, `DateTime.Date`, `DateTime.Time`, `DateTime.LocalNow`, `Date.ToText(_, "yyyy-MM")` (só formatos com `yyyy yy MM M dd d MMM MMMM`, nomes de mês de tabela fixa e cultura opcional).
- **Duração:** `Duration.Days(a - b)`, quando a diferença é entre datas.

Novo comando:

| Comando | Entrada | Saída |
| :--- | :--- | :--- |
| `etl_step_m` | `{ query: EtlQuery, index: number }` | `string`: só a expressão da etapa `index`, como na barra de fórmulas do Power Query (`= Table.AddColumn(#"Etapa Anterior", …)`) |

- **Edição na barra de fórmulas:** o frontend troca a expressão da etapa no M completo (`etl_to_m`), reconverte com `etl_from_m` e substitui só aquela etapa. Se a conversão falhar, mostra o erro e não altera a consulta.
- **"Atualizar Dados":** tabelas com consulta ETL são recarregadas por `etl_load_raw` (com `pbipRoot`) seguido de `etl_apply`. **Nunca** pelo interpretador M antigo nem por `refresh_pbip_tables`, que descartariam as etapas editadas.

### Desvios da Rodada 2 (implementação)
- **`keepDuplicates` não tem equivalente nativo no Power Query.** É emitido como uma pseudo-função `VetroBI.KeepDuplicates(prev, {"col", ...})` (ou sem a lista de colunas, para "todas as colunas"), no mesmo espírito de `VetroBI.DbTable` para a fonte `db`. Só o `etl_from_m` do VetroBI reconhece essa forma (e `PrismBI.KeepDuplicates` legado); ela não roda em outro motor de Power Query. Round-trip: `etl_to_m(etl_from_m(m)) == m` para scripts gerados pelo próprio VetroBI.
- **`Table.TransformColumns(prev, {{"c", each Text.Upper(_), type text}})` com uma das 5 funções de texto já suportadas por `textTransform`** (`Upper/Lower/Proper/Trim/Clean`) é normalizado para `textTransform` na leitura, não para `transformColumns` — mesmo padrão das outras normalizações semânticas já documentadas (`eq null` → `isNull`, etc.). Para preservar uma etapa como `transformColumns` explícita, use uma expressão que não seja exatamente `<Função>(_)` com uma dessas 5 funções (ex. `Text.Start(_, 3)`, `_ * 2`, `Text.Upper(_) & ""`).
- **`transpose`** materializa `Column1..ColumnN` (N = nº de linhas da tabela de origem) com uma sub-select por célula; por simplicidade de implementação há um limite de 10 000 células (linhas × colunas de origem) — tabelas maiores falham com uma mensagem clara em vez de rodar uma consulta muito cara. As flags de erro são descartadas (remodela a tabela), como os demais passos de reshape.
- **`demoteHeaders`** descarta as flags de erro das colunas (viram `Column1..N`, todas texto) e insere a linha de cabeçalho com `__row_idx` = `min(__row_idx) - 1`.
- **`Date.ToText`**: os tokens `MMM` (mês abreviado) usam os 3 primeiros caracteres do nome completo da tabela estática (`pt`/`en`/`es`), não uma tabela de abreviações própria — para `pt-BR` isso diverge um pouco do Power Query em meses como "setembro" → "set" é igual, mas é uma aproximação, não uma tabela oficial de abreviações.
- **`DateTime.Time`** não tem um `EtlType` próprio no contrato (só `date`/`datetime`); o valor fica como categoria interna "outro" (SQL `TIME`), e só pode ser materializado convertendo para texto (`type: "text"` na etapa, ou `Text.From`).
- **`Duration.Days(a - b)`**: reconhecido apenas quando o argumento é sintaticamente uma subtração `a - b` com `a`/`b` avaliando para data/data-hora; a diferença é em dias inteiros entre as datas (parte de hora é descartada). Qualquer outra forma de produzir uma duração (ex. variável intermediária) não é suportada e gera erro claro.
- **`DateTime.LocalNow()`** é não determinístico por natureza (hora de execução); os testes verificam apenas que a etapa compila e executa sem falhar, não um valor fixo.
