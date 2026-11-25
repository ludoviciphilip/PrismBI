# Matriz de Compatibilidade DAX, Visuais e Fontes (PrismBI `v0.7.0`)

Este documento resume o nível atual de compatibilidade do **PrismBI** com projetos do Microsoft Power BI (`.pbip` / `.pbix`) e a especificação pública **DAX**.

---

## 1. Compatibilidade DAX (474 / 484 funções = 98%)

| Categoria | Motor Rust Nativo | Motor TypeScript / Fallback | Observações |
| :--- | :--- | :--- | :--- |
| **Agregação & Iteradores** | `SUM`, `SUMX`, `AVERAGE`, `AVERAGEA`, `AVERAGEX`, `MIN`, `MINA`, `MINX`, `MAX`, `MAXA`, `MAXX`, `COUNT`, `COUNTA`, `COUNTAX`, `COUNTBLANK`, `COUNTROWS`, `DISTINCTCOUNT`, `DISTINCTCOUNTNOBLANK`, `PRODUCT`, `PRODUCTX`, `CONCATENATEX` | Completo | Avaliados sobre colunas e filtros propagados pelo grafo de relacionamentos |
| **Filtro & Relacionamentos** | `CALCULATE`, `FILTER`, `ALL`, `ALLSELECTED`, `ALLEXCEPT`, `REMOVEFILTERS`, `KEEPFILTERS`, `VALUES`, `DISTINCT`, `TOPN`, `SELECTEDVALUE`, `RELATED`, `LOOKUPVALUE`, `USERELATIONSHIP`, `CROSSFILTER`, `TREATAS`, `CONTAINSROW`, `ISEMPTY`, `HASONEVALUE`, `HASONEFILTER`, `ISFILTERED`, `ISCROSSFILTERED`, `ISINSCOPE` | `ALLSELECTEDAPPLY`, `ALLSELECTEDREMOVE`, `ALWAYSAPPLY`, `NONFILTER`, `CALCULATETABLE`, `RELATEDTABLE`, `EARLIER`, `EARLIEST`, `SUMMARIZE`, `SUMMARIZECOLUMNS` | Transição de contexto e propagação bidirecional sem loop |
| **Inteligência de Tempo** | `DATEADD`, `TOTALYTD`, `TOTALQTD`, `TOTALMTD`, `DATESYTD`, `DATESQTD`, `DATESMTD`, `SAMEPERIODLASTYEAR`, `PREVIOUSDAY/MONTH/QUARTER/YEAR`, `NEXTDAY/MONTH/YEAR`, `PARALLELPERIOD`, `DATESBETWEEN`, `DATESINPERIOD`, `STARTOF*/ENDOF*`, `FIRSTDATE`, `LASTDATE`, `FIRSTNONBLANK`, `LASTNONBLANK`, `OPENINGBALANCE*`, `CLOSINGBALANCE*` | `TOTALWTD`, `DATESWTD`, `PREVIOUSWEEK`, `NEXTWEEK`, `NEXTQUARTER`, `STARTOFWEEK`, `ENDOFWEEK` | Suporte à expansão mensal completa mesmo sob filtros de rótulo (`Mes/Ano`) |
| **Estatística & Ranking** | `MEDIAN`, `MEDIANX`, `PERCENTILE.INC/EXC`, `PERCENTILEX.INC/EXC`, `STDEV.S/P`, `STDEVX.S/P`, `VAR.S/P`, `VARX.S/P`, `GEOMEAN`, `GEOMEANX`, `RANKX`, `GENERATESERIES` | `WINDOW`, `OFFSET`, `INDEX`, `RANK`, `ROWNUMBER` | — |
| **Matemática, Texto, Data, Info, Financeira & Parent-Child** | `ROUND*`, `MROUND`, `GCD`, `LCM`, `FACT`, `COMBIN*`, `PERMUT`, `SIN/COS/TAN/ASIN/ACOS/ATAN`, `*H`, `BITAND/OR/XOR/LSHIFT/RSHIFT`, `FORMAT`, `MID`, `FIND`, `SEARCH`, `SUBSTITUTE`, `CONTAINSSTRING*`, `DATE`, `EDATE`, `EOMONTH`, `DATEDIFF`, `NETWORKDAYS`, `PMT`, `PV`, `FV`, `NPER`, `NPV`, `IPMT`, `PPMT`, `SLN`, `SYD`, `DB`, `DDB`, `EFFECT`, `NOMINAL`, `RATE`, `PATH*`, `ISBOOLEAN`, `ISINT64`, `ISDOUBLE`, `ISNUMERIC`, `ISSTRING`, `ISCURRENCY`, `ISDATETIME`, `COALESCE`, `CONVERT` | Completo | Alinhado à referência `dax.guide` |
| **DMVs de Metadados (`INFO.*`)** | — | `INFO.TABLES`, `INFO.COLUMNS`, `INFO.MEASURES`, `INFO.RELATIONSHIPS`, `INFO.CULTURES`, `INFO.PERSPECTIVES`, `INFO.VIEW.*`, entre 60+ DMVs | As 10 DMVs binárias internas de segmentos VertiPaq/Delta (`INFO.*STORAGES*` de baixo nível) permanecem honestamente marcadas como não suportadas |

