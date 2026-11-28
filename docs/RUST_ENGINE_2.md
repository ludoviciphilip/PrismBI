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

## 5. v0.11 Rodada 2, Frente G: desempenho do caminho sem pushdown

Meta (`docs/V011.md` Rodada 2): cair pelo menos 10× no tempo de consulta do
bench `dax::pushdown::bench_tests::bench_pushdown_vs_dax_10_million_rows`
(`--release -- --ignored`), sem aumentar RAM, e cachear a partição ordenada
das funções de janela.

### 5.1 Perfil (antes de mexer)

Sem `perf` disponível sem sudo neste ambiente, o perfil foi por fases com
`Instant` (o bench já media load/query separadamente) e por leitura de
código guiada pelos números: para 10M linhas, SUM por categoria (100
categorias), o caminho DAX gastava 1,25 s carregando e ~16,8 s consultando.

A consulta percorre `execute_visual_query_dax`'s "standard grouped series"
loop: para cada uma das 100 categorias, clona o `FilterContext`, aplica um
filtro de coluna com o valor da categoria e chama `eval_field_opt`, que por
sua vez chama `DaxModel::get_visible_row_indices`. Essa função **reconstrói
uma máscara `Vec<bool>` do tamanho da tabela para TODA tabela do modelo em
TODA chamada** (mesmo com uma única tabela). Resultado: 100 categorias × 10M
linhas ≈ 1×10⁹ visitas de linha só para calcular máscaras — bate quase exato
com os 16,8 s medidos (≈60 M linhas/s de trabalho de máscara+filtro, plausível
em Rust release para esse tipo de laço). O carregamento (`duckdb_load`) não
participa desse custo e ficou de fora do escopo desta rodada (ver §5.4).

Para as funções de janela (`src-tauri/src/dax/evaluator/window.rs`), o
mesmo padrão: `prepare_partition` refiltrava e reordenava a relação inteira
em TODA chamada, e `eval_offset`/`eval_rank`/... chamavam
`eval_window_relation` (que também varre a relação) a cada chamada. Dentro
de um `SUMX` sobre N linhas chamando `RANK`/`OFFSET` uma vez por linha, isso
é O(N² log N): um bench de 5.000 linhas com `RANK` dentro de `SUMX` levava
6,36 s nesse estado, e um de 50.000 linhas foi estimado (extrapolando a
curva quadrática observada) em ~12 minutos — inviável medir na íntegra.

### 5.2 Otimização: agregação vetorizada de passe único (query_planner.rs)

`try_vectorized_group_agg` substitui o laço "uma chamada de
`get_visible_row_indices` por categoria" por UM único passe sobre as linhas
visíveis da tabela base, acumulando SUM/COUNT/MIN/MAX/AVG/DISTINCTCOUNT por
grupo (dicionário `HashMap<ValueKey, índice do grupo>`) — O(categorias·N) →
O(N). Escopo: só quando o modelo não tem relacionamentos (filtrar uma coluna
de uma tabela sem relacionamentos é independente por linha, então o passe
único é exatamente equivalente ao laço por categoria — sem precisar
rederivar as regras de propagação de `get_visible_row_indices`) e quando os
campos do eixo Y são colunas simples (não medidas) na MESMA tabela da
categoria. Fora desse formato (medidas, colunas de outra tabela, qualquer
relacionamento), cai no caminho genérico — que continua sendo o fallback,
nunca removido. Comparado via teste diferencial contra o laço genérico
(BLANK, texto, datas, filtro pré-existente); corpus inteiro continua verde.

### 5.3 Otimização: cache de janela (evaluator/window.rs)

Três caches/índices novos, todos por `thread_local!`, com chave
`(relation, ORDERBY, PARTITIONBY, MATCHBY, BLANKS, contexto de filtro)` via
`window_cache_key` (endereço do `&Expr` de cada cláusula — estável porque é
o mesmo AST já parseado — mais um hash de conteúdo do `FilterContext`,
incluindo a sombra do ALLSELECTED). Limpos no início de toda chamada
top-level `DaxModel::evaluate_dax` — vida igual a uma avaliação de
medida/consulta, nunca atravessa avaliações diferentes:

