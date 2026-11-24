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

## `v0.5.0` — Atualização Real de Fontes, Mapeamento de Tabelas & Conectores Nativos (Atual)
Foco em substituir a VM no fluxo diário de atualização e gerenciamento de dados:
- [x] **Atualização Real (`Atualizar Dados`):** recarregamento direto dos arquivos vinculados (`.xlsx`, `.csv`, `.parquet`, `.json`, REST e `.pbip`) preservando páginas, visuais, medidas, filtros e relacionamentos.
- [x] **Mapeamento, Substituição & Renomeação de Tabelas:** vincular qualquer aba do Excel (ex.: aba `Fato` → tabela `fLançamentos`), substituir dados de tabelas existentes sem quebrar o relatório e renomear tabelas atualizando automaticamente fórmulas DAX, relacionamentos, visuais e filtros.
- [x] **Remapeamento Windows → Linux & Power Query (M):** resolução inteligente de caminhos vindos do Windows (`C:\Users\...`, subpastas locais) e suporte a `Table.RemoveColumns`, `Table.SelectColumns`, `Table.RenameColumns` e `Table.SelectRows`.
- [x] **Conectores Nativos DuckDB:** suporte embutido para **Parquet**, **JSON** e ingestão direta de **API REST (JSON)**.

---

## `v0.6.0` — Edição Profissional & Round-Trip `.pbip`
- **Undo / Redo** no canvas, guias inteligentes de alinhamento (snap guides) e seleção múltipla de visuais.
- Preservar formatação, filtros e temas no exportador `.pbip` para editar no PrismBI e reabrir no Power BI Desktop sem perdas.
- Extração aprimorada de modelos e visuais a partir de arquivos `.pbix`.

---

## `v0.7.0 → v1.0.0` — Empacotamento Linux & Comunidade
- Pipeline **GitHub Actions** gerando releases automáticas para Linux: **AppImage**, **`.deb`**, **`.rpm`**, **Flatpak** e **AUR (`prismbi-bin`)**.
- Documentação completa com capturas de tela, matriz de compatibilidade DAX/visuais e guia de contribuição.