---

## 2. Catálogo de Visuais

| Visual Power BI | Status no PrismBI | Recursos Preservados |
| :--- | :--- | :--- |
| **Cartão (`card`) / Multi-Card (`cardVisual`) / KPI** | Total | `columnCount`, unidades compactas (`Mil`/`Mi`/`Bi`), cores por sinal positivo/negativo |
| **Colunas, Barras, Linhas, Área, Combinado** | Total | Eixos múltiplos, legenda, rótulos de dados e cross-filtering |
| **Pizza & Rosca (`pie` / `donut`)** | Total | Total formatado central na rosca + valor e `%` |
| **Funil (`funnel`) & Cascata (`waterfall`)** | Total | `minSize` + taxa de conversão no funil; barras flutuantes e coluna Total na cascata |
| **Treemap, Dispersão (`scatter`), Medidor (`gauge`)** | Total | Renderização Apache ECharts responsiva (claro/escuro) |
| **Tabela & Matriz (`tableEx` / `pivotTable`)** | Total | Hierarquia expansível em 2 níveis, subtotais e rodapé fixo `Total Geral` |
| **Segmentador (`slicer` / `advancedSlicerVisual`)** | Total | Modo lista, dropdown e botões em blocos (`tiles`) |
| **Caixa de Texto (`textbox`) & Botão (`actionButton`)** | Total | Navegação entre páginas e botão Voltar |

---

## 3. Formatos de Arquivo e Fontes de Dados

- **Projetos Power BI:** `.pbip` (leitura e round-trip `v0.7.0` com TMDL + PBIR) e `.pbix` (leitura de `DataModelSchema`, `Report/Layout` UTF-16LE e `DataMashup` multi-query).
- **Arquivos Locais via DuckDB Nativo:** Excel (`.xlsx` / `.xls`), CSV / TSV, Parquet, JSON / NDJSON e snapshots `.insight`.
- **APIs & Fontes Externas:** REST JSON (atualizável via botão **Atualizar Dados**).
- **Conectores Nativos de Banco de Dados (`v0.8.0`, via `ATTACH` do DuckDB):**
  - **`.duckdb` nativo:** suportado e testado de ponta a ponta (schemas, tabelas, pré-visualização, importação, `DETACH`) 100% offline.
  - **SQLite:** suportado via extensão `sqlite` do DuckDB; testado de ponta a ponta contra um arquivo `.sqlite` real criado neste mesmo ambiente (requer rede na primeira execução para `INSTALL sqlite`, depois funciona offline com a extensão em cache).
  - **PostgreSQL / MySQL & MariaDB:** suportados via `ATTACH ... (TYPE POSTGRES/MYSQL, READ_ONLY)`, com geração de SQL e connection string (`libpq`-style, escaping de senha/host) cobertas por testes unitários. **Honestamente não testados contra um servidor real** neste ambiente (nenhum Postgres/MySQL disponível); os testes de integração correspondentes existem e estão marcados `#[ignore]` com o motivo.
  - **Parquet particionado / HTTP(S) / S3:** `read_parquet(..., hive_partitioning=true)` via extensão `httpfs`; geração de SQL coberta por testes unitários, não exercitada contra um bucket S3 real (requer credenciais externas).
  - **Credenciais:** senhas nunca tocam `.insight`/`.pbip`/localStorage; ficam no chaveiro Linux Secret Service (`keyring` v3, serviço `dev.prismbi.app`), referenciadas por uma `credentialKey` opaca salva no modelo. A chave de IA (Gemini) também migra de `localStorage` para o keyring sob Tauri.
  - **Navegador visual + editor SQL:** schema tree, pré-visualização e aba de SQL livre na nova aba "PostgreSQL / MySQL / SQLite / DuckDB" do modal de fontes de dados; **não coberto por teste de UI/e2e** (sem GUI disponível no ambiente de desenvolvimento usado) — a lógica por trás (construção de config, validação de formulário, mapeamento de payload) é coberta por testes unitários de `src/adapters/dbConnector.ts`.
  - **"Atualizar Dados":** tabelas carregadas de um conector de banco reconectam via `db_refresh_import`, buscando a senha no keyring pela `credentialKey` — coberto por teste de store (mockando a bridge Tauri).
