# Cobertura de testes do crate Rust (`src-tauri`) — v0.10, rodada 1

Medido com `cargo-llvm-cov` (instrumentação LLVM, não contagem de `#[test]`).
Comando usado (via `systemd-run`, target dir dedicado para não recompilar o
DuckDB do dia a dia):

```sh
cd src-tauri
systemd-run --user --wait --collect --pipe --quiet \
  -p WorkingDirectory="$PWD" -p Nice=19 -p CPUQuota=300% \
  -E CARGO_BUILD_JOBS=3 \
  -E CARGO_TARGET_DIR='/mnt/jogos/Power BI on Linux/src-tauri/target-cov' \
  cargo llvm-cov --lib --tests --summary-only -j 3
```

## Limitação do "antes" medido

A máquina é compartilhada com outras frentes do v0.10 (cada uma compilando o
mesmo crate Tauri+DuckDB em paralelo) e com renders de vídeo de outro
projeto. Uma rodada de `cargo-llvm-cov` recompila o crate inteiro do zero
(DuckDB incluso) e levou entre 35 e 50 minutos; uma primeira tentativa com a
paralelização padrão do cargo foi **matada pelo OOM killer** do kernel (ver
`journalctl --user`: `run-p3134783-*.service: The kernel OOM killer killed
some processes in this unit`) com a RAM da máquina perto do limite. A
segunda tentativa, com `-j 3` e `CPUQuota=300%`, terminou com sucesso.

Dado o custo de cada rodada, só foi possível medir **uma vez com
cargo-llvm-cov**, já com os testes novos desta rodada aplicados — não há um
"antes" instrumentado (linha a linha) desta tarefa especificamente para
comparar. O que dá para afirmar com certeza, pelo estado do código antes
desta rodada (via `grep -L '#\[test\]'` nos 48 arquivos `.rs` do crate):

- **Zero `#[test]` em todo o arquivo**: `src/etl/sql.rs` (1952 linhas),
  `src/etl/m_from.rs` (1482 linhas), `src/etl/m_to.rs` (720 linhas),
  `src/lib.rs` (534 linhas), `src/dax/lexer.rs`, `src/dax/parser.rs`,
  `src/dax/ast.rs`, `src/dax/duckdb_load.rs`, `src/dax/evaluator/{aggregate,
  filter,row,scalar,table,time}.rs`, `src/pbi/mod.rs`, `src/engine/mod.rs`,
  `src/etl/commands.rs`, `src/etl/custom_expr.rs`, `src/connectors/{mod,
  manager}.rs`, `src/main.rs`.
- Os outros 26 arquivos já tinham suíte própria (notavelmente
  `dax/evaluator.rs`, `dax/mod.rs`, `dax/functions.rs` parcialmente,
  `pbi/tmdl.rs`, `pbi/pbip.rs`, `pbi/pbix.rs`, `pbi/pbip_writer.rs`,
  `pbi/m_loader.rs`, `updater.rs`, `connectors/config.rs`,
  `connectors/keyring_store.rs`, todo `etl/tests/*.rs`), o que já cobria boa
  parte de `etl/m_from.rs`/`etl/m_to.rs`/`etl/sql.rs` **indiretamente**
  (via `query_from_m`/`query_to_m`/`compile`, chamados pelos testes de
  `etl::tests::*`), mesmo sem terem teste próprio no arquivo.

A tabela "depois" abaixo é a medição real; ela já mostra, por arquivo, a
combinação do que a suíte antiga cobria indiretamente **mais** o que os
testes novos desta rodada acrescentaram.

## Depois (medido): cobertura por arquivo

(`cargo llvm-cov --lib --tests --summary-only`, inclui `tests/dax_corpus.rs`,
`tests/connectors_duckdb_sqlite.rs`, `tests/connectors_sqlite_network.rs`)

