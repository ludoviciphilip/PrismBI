# Motor DAX em Rust 2.0: diagnóstico e plano de migração

> Status: proposta (v0.8-dev). Decisão: o motor principal é o Rust; o motor TS
> (herdado do InsightHub) vira fallback congelado e depois sai.

## 1. Como o motor Rust funciona hoje

| Aspecto | Hoje (`src-tauri/src/dax`) | Consequência |
| :--- | :--- | :--- |
| **Tipos** | `DaxValue = Blank \| Number(f64) \| Text \| Boolean` | Data é texto ISO. Não há `Integer`/`Currency`/`DateTime`. `"2024"` (texto) e `2024` (número) têm a mesma `canonical_key`, então filtros misturam tipos. Moeda acumula erro de `f64`. |
| **Igualdade/filtros** | `canonical_key()` → `String`, filtros = `HashSet<String>` | Cada comparação aloca uma string. Lento e semanticamente frouxo. |
| **Armazenamento** | `Vec<Vec<DaxValue>>` por linha, enviado como JSON pelo frontend (`sync_model_to_rust`) | Os dados existem três vezes: no JS, no DuckDB e no Rust. Toda edição reenvia o modelo inteiro. |
| **Contexto de filtro** | `tabela → coluna → valores permitidos` | Só representa filtros por coluna. Filtros de **tupla** (`FILTER` em 2 colunas, `SUMMARIZE`, `TREATAS` multi-coluna), `ALLSELECTED` real (contexto "sombra") e `KEEPFILTERS` sobre tabelas não cabem. |
| **Propagação** | `get_visible_row_indices` refaz as máscaras de **todas** as tabelas a cada chamada | Custa O(linhas do modelo) por avaliação de medida. Uma matriz 50×20 com 3 medidas são cerca de 3 mil varreduras completas. |
| **Expressões de tabela** | `DaxTableData` clonado (`TableEval::Owned`) | Muitas cópias em iteradores aninhados. |
| **Membro desconhecido** | não existe | Fato com chave sem dimensão some em vez de cair na linha em branco. |

O corpus mostrou um caso concreto disso: `MAXX(VALUES(d_Produto[Categoria]), [Total Vendas])`
retorna 320 em vez de 920. A transição de contexto sobre `VALUES` de uma
coluna que não é chave não chega corretamente ao fato.

## 2. Arquitetura proposta

```
DuckDB (fonte da verdade dos dados)
   │  Arrow RecordBatch (sem JSON)
   ▼
ColumnStore  ──  coluna = dicionário de valores distintos + Vec<u32> (ids por linha)
   │               (+ índice de relacionamento: chave → linhas, calculado uma vez)
   ▼
FilterContext ── lista de filtros; cada filtro = (colunas, conjunto de tuplas de ids)
   │               Bitset por tabela, memoizado por (contexto, tabela)
   ▼
Evaluator     ── DaxValue tipado; tabelas como "views" (ids de linha + colunas), sem cópia
   ▼
QueryPlanner  ── visual → grupos → medidas; agregações simples podem ir para o DuckDB (pushdown, v0.11)
```

### 2.1 Valores tipados
```rust
enum DaxValue { Blank, Boolean(bool), Integer(i64), Decimal(f64), Currency(i64 /* ×10⁴ */),
                DateTime(f64 /* dias desde 1899-12-30, como o DAX */), Text(Arc<str>) }
```
- Mesmas regras de conversão do DAX (BLANK→0/""/FALSE conforme o operador; `Currency` com 4 casas).
- Data vira número de série, o que torna `DATEADD`, `EOMONTH` e `DATEDIFF` aritmética simples. Isso resolve uma classe inteira de bugs de inteligência de tempo.

### 2.2 Armazenamento colunar com dicionário
- Carregado do DuckDB com `query_arrow`, por tabela. O frontend não envia mais as linhas, só metadados (tipos, relacionamentos, medidas).
- Filtro de coluna = `BitSet` sobre os ids do dicionário. Filtrar uma tabela vira um teste de bit por linha, sem nenhuma string.
- Relacionamento: `HashMap<id_chave_dim, Vec<row_fato>>` e a direção inversa, calculados no carregamento.