1. **Relação cacheada** (`WINDOW_RELATION_CACHE`): `eval_window_relation`
   (que varre a relação, ex. `ALL(tabela)`) passa a rodar uma vez por grupo
   de chamadas com a mesma chave, não uma vez por linha iterada.
2. **Partições agrupadas e ordenadas** (`WINDOW_PARTITION_CACHE`):
   `prepare_partition` monta TODOS os grupos de PARTITIONBY de uma vez
   (`build_partition_groups`), cada um já ordenado por ORDERBY com o
   tie-breaker determinístico de sempre — pago uma vez, não uma vez por
   linha.
3. **Índice de posição da linha atual** (`CachedPartition::by_full_row_key`):
   o caminho sem MATCHBY fazia uma busca linear (`position()`) pela linha
   atual dentro da partição a cada chamada — O(N) escondido mesmo com a
   ordenação cacheada. Virou um `HashMap<chave da linha inteira, posição>`,
   O(1), para o caso comum (todas as colunas da relação resolvidas, que é
   exatamente o que `SUMX` entrega via contexto de linha).
4. **Rank pré-computado** (`CachedPartition::skip_rank`/`dense_rank`):
   `RANK` tinha seu PRÓPRIO laço O(posição) contando empates a cada chamada
   (`for i in 1..=pos`), um terceiro custo O(N) escondido atrás dos dois
   anteriores. Agora é um vetor pré-computado uma vez por grupo, lookup O(1).

Os `Vec`/`HashMap` cacheados são compartilhados via `Rc` (nunca clonados
linha a linha): devolver uma partição cacheada é um incremento de contador
de referência, não uma cópia de até N linhas — sem isso, o cache de
ordenação por si só não bastava (testado: a primeira versão só cacheava a
ordenação e ainda era O(N²) por causa do `clone()` do vetor e da busca
linear de posição, ver números abaixo).

Limite conhecido (documentado, não contornado por adivinhação): uma
expressão de RELATION que leia a linha ATUAL (ex. via transição de contexto
implícita dentro do argumento da relação) invalidaria a premissa de que a
relação não depende da linha iterada — formato incomum para uma função de
janela (que é justamente sobre ter um quadro estável) e não exercitado pelo
corpus; um caso futuro sensível a isso deve incluir os valores resolvidos da
linha na chave ou contornar o cache.

### 5.4 Fora do escopo desta rodada

- **Carregamento colunar com dicionário via Arrow** (§2.2 deste documento):
  `duckdb_load::load_table_from_duckdb` continua carregando linha a linha;
  o tempo de load (1,25 s/10M) não mudou nesta rodada. A meta da Rodada 2
  era só a CONSULTA (10×); o carregamento fica para a migração ColumnStore
  completa (§3, item 2), que exige trocar `DaxTableData`'s `Vec<Vec<DaxValue>>`
  por colunas tipadas em todo o motor — mudança maior que o escopo de uma
  frente.
- **Filtro por bitset / bitmap de linhas visíveis** (`get_visible_row_indices`
  continua alocando `Vec<bool>`/`Vec<usize>` por chamada): o passe único de
  `try_vectorized_group_agg` evita precisar disso no caminho rápido, mas o
  caminho genérico (medidas, relacionamentos) continua com o custo O(N) por
  chamada de `get_visible_row_indices`, só que agora chamado 1× por consulta
  em vez de 1× por categoria nos casos que caem no caminho rápido.
- **Cache por (medida, contexto) dentro de uma consulta**: não implementado
  nesta rodada; o cache de janela cobre o caso medido (RANK/OFFSET dentro de
  SUMX), que era o mandato explícito da Rodada 2.

### 5.5 Números antes/depois (release, 5 rodadas intercaladas)

Máquina compartilhada; `uptime` conferido antes de cada rodada, builds a
`Nice=19`. Binários "antes" e "depois" compilados uma vez cada e copiados
para fora do `target/`, então executados intercalados (nunca dois builds de
DuckDB do zero ao mesmo tempo).

**Bench `bench_pushdown_vs_dax_10_million_rows`** (SUM por categoria, 100
categorias, 10M linhas):

