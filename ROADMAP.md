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

## `v0.3.0` — Fidelidade Visual & Compatibilidade de Relatórios PBIR (Atual)
Foco em deixar os relatórios importados do Power BI o mais fiéis possível visualmente:
- **Cartões & Formatação Avançada:** respeitar `columnCount` / orientação do `cardVisual` (ex.: 3 colunas lado a lado), opção de unidades compactas (`Mil`, `Mi`, `Bi`) e coloração opcional por sinal positivo/negativo.
- **Gráficos mais fiéis ao Power BI:**
  - **Rosca/Pizza:** rótulos com valor + `%` e total formatado exibido no centro da rosca.
  - **Funil:** escala legível para estágios finais (`minSize`), exibição de `100%` no topo e taxa de conversão no rodapé.
- **Segmentadores & Filtros PBIR:** suporte ao modo **blocos/botões (`tiles`)** para `advancedSlicerVisual` além de lista suspensa, e importação de `filterConfig` básicos (`In`) em nível de página e visual.
- **Elementos de Layout:** suporte a `textbox` e `actionButton` (`Back` / navegação entre páginas).

---

## `v0.4.0` — Motor DAX & QueryPlanner 100% Nativo em Rust + DuckDB
Foco em escalar para centenas de milhares e milhões de linhas sem sobrecarregar o frontend JavaScript:
- Migrar o `QueryPlanner` para executar agregações e agrupamentos diretamente no backend Rust + DuckDB.
- Expandir funções DAX nativas em Rust: Time Intelligence completa (`DATEADD`, `SAMEPERIODLASTYEAR`, `PREVIOUSMONTH`, `TOTALYTD`), iteradores (`SUMX`, `AVERAGEX`, `RANKX`), `ALL`/`ALLEXCEPT`/`REMOVEFILTERS` e tabelas calculadas.
- Hierarquia expansível na **Matriz** (drill-down de linhas/colunas, ex.: Grupo → Categoria).
- Painel **Performance Analyzer** medindo tempo real de consulta DAX (Rust) vs renderização (UI).

---

## `v0.5.0` — Fontes de Dados Reais & Power Query (M)
Foco em substituir a VM no fluxo diário de atualização de dados:
- Remapeamento automático de caminhos Windows → Linux ao abrir `.pbip` criado no Windows e botão **Atualizar Dados (Refresh)** sem reimportar o projeto.
- Evoluir o interpretador de **Power Query (M)** em Rust (filtros, colunas calculadas, merges/joins, `Table.Combine`).
- Conectores nativos via DuckDB/Rust: **PostgreSQL**, **MySQL**, **SQLite**, **Parquet**, **JSON** e **API REST/OData**.

---

## `v0.6.0` — Edição Profissional & Round-Trip `.pbip`
- **Undo / Redo** no canvas, guias inteligentes de alinhamento (snap guides) e seleção múltipla de visuais.
- Preservar formatação, filtros e temas no exportador `.pbip` para editar no PrismBI e reabrir no Power BI Desktop sem perdas.
- Extração aprimorada de modelos e visuais a partir de arquivos `.pbix`.

---

## `v0.7.0 → v1.0.0` — Empacotamento Linux & Comunidade
- Pipeline **GitHub Actions** gerando releases automáticas para Linux: **AppImage**, **`.deb`**, **`.rpm`**, **Flatpak** e **AUR (`prismbi-bin`)**.
- Documentação completa com capturas de tela, matriz de compatibilidade DAX/visuais e guia de contribuição.