| Arquivo | Linhas cobertas | Funções cobertas | Regiões |
|---|---|---|---|
| connectors/config.rs | 88.29% | 100.00% | 89.30% |
| connectors/keyring_store.rs | 56.04% | 40.00% | 57.38% |
| connectors/manager.rs | 73.44% | 47.92% | 63.96% |
| dax/duckdb_load.rs | 72.68% | 68.75% | 70.35% |
| dax/evaluator.rs | 94.56% | 97.62% | 92.80% |
| dax/evaluator/aggregate.rs | 86.17% | 78.38% | 80.60% |
| dax/evaluator/filter.rs | 78.09% | 71.43% | 77.73% |
| dax/evaluator/row.rs | 92.23% | 100.00% | 86.67% |
| dax/evaluator/scalar.rs | 78.07% | 79.17% | 72.97% |
| dax/evaluator/table.rs | 93.97% | 94.44% | 90.05% |
| dax/evaluator/time.rs | 88.89% | 96.97% | 84.60% |
| **dax/functions.rs** | **76.66%** | **85.71%** | 76.66% |
| **dax/lexer.rs** | **78.36%** | **60.00%**¹ | 75.98% |
| dax/mod.rs | 77.24% | 84.04% | 74.17% |
| **dax/parser.rs** | **92.45%** | **100.00%** | 82.41% |
| dax/query_planner.rs | 72.59% | 57.14% | 70.98% |
| **engine/duckdb_store.rs** | **72.99%** | **59.13%** | 69.34% |
| etl/commands.rs | 73.26% | 60.61% | 70.04% |
| etl/custom_expr.rs | 84.79% | 90.00% | 71.25% |
| **etl/m_from.rs** | **75.63%** | **85.71%** | 66.44% |
| etl/m_parse.rs | 79.70% | 81.61% | 67.93% |
| **etl/m_to.rs** | **94.57%** | **85.71%** | 90.34% |
| etl/model.rs | 86.69% | 95.83% | 89.55% |
| etl/raw.rs | 76.88% | 59.26% | 70.37% |
| **etl/sql.rs** | **89.82%** | **89.81%** | 86.08% |
| **lib.rs** | **0.00%** | **0.00%** | 0.00% |
| main.rs | 0.00% | 0.00% | 0.00% |
| **pbi/m_loader.rs** | **75.68%** | **69.23%** | 72.69% |
| **pbi/mod.rs** | **64.59%** | **54.69%** | 61.30% |
| pbi/pbip.rs | 76.00% | 78.40% | 77.82% |
| pbi/pbip_writer.rs | 76.22% | 55.81% | 75.72% |
| pbi/pbix.rs | 84.95% | 77.78% | 84.12% |
| **pbi/tmdl.rs** | **96.51%** | **90.00%** | 94.53% |
| **updater.rs** | **68.67%** | **57.89%** | 71.88% |
| **TOTAL (crate)** | **79.86%** | **75.06%** | 77.95% |

¹ `dax/lexer.rs`'s only uncovered *function record* is a monomorphized
closure instance (the real function `tokenize` is covered); see "funções
sem cobertura" abaixo, que deduplica por nome (soma de contagem entre todos
os binários de teste), não por registro do LLVM.

Negrito = arquivos alvo desta rodada (parsers M/TMDL/pbix, DAX functions,
ETL SQL/M, DuckDB store, updater, lib.rs).

## Funções públicas / `pub(crate)` SEM NENHUMA cobertura, por módulo

Lista deduplicada (soma da contagem de chamadas de cada função através de
**todos** os binários de teste — `--lib`, `dax_corpus`,
`connectors_duckdb_sqlite`, `connectors_sqlite_network` — e não apenas o
primeiro registro do LLVM, que varia por binário/instanciação genérica).

- **dax/functions.rs**: `pseudo_random_0_1`, `today_ymd` — ambas dependem
  do relógio/estado de processo (`SystemTime::now()`), não têm valor
  determinístico para testar por igualdade; cobertas apenas
  indiretamente (chamadas por `RAND`/`TODAY`/`NOW`, cujo retorno os testes
  não podem comparar a um valor fixo).
- **dax/parser.rs, dax/lexer.rs, dax/ast.rs, dax/duckdb_load.rs**: **zero**
  funções sem cobertura.
- **pbi/tmdl.rs, pbi/mod.rs, pbi/pbix.rs, pbi/pbip_writer.rs**: **zero**
  funções sem cobertura.
- **pbi/pbip.rs**: `extract_pbir_textbox_text` (um branch de extração de
  texto de caixa de texto do PBIR ainda não exercitado pelos fixtures).
- **pbi/m_loader.rs**: `_keep_sanitize_import` (função placeholder/`#[allow(dead_code)]`-style para manter um import vivo; não é lógica real).
- **etl/m_from.rs**: `collect_ident_spans`, `rename_ident_refs` (ramos do
  renomeador de identificadores M usados só em alguns padrões de
  `each`/variável livre que os fixtures atuais não disparam).
- **etl/m_to.rs**: **zero** funções sem cobertura.
- **etl/sql.rs**: `serial_to_timestamp` (conversão de serial Excel para
  TIMESTAMP; `serial_to_date` tem teste, o par timestamp ficou de fora).