| Rodada | DAX query, antes | DAX query, depois |
|---|---|---|
| 1 | 22,45 s | 926,7 ms |
| 2 | 16,74 s | 814,1 ms |
| 3 | 16,75 s | 873,4 ms |
| 4 | 16,96 s | 899,7 ms |
| 5 | 16,97 s | 916,5 ms |

Load ficou em ~1,25 s em ambos (não mexido nesta rodada, §5.4). VmHWM: antes
≈1.624–1.625 MB, depois ≈1.625–1.627 MB — praticamente igual (meta "sem
aumentar RAM" cumprida; não há columnar store novo, só uma mudança de
algoritmo). **Speedup médio da consulta (excluindo a 1ª rodada, com ruído de
outro processo): ≈18–20×**, acima da meta de 10×.

**Bench `bench_pushdown_vs_dax_1_million_rows`:**

| Rodada | DAX query, antes | DAX query, depois |
|---|---|---|
| 1 | 1,663 s | 87,2 ms |
| 2 | 1,676 s | 81,8 ms |
| 3 | 1,695 s | 82,2 ms |
| 4 | 1,720 s | 85,1 ms |
| 5 | 1,654 s | 85,7 ms |

Speedup ≈19–20×. VmHWM ≈201–204 MB em ambos.

**Bench de janela** (`RANK(SKIP, ALL(big[ID], big[Valor]), ORDERBY(big[Valor], ASC))`
dentro de `SUMX`, tabela sem relacionamentos, release, medição única por N —
o estado "antes" em N=50.000 estava projetado em minutos pela curva
quadrática observada em N menores, então a comparação formal é em N=2.000 e
N=5.000, com N=50.000 medido só no "depois" para confirmar que a meta da
Rodada 2 (SUMX sobre 50 mil linhas) ficou rápida):

| N | antes | depois | speedup |
|---|---|---|---|
| 2.000 | 1,126 s | 21,7 ms | ≈52× |
| 5.000 | 7,18 s (↑) / 6,36 s | 130,2 ms | ≈49–55× |
| 50.000 | *(não medido — projeção ≈12 min pela curva O(N²))* | 139,8 ms | — |

A escala de 2.000→5.000 "antes" (6,25× de linhas, 6,25–7,04× o tempo
medido, raiz em N² por causa da combinação de reordenação, busca linear de
posição e laço de empates do RANK, todas O(N) por chamada) confirma a leitura
quadrática; de 5.000→50.000 "depois" o tempo cresce ~1,07× para 10× as
linhas — perto de O(N log N), dominado pela ordenação única por grupo.

### 5.6 Paridade

Corpus inteiro (412 ids) continua verde nas duas rodadas de mudança (ver
commits). Testes diferenciais novos: `fast_path_matches_generic_path_*`
(query_planner.rs, SUM/COUNT/MIN/MAX/AVG/AVERAGE/DISTINCTCOUNT com BLANK,
texto, data, filtro pré-existente) e
`window_partition_cache_matches_uncached_recompute` (window.rs, mesma
consulta RANK-dentro-de-SUMX reavaliada do zero duas vezes, resultado
idêntico). Os testes de janela pré-existentes (`rank_skip_vs_dense_and_rownumber_with_ties`,
`offset_*`, `index_*`, `window_running_total_and_moving_sum`) passam sem
alteração, provando que o cache não mudou nenhum resultado dessas funções.

## 5.7 Rodada 2: caminho vetorizado para o modelo estrela

A Rodada 1 só acelerava o caso "sem relacionamentos, categoria e y-field na
mesma tabela" — o caso real de Power BI (fato + dimensões, categoria vindo
da dimensão) continuava O(categorias·N) via `get_visible_row_indices` por
categoria. `try_vectorized_star_group_agg` (`query_planner.rs`) estende o
mesmo padrão de passe único para esse formato:

- Um passe sobre as linhas visíveis da dimensão constrói
  `HashMap<ValueKey, índice da linha da dimensão>` pela coluna-chave do
  relacionamento.
- Um passe sobre as linhas visíveis do fato resolve a categoria de cada
  linha via sua FK nesse mapa, acumulando SUM/COUNT/MIN/MAX/AVG/DISTINCTCOUNT
  por grupo — O(N) total em vez de O(categorias·N).
