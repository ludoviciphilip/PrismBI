# Roadmap do PrismBI

Este documento descreve a evolução planejada para o **PrismBI**, uma aplicação desktop nativa para Linux construída com **Tauri v2 + Rust + DuckDB + React/TypeScript + Apache ECharts**, focada em produtividade e compatibilidade com projetos do Power BI (`.pbip` / `.pbix`).

---

## `v0.2.0` — Fundação Nativa & Primeiro Projeto `.pbip` (Concluído)
- [x] Shell nativo Linux com Tauri v2 e **DuckDB** embarcado em memória para ingestão de CSV/XLSX.
- [x] Parser nativo em Rust para projetos `.pbip` (TMDL + PBIR) e `.pbix` (layout UTF-16LE e esquema).
- [x] Engine **DAX nativa em Rust** com suporte a `CALCULATE`, `SWITCH`, `VAR`/`RETURN`, medidas encadeadas e propagação de filtros via relacionamentos.
- [x] Suporte inicial a visuais (cartões, cartões múltiplos, gráficos de colunas/barras/linhas/pizza/rosca/funil/cascata/treemap, matriz e segmentador).
- [x] Correção de resolução de campos PBIR renomeados (`field.*.Property`), aceleração por GPU no WebKitGTK Linux e otimizações de fluidez na interface.

---

## `v0.3.0` — Fidelidade Visual & Compatibilidade de Relatórios PBIR (Concluído)
Foco em deixar os relatórios importados do Power BI o mais fiéis possível visualmente:
- [x] **Cartões & Formatação Avançada:** respeitar `columnCount` / orientação do `cardVisual` (ex.: 3 colunas lado a lado), opção de unidades compactas (`Mil`, `Mi`, `Bi`) e coloração opcional por sinal positivo/negativo.
- [x] **Gráficos mais fiéis ao Power BI:**
  - **Rosca/Pizza:** rótulos com valor + `%` e total formatado exibido no centro da rosca.
  - **Funil:** escala legível para estágios finais (`minSize`), exibição de `100%` no topo e taxa de conversão no rodapé.
  - **Cascata:** variações positivas/negativas cruzando zero, linhas conectoras e coluna `Total` final.
- [x] **Segmentadores & Filtros PBIR:** suporte ao modo **blocos/botões (`tiles`)** para `advancedSlicerVisual` além de lista suspensa, e importação de `filterConfig` básicos (`In`) em nível de página e visual.
- [x] **Elementos de Layout:** suporte a `textbox` e `actionButton` (`Back` / navegação entre páginas).

---

## `v0.4.0` — Motor DAX & QueryPlanner Nativo em Rust + DuckDB (Concluído)
Foco em escalar consultas para o backend Rust nativo e melhorar a análise hierárquica:
- [x] **QueryPlanner Nativo em Rust (`query_visual_native`):** execução de consultas por visual (cartões, multi-cards, segmentadores, gráficos agrupados e matrizes) diretamente sobre o `DaxModel` em Rust com fallback automático.
- [x] **Expansão de Funções DAX em Rust:** Time Intelligence (`TOTALYTD`, `TOTALMTD`, `DATESYTD`, `DATESMTD`, `PREVIOUSYEAR`, `NEXTMONTH`, `NEXTYEAR`, `PREVIOUSQUARTER`), `RANKX` e escalares (`INT`, `SQRT`, `POWER`).
- [x] **Hierarquia Expansível na Matriz:** expansão/recolhimento em 2 níveis de linha diretamente no visual de Matriz (com seletor de sub-linha no painel de Visualizações).
- [x] **Performance Analyzer Real:** telemetria do tempo de execução das consultas nativas em Rust registrada automaticamente na aba de Governança.

---

## `v0.5.0` — Atualização Real de Fontes, Mapeamento de Tabelas & Conectores Nativos (Concluído)
Foco em substituir a VM no fluxo diário de atualização e gerenciamento de dados:
- [x] **Atualização Real (`Atualizar Dados`):** recarregamento direto dos arquivos vinculados (`.xlsx`, `.csv`, `.parquet`, `.json`, REST e `.pbip`) preservando páginas, visuais, medidas, filtros e relacionamentos.
- [x] **Mapeamento, Substituição & Renomeação de Tabelas:** vincular qualquer aba do Excel (ex.: aba `Fato` → tabela `fLançamentos`), substituir dados de tabelas existentes sem quebrar o relatório e renomear tabelas atualizando automaticamente fórmulas DAX, relacionamentos, visuais e filtros.
- [x] **Remapeamento Windows → Linux & Power Query (M):** resolução inteligente de caminhos vindos do Windows (`C:\Users\...`, subpastas locais) e suporte a `Table.RemoveColumns`, `Table.SelectColumns`, `Table.RenameColumns` e `Table.SelectRows`.
- [x] **Conectores Nativos DuckDB:** suporte embutido para **Parquet**, **JSON** e ingestão direta de **API REST (JSON)**.

---

## `v0.6.0` — Edição Profissional, Round-Trip `.pbip` & Biblioteca DAX Completa (Atual)
- [x] **Motor DAX Nativo Expandido (+110 funções alinhadas à especificação pública DAX):** funções matemáticas/trigonométricas, texto (`FORMAT`, `MID`, `TRIM`, `FIND`, `SEARCH`, `SUBSTITUTE`, `CONTAINSSTRING`), data/hora (`DATE`, `EDATE`, `EOMONTH`, `DATEDIFF`, `NETWORKDAYS`, `WEEKDAY`, `WEEKNUM`, `QUARTER`), estatísticas/iteradores (`MEDIAN`, `PERCENTILE.*`, `STDEV.*`, `VAR.*`, `PRODUCT`, `GEOMEAN`, `CONCATENATEX`, `COUNTBLANK`), tabela/relacionamento (`VALUES`, `DISTINCT`, `TOPN`, `ALLSELECTED`, `LOOKUPVALUE`, `USERELATIONSHIP`, `CROSSFILTER`), informação (`ISFILTERED`, `ISCROSSFILTERED`, `HASONEFILTER`, `ISINSCOPE`), financeiras (`PMT`, `PV`, `FV`, `NPER`) e Time Intelligence completo (`TOTALQTD`, `DATESQTD`, `PARALLELPERIOD`, `DATESBETWEEN`, `DATESINPERIOD`, `STARTOF*`/`ENDOF*`, `FIRSTDATE`/`LASTDATE`, `OPENINGBALANCE*`/`CLOSINGBALANCE*`).
- [x] **Edição Profissional no Canvas:** histórico **Undo / Redo** (`Ctrl+Z` / `Ctrl+Y` / Ribbon), **seleção múltipla** (`Shift`/`Ctrl` + clique, arraste em grupo, nudge com setas, `Delete` e barra flutuante de alinhamento/distribuição) e **Smart Snap Guides** a 60fps.
- [x] **Round-Trip `.pbip` & Extração `.pbix` Aprimorada:** exportação preservando scripts Power Query (M), colunas de cartões múltiplos, `advancedSlicerVisual` (`tiles`), `textbox`, `actionButton`, títulos, `filterConfig` em páginas/visuais e temas (`PrismTheme.json`), além de extração multi-query do `DataMashup` em `.pbix`.

---

## `v0.7.0 → v1.0.0` — Empacotamento Linux & Comunidade
- Pipeline **GitHub Actions** gerando releases automáticas para Linux: **AppImage**, **`.deb`**, **`.rpm`**, **Flatpak** e **AUR (`prismbi-bin`)**.
- Documentação completa com capturas de tela, matriz de compatibilidade DAX/visuais e guia de contribuição.