- **engine/duckdb_store.rs**: `DuckDbStore::get_table_preview`,
  `DuckDbStore::import_xlsx`, `DuckDbStore::list_tables` — têm equivalentes
  ou vizinhos testados (`import_xlsx_sheet`, `query_sql`), mas o wrapper
  específico não tem teste direto.
- **updater.rs**: `check_for_updates`, `download_and_install_update`,
  `detect_install_kind`, `http_get_bytes` — todas fazem rede real
  (`ureq::get` contra a API do GitHub) ou leem `std::env::current_exe()`/
  variáveis de ambiente do processo real; by design não são chamadas em
  teste unitário (a lógica pura que elas delegam para —
  `evaluate_release`, `classify_install`, `parse_sha256sums`,
  `verify_detached_signature`, `is_newer_version`, `normalize_version`,
  `is_trusted_download_url` — está 100% testada nesta rodada).
- **lib.rs**: as 48 funções do arquivo (comandos `#[tauri::command]` e
  `run()`) estão a 0%: todas pedem `tauri::State<AppState>` ou
  `tauri::AppHandle`, isto é, uma janela/app Tauri real. Não há harness de
  teste de comando Tauri sem UI neste crate; a lógica de negócio que cada
  comando delega (DuckDB, DAX, ETL, pbi, updater, connectors) já está
  testada nos módulos correspondentes.

## O que ainda fica sem cobertura — e por quê

| Área | Por quê |
|---|---|
| `lib.rs` (comandos Tauri, `run()`) | Precisam de `tauri::State`/`AppHandle`/janela real; sem harness de teste de comando Tauri headless no projeto. |
| `updater::check_for_updates`, `download_and_install_update`, `http_get_bytes` | Fazem rede real contra a API do GitHub; a lógica pura por trás (`evaluate_release`, `parse_sha256sums`, `verify_detached_signature`) está 100% testada. |
| `updater::detect_install_kind` | Lê `std::env::current_exe()` do processo de teste real; a versão pura testável (`classify_install`/`classify_install_windows`) está 100% testada. |
| `dax::functions::pseudo_random_0_1`, `today_ymd` | Não-determinísticos (relógio/estado de processo); sem valor esperado fixo para asserir. |
| `main.rs` | Só chama `vetrobi_lib::run()`; é o binário, não a lib testada. |
| Alguns ramos raros de `etl/m_from.rs` (renomeação de identificador livre), `pbi/pbip.rs` (texto de caixa de texto) e `etl/sql.rs` (timestamp de serial) | Exigem fixtures mais exóticos (`.pbip` com caixa de texto populada, M com variável livre renomeada) que não existem ainda no corpus de testes do projeto. |
| `connectors/keyring_store.rs` (56%), `connectors/manager.rs` (73%) | Vários branches pedem um chaveiro Secret Service real ou conexão de rede a Postgres/MySQL/SQLite reais; os testes existentes já cobrem o caminho "sem rede"; o resto está marcado `#[ignore]` com uma mensagem explicando o requisito (ver `connectors_duckdb_sqlite.rs`). |

## Testes adicionados nesta rodada

Novos arquivos (fora de `src/dax/**`, `src/pbi/**` e `src/updater.rs`, o
território de outras frentes paralelas — a única alteração nesses
diretórios foi a declaração dos três módulos de teste em `lib.rs`):

- `src/tests_cov_dax.rs` — 42 testes: funções de data pura (`parse_iso_ymd`,
  `add_months`, `civil_to_days`/`days_to_civil`, `serial_to_civil`…),
  estatística (`calc_median`, `calc_percentile`, `calc_variance`/`stdev`,
  `calc_geomean`), `try_eval_scalar_fn` para Math/Trig, Texto, Data/Hora e
  Financeiro (`MOD`, `CEILING`/`FLOOR`, `FACT`, `GCD`/`LCM`, `MID`, `FIND`
  vs `SEARCH`, `SUBSTITUTE`, `VALUE`, `DATE`/`EDATE`/`EOMONTH`/`WEEKDAY`/
  `DATEDIFF`, `PMT`, `FV`, `SLN`, `XNPV`/`XIRR`), cobrindo BLANK e bordas
  (divisão por zero em `MOD`/`CEILING`, texto vazio em `VALUE`, `LN`/`FACT`
  fora de domínio, XIRR sem sinais opostos).
- `src/tests_cov_pbi.rs` — 10 testes: `parse_table_tmdl` (colunas, medida
  com formatString e com code fence, tabela oculta, comentário `///`,
  entrada vazia é erro) e `parse_relationships_tmdl` (1:N, N:N, 1:1,
  bidirecional inativa, bloco incompleto é ignorado).