- Escopo deliberadamente estreito para a correção ficar provável sem
  rederivar a propagação geral de N relacionamentos: o modelo tem
  EXATAMENTE um relacionamento, ativo, de direção única (`"single"`, não
  `"both"`), com a tabela da categoria no lado "1" (`to_table`) e a tabela
  dos y-fields no lado "N" (`from_table`); sem `USERELATIONSHIP`/`CROSSFILTER`
  no contexto; y-fields são colunas simples (não medidas). Fora disso, cai
  no caminho genérico (laço por categoria), que continua sendo o fallback,
  nunca removido.
- Casamento de chave pela `ValueKey` normalizada (igual a todo outro
  filtro/join do motor): uma FK BLANK bate com uma linha de dimensão cuja
  própria chave é BLANK, sem caso especial — cai naturalmente do lookup no
  HashMap. Filtros em QUALQUER das duas tabelas (coluna, range ou tupla)
  são respeitados porque os dois conjuntos de linhas visíveis vêm do
  `get_visible_row_indices` do próprio modelo, não reimplementados aqui.

**Órfão: ajuste em relação ao enunciado da tarefa.** O brief desta rodada
pedia "linha órfã indo para o membro em branco". A implementação inicial
fez exatamente isso (FK sem nenhuma linha de dimensão correspondente ->
grupo "Sem dados"), mas isso regredia um teste diferencial JÁ existente e
testado da Frente H
(`dax::pushdown::tests_differential::grouped_sum_matches_dax_with_blanks_blank_member_and_orphan_fk`,
comentário explícito: "the orphan Valor=500 never appears") — o caminho
genérico atual (e o SQL do pushdown, que precisa bater com ele bit a bit)
já tem o comportamento documentado e testado de **descartar** a linha
órfã por completo, nunca a surfaceando em categoria alguma, nem "Sem
dados" (só uma linha de dimensão com a CATEGORIA genuinamente em branco
cai em "Sem dados"). Por "corpus é catraca: nada sai da baseline", a
versão final de `try_vectorized_star_group_agg` reproduz esse
comportamento JÁ testado em vez do enunciado solto da tarefa: linha órfã é
descartada, não bucketed. BLANK-casa-com-BLANK continua coberto (dimensão
com chave BLANK + fato com FK BLANK é um match real, não um órfão).

### Testes diferenciais

`star_fast_path_matches_oracle_*` (query_planner.rs): um oráculo
independente (laço aninhado simples, sem HashMap, sem reusar
`try_vectorized_star_group_agg`), cobrindo as 7 agregações, FK BLANK
casando com chave BLANK da dimensão, linha órfã descartada, linha de
dimensão com categoria BLANK, filtro na dimensão, filtro no fato, e
`star_fast_path_matches_oracle_across_many_fresh_models_hashmap_order`
roda a mesma comparação 64 vezes com um modelo novo (e portanto um
`HashMap` novo) a cada rodada, para pegar dependência de ordem de
iteração do HashMap. `star_fast_path_bails_out_for_two_relationships_bidirectional_or_measures`
cobre os bail-outs (2+ relacionamentos, bidirecional, inativo, y-field
medida). Corpus inteiro (412 ids) e os 388 testes de lib continuam verdes.

### Bench estrela de 10M linhas (release, 5 rodadas intercaladas)

Fato (`fact_star`) com FK para uma dimensão (`dim_cat`) de 1.000
categorias, `SUM` por categoria, carregado via DuckDB real
(`duckdb_load::load_table_from_duckdb`) com um relacionamento 1:N
single-direction ativo. Mesmo binário para antes/depois (uma flag de
ambiente `VETROBI_BENCH_FORCE_GENERIC_STAR` força o caminho genérico só
durante a medição; removida do código antes do commit), `uptime`
conferido, `systemd-run` com `Nice=19`/`CPUQuota=300%`/`MemoryMax=4G`:

| Rodada | DAX query, antes (genérico) | DAX query, depois (caminho estrela) |
|---|---|---|
| 1 | 167,3 s | 2,267 s |
| 2 | 196,2 s | 2,078 s |
| 3 | 192,8 s | 2,094 s |
| 4 | 214,8 s | 1,675 s |
| 5 | 201,6 s | 2,106 s |

