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