- `src/tests_cov_updater.rs` — 17 testes: `normalize_version`/
  `is_newer_version` (prefixo `v`, pré-release, comprimentos diferentes),
  `classify_install`/`classify_install_windows` (AppImage, Flatpak,
  pacman/deb/rpm, MSI vs portátil), `parse_sha256sums` (nome exato,
  `*nome` com wildcard, hash curto/não-hex rejeitado), `is_trusted_download_url`
  (prefixo exato, `..`, URL embutida), `verify_detached_signature` (chave/
  assinatura malformadas não derrubam o processo), `evaluate_release`
  (sem update, instalação gerenciada, asset ausente, sums sem assinatura).
- `src/etl/sql.rs` (`mod tests_cov`, dentro do próprio arquivo) — 16 testes
  nas funções de geração de SQL com foco em injeção: `quote_ident`/
  `lit_str` com aspas e NUL embutidos, `lit_num` com NaN/±Infinity,
  `check_user_name` rejeitando nomes reservados, `cat_of`/`etl_type_name`,
  `culture_info` (pt-BR, en-US, locale inválida), `convert_expr` (texto em
  branco vira NULL, Decimal→Date via serial), `clean_err`.
- `src/engine/duckdb_store.rs` (dentro do `mod tests` existente) — 5 testes
  novos: `sanitize_identifier` (incluindo um nome em formato de injeção SQL
  virando identificador inofensivo), `format_days_since_epoch` (ancoras
  conhecidas, incluindo 2024-02-29), `bump_table_version`/`get_table_version`
  (case-insensitive, monotônico), `calamine_cell_to_json` (coerções number/
  boolean/texto em branco), `import_csv` com arquivo inexistente.

Total: **90 testes novos**, todos com asserts de valor (nada de
`assert!(result.is_ok())` isolado quando o valor dava para verificar), sem
`#[ignore]` com stub e sem estado global compartilhado entre testes (o
único contador global tocado, `bump_table_version`, usa uma chave de tabela
exclusiva do teste — `cov_probe_tbl_7f3a` — e lê o delta, não um valor
absoluto).

## Bugs encontrados escrevendo os testes

Nenhum: todas as discrepâncias entre a expectativa inicial e o
comportamento real (ex.: `add_months` clampa o dia ao mês de destino,
`DATE`/`EDATE`/`EOMONTH` retornam texto ISO e não um valor `DateTime`
binário, `VALUE`/`LN`/`FACT` fora de domínio retornam `BLANK` em vez de
erro, cardinalidade `fromCardinality: many`/`toCardinality: one` é `N:1`,
`isHidden` de tabela exige indentação de 1 nível) eram do **teste**, não do
motor — corrigidos antes do commit, sem tocar em código de produção.

## Suíte normal: tempo antes/depois

`cargo test --lib --test dax_corpus` (target dir compartilhado normal, não
instrumentado):

- **Depois** (com os 90 testes novos): `test result: ok. 252 passed; 0
  failed; 3 ignored` em **5.97s** + `dax_corpus`: 1 passed em **0.46s**.
  Total de execução dos testes: **~6.4s**, bem abaixo do limite de 60s.
- **Antes**: não foi medido isoladamente (rodar a suíte antiga exigiria
  reverter os commits desta rodada e recompilar de novo no mesmo target
  dir compartilhado, competindo com as outras 4 frentes); como contexto,
  a suíte já tinha ~190 testes antes desta rodada e historicamente corre
  em poucos segundos (ver `CONTRIBUTING.md`/CI), então a adição de 90
  testes determinísticos e sem I/O não deveria (e não demonstrou, no
  "depois") mudar a ordem de grandeza.
- Nenhum teste novo é lento: todos são cálculos puros, parsing de strings
  pequenas ou (no caso de `import_csv_rejects_missing_file`) uma tentativa
  de abrir um arquivo inexistente, que falha imediatamente.

## CI

`.github/workflows/coverage.yml`: `workflow_dispatch` + agendamento semanal
(segunda 06:00 UTC), nunca em PR (a instrumentação recompila o crate
inteiro, 20+ minutos). Roda em `ubuntu-22.04`, usa
`Swatinem/rust-cache@v2` com `shared-key: coverage` e
`save-if: github.ref == 'refs/heads/main'` como os outros workflows,
instala `cargo-llvm-cov` via `taiki-e/install-action`, mede
`--lib --test dax_corpus`, publica o resumo em `$GITHUB_STEP_SUMMARY` e
sobe o `lcov.info` como artifact (`lcov-report`, 30 dias de retenção).