### 2.3 Contexto de filtro com tuplas
- `Filter { columns: Vec<ColRef>, tuples: TupleSet }`, em que `TupleSet` é um bitset no caso de uma coluna só ou um `HashSet<SmallVec<u32>>` para várias.
- `CALCULATE` aplica os modificadores na ordem documentada: primeiro ALL*/REMOVEFILTERS, depois USERELATIONSHIP/CROSSFILTER, depois os filtros, com KEEPFILTERS fazendo interseção.
- `ALLSELECTED` usa a pilha de contextos de "shadow filters" do visual.

### 2.4 Cache
- Máscara visível por `(hash do contexto, tabela)`, com vida igual a uma consulta de visual.
- Cache de medida por `(medida, hash do contexto)`.

## 3. Ordem de migração (orientada pelo corpus)

Cada passo só entra quando o corpus inteiro continua verde (a baseline nunca regride).

1. **v0.8.x, fundação de tipos:** `DaxValue` tipado + `DateTime` serial, sem trocar o armazenamento. Fazer `time.json`/`scalar.json` passarem em Rust.
2. **v0.9, ColumnStore + Arrow:** carregar do DuckDB; o `sync_model_to_rust` passa a mandar só metadados. Benchmark de 100k/1M linhas no CI (roadmap v0.13).
3. **v0.10, FilterContext 2.0 — concluído:** `Filter { columns, tuples }` (`TupleFilter`, §2.3) e ALLSELECTED real (contexto sombra capturado em `build_filter_context`, restaurado por `FilterContext::shadow_ctx()`) entraram no motor Rust. `cases/filter.json` + `filter_rel.json` passam 100% (82/82, ver `docs/DAX_CONFORMANCE.md`), incluindo os 15+ casos novos de tuplas/ALLSELECTED/KEEPFILTERS/membro em branco. Sem regressão na baseline (364 → 383 ids Rust). Bench de 200k linhas (`cargo test --lib bench_200k -- --ignored --nocapture`, release): 5 execuções intercaladas contra `origin/v0.10-dev` (worktree temporário, binário copiado antes de cada rodada) — base 3,78 ms/query (média de 3.802/3.759/3.757/3.821/3.784) vs. depois 3,85 ms/query (média de 3.818/3.867/3.841/3.806/3.899); variação de ≈+1,6%, dentro do ruído e bem abaixo do limite de regressão de 15%.
   - Limite conhecido, documentado e deixado para v0.11: um filtro de tupla só junta colunas da **mesma** tabela (FILTER/SUMMARIZE/TREATAS com 2+ colunas de uma dimensão ou da própria fato). Um SUMMARIZE que combina colunas de **duas dimensões diferentes** (ex. `SUMMARIZE(f_Vendas, d_Produto[Categoria], d_Cliente[Cidade])`) ainda decompõe em filtros de coluna independentes por tabela — exigiria propagar a tupla através da fato via RELATED, equivalente a uma "relação virtual" de verdade.
   - Limite conhecido: `ALL(coluna)`/`clear_column` sobre uma coluna que participa de um `TupleFilter` remove a tupla inteira (perde a restrição das outras colunas), em vez da marginalização exata do DAX.
4. **v0.11, tabelas/janela em Rust:** `SUMMARIZECOLUMNS`, `CALCULATETABLE`, `WINDOW`/`OFFSET`/`INDEX`/`RANK`/`ROWNUMBER`, colunas e tabelas calculadas. Pushdown de SUM/COUNT/DISTINCTCOUNT agrupados para o DuckDB.
5. **Desligar o fallback TS por categoria** quando Rust ≥ TS no corpus daquela categoria. Remover `src/engine/expr` e `src/engine/functions` no fim.

## 4. Métrica de progresso

`docs/DAX_CONFORMANCE.md` (gerado por `pnpm dax:report`) substitui o "98%" do
catálogo. O catálogo continua útil como inventário de funções reconhecidas pelo
parser, mas não como medida de corretude.