**Speedup médio: ≈95×** (de ~194,5s para ~2,04s, média das 5 rodadas).
Muito acima da meta de 10× da Rodada 1 porque 1.000 categorias (vs. 100 no
bench da Rodada 1) tornam o laço O(categorias·N) do caminho genérico ainda
mais caro — o speedup cresce com o número de categorias, como esperado de
uma mudança O(C·N) -> O(N). VmHWM ficou em ~2,01–2,03 GB em ambos
("antes"/"depois"): nenhuma estrutura nova por categoria, só o algoritmo
muda — meta "sem aumentar RAM" da Rodada 1 continua valendo aqui.

Bench de 1M linhas (mesmo formato, 5 rodadas):

| Rodada | DAX query, antes | DAX query, depois |
|---|---|---|
| 1 | 16,27 s | 182,2 ms |
| 2 | 20,05 s | 190,4 ms |
| 3 | 20,28 s | 187,1 ms |
| 4 | 19,83 s | 214,7 ms |
| 5 | 19,77 s | 187,0 ms |

Speedup ≈95–100×. Load ficou em ~175–230 ms em ambos (não é o alvo desta
otimização, ver §5.8).

### 5.8 Item 4 (se der tempo): carregamento via Arrow em lote

`duckdb_load::load_table_from_duckdb_arrow` lê a tabela usando
`Statement::query_arrow` (DuckDB já reexporta `arrow` — nenhuma feature
nova no `Cargo.toml`), convertendo `RecordBatch`es inteiros (arrays
Int8..UInt64, Float32/64, Utf8/LargeUtf8, Boolean, Date32,
Timestamp(microsegundo), Decimal128) para as mesmas células `DaxValue`,
em vez de um `row.get_ref()` por célula. Qualquer coluna com um
`arrow::DataType` fora dessa lista faz a função inteira devolver `Err`
(nunca converte a tabela pela metade); `load_table_from_duckdb_fast`
tenta o caminho Arrow primeiro e cai no carregador linha a linha nesse
caso — é essa função que `load_changed_tables` (o passo de carga do
`sync_model_to_rust`) chama agora.

Testes diferenciais (`dax::duckdb_load::tests`): mesmo `DaxTableData` do
carregador linha a linha para cada tipo de coluna suportado (declarado e
inferido), NULLs em cada um, e 10 mil linhas atravessando múltiplos
`RecordBatch`es com soma em forma fechada.

**Bench isolado do load, 10M linhas, 2 colunas (release, 5 rodadas, mesmo
binário, flags iguais ao bench estrela):**

| Rodada | row-by-row | arrow |
|---|---|---|
| 1 | 1,089 s | 1,042 s |
| 2 | 1,163 s | 1,025 s |
| 3 | 1,080 s | 1,013 s |
| 4 | 1,131 s | 1,031 s |
| 5 | 1,100 s | 1,004 s |

Melhora modesta: médias 1,113 s -> 1,023 s, **≈8% mais rápido**, bem abaixo
de uma vitória grande. VmHWM do processo SUBIU no caminho Arrow (~1,23 GB
no row-by-row vs. ~1,70 GB no Arrow) — os buffers Arrow (dicionários de
string, buffers de validade/nulos) ficam alocados ao lado do
`Vec<Vec<DaxValue>>` final durante a conversão, antes de serem liberados.
Leitura honesta: a maior parte do custo do load (`CREATE TABLE ... AS
SELECT` + materialização interna do DuckDB, que já tínhamos pago antes de
qualquer conversão) não está no loop de conversão célula-a-célula que esta
mudança otimiza — o `row.get_ref()` por célula já era relativamente barato
frente ao custo de I/O e execução da query dentro do próprio DuckDB. Como
a melhora é pequena E custa RAM extra durante o load, mantive
`load_table_from_duckdb_fast` como o caminho de produção (ganho real,
ainda que modesto, e sem regressão de corretude), mas não reivindico a
meta "1,25s -> muito mais rápido" do item 4 — ela ficou sem alcançar de
forma significativa nesta rodada; o carregamento de 10M continua ~1,0–1,1s
depois desta mudança (era ~1,25s na Rodada 1, a variação é ruído de
máquina compartilhada, não desta otimização).

