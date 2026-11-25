# Roadmap do PrismBI

Este documento descreve a evolução planejada para o **PrismBI**, uma aplicação desktop nativa para Linux construída com **Tauri v2 + Rust + DuckDB + React/TypeScript + Apache ECharts**, focada em produtividade e compatibilidade com projetos do Power BI (`.pbip` / `.pbix`).

> **Política de Versionamento (Pré-1.0):** Enquanto o app evolui até a estabilidade plena de produção, seguimos versionamento incremental em `0.x.0` (`0.8.0`, `0.9.0`, `0.10.0`, `0.11.0`, `0.12.0`, `0.13.0`...), reservando a `v1.0.0` apenas para quando o conjunto completo de recursos estiver consolidado, testado em alta escala e estável.

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

## `v0.6.0` — Edição Profissional, Round-Trip `.pbip` & Biblioteca DAX Completa (Concluído)
- [x] **Motor DAX Nativo Expandido (+110 funções alinhadas à especificação pública DAX):** funções matemáticas/trigonométricas, texto (`FORMAT`, `MID`, `TRIM`, `FIND`, `SEARCH`, `SUBSTITUTE`, `CONTAINSSTRING`), data/hora (`DATE`, `EDATE`, `EOMONTH`, `DATEDIFF`, `NETWORKDAYS`, `WEEKDAY`, `WEEKNUM`, `QUARTER`), estatísticas/iteradores (`MEDIAN`, `PERCENTILE.*`, `STDEV.*`, `VAR.*`, `PRODUCT`, `GEOMEAN`, `CONCATENATEX`, `COUNTBLANK`), tabela/relacionamento (`VALUES`, `DISTINCT`, `TOPN`, `ALLSELECTED`, `LOOKUPVALUE`, `USERELATIONSHIP`, `CROSSFILTER`), informação (`ISFILTERED`, `ISCROSSFILTERED`, `HASONEFILTER`, `ISINSCOPE`), financeiras (`PMT`, `PV`, `FV`, `NPER`) e Time Intelligence completo (`TOTALQTD`, `DATESQTD`, `PARALLELPERIOD`, `DATESBETWEEN`, `DATESINPERIOD`, `STARTOF*`/`ENDOF*`, `FIRSTDATE`/`LASTDATE`, `OPENINGBALANCE*`/`CLOSINGBALANCE*`).
- [x] **Edição Profissional no Canvas:** histórico **Undo / Redo** (`Ctrl+Z` / `Ctrl+Y` / Ribbon), **seleção múltipla** (`Shift`/`Ctrl` + clique, arraste em grupo, nudge com setas, `Delete` e barra flutuante de alinhamento/distribuição) e **Smart Snap Guides** a 60fps.
- [x] **Round-Trip `.pbip` & Extração `.pbix` Aprimorada:** exportação preservando scripts Power Query (M), colunas de cartões múltiplos, `advancedSlicerVisual` (`tiles`), `textbox`, `actionButton`, títulos, `filterConfig` em páginas/visuais e temas (`PrismTheme.json`), além de extração multi-query do `DataMashup` em `.pbix`.

---

## `v0.7.0` — Empacotamento Linux, Auto-Updater, Tela Inicial & 98% DAX (Atual)
- [x] **GitHub Actions CI/CD:** pipeline de validação automática (`ci.yml`) e geração de releases Linux **AppImage**, **`.deb`**, **`.rpm`** com checksums **SHA-256** (`release.yml`), além de manifestos para **AUR (`prismbi-bin`)** e **Flatpak**.
- [x] **Atualizador Automático Integrado no App (`updater.rs` + `UpdateModal`):** verificação automática via GitHub Releases, download seguro com validação SHA-256, atualização atômica in-place para **AppImage** e download direto para `.deb`/`.rpm`.
- [x] **Tela Inicial (`WelcomeScreen`) & Navegação:** inicialização com hub de projetos recentes, retomada da última sessão salva e abertura direta de `.pbip`, `.pbix` ou `.insight` (sem forçar template), além de pan livre no canvas e modelo com botão do meio (scroll pressionado) e atalho `Esc` no Modo Apresentação.
- [x] **Conformidade DAX 98% (474/484) & Motor Rust Expandido:** migração para Rust de `TREATAS`, `CONTAINSROW`, `GENERATESERIES`, `FIRSTNONBLANK`/`LASTNONBLANK`, `COALESCE`, `CONVERT`, família `IS*`, bit-shifts, combinatória/trigonometria, financeiro avançado (`NPV`, `IPMT`, `PPMT`, `SLN`, `SYD`, `DB`, `DDB`, `EFFECT`, `NOMINAL`, `RATE`) e `PATH*`, além de expansão das DMVs `INFO.*` no catálogo.
- [x] **Documentação & Comunidade:** [matriz de compatibilidade](./docs/COMPATIBILITY.md) e [guia de contribuição](./CONTRIBUTING.md).

---

## `v0.8.0` — Conectores Nativos de Banco de Dados (via DuckDB) & Credenciais Seguras (Concluído parcialmente — ver observações)
- [x] Conectores nativos reais em DuckDB para **PostgreSQL**, **SQLite**, **MySQL/MariaDB**, arquivos **`.duckdb`** e **Parquet particionado / HTTP / S3** (sem depender de gateway externo). Implementado via `ATTACH ... (TYPE POSTGRES/SQLITE/MYSQL, READ_ONLY)` e `read_parquet(..., hive_partitioning=true)` com `INSTALL`/`LOAD` das extensões `postgres`/`sqlite`/`mysql`/`httpfs`. **Testado de ponta a ponta** contra um `.duckdb` real (100% offline) e um `.sqlite` real criado neste ambiente (extensão baixada via rede). **PostgreSQL 16 e MariaDB 11 testados contra servidores reais** (containers locais e serviços do CI), incluindo senhas com aspas e espaços, que ficam num `TEMPORARY SECRET` do DuckDB e nunca na connection string. Parquet/S3 remoto ainda não foi exercitado contra um bucket real.
- [x] Navegador visual de schemas e tabelas com editor SQL nativo e pré-visualização antes da carga. Comandos Rust: `db_connect`, `db_list_schemas`, `db_list_tables`, `db_preview_table`, `db_query_preview`, `db_import_table`, `db_disconnect`. UI: nova aba "PostgreSQL / MySQL / SQLite / DuckDB" no `DataSourceModal`, substituindo o antigo conector simulado (`postgresMockAdapter`, que permanece só com seus próprios testes, fora dos caminhos de produção). A navegação/formulário em si não tem teste de UI automatizado (sem GUI no ambiente); a lógica (construção de config, validação, mapeamento) é testada em `src/adapters/dbConnector.ts`.
- [x] Integração com o botão **Atualizar Dados** para recarregar diretamente a partir do banco conectado: tabelas de origem `db` guardam um `TableSourceConfig.db` (tipo do conector, config sem senha, `credentialKey`, tabela/SQL de origem) e o refresh chama `db_refresh_import`, que reconecta buscando a senha no keyring.
- [x] Armazenamento seguro de credenciais utilizando o chaveiro nativo do Linux (`Secret Service` / crate `keyring` v3), evitando senhas em texto puro em arquivos `.insight` ou `.pbip`. Camada abstraída por um trait (`CredentialStore`) testável com um fake em memória (o Secret Service real não está disponível em CI/sandboxes — há um teste `#[ignore]` documentando isso). A chave de API do Gemini (antes em `localStorage`) também migra para o keyring sob Tauri, com fallback de navegador preservado.

---

## `v0.9.0` — Power Query Visual (ETL) & Round-Trip Real de Scripts `M`
- [ ] Editor ETL visual passo a passo com preview em tempo real: **Alterar Tipo**, **Filtrar Linhas**, **Dividir/Mesclar Colunas**, **Substituir Valores**, **Preencher Abaixo/Acima**, **Unpivot / Pivot**, **Agrupar Por (`GroupBy`)**, **Coluna Condicional/Personalizada**, **Mesclar (`Merge/Join`)** e **Acrescentar (`Append`)**.
- [ ] Geração e leitura bidirecional (round-trip) dos scripts **Power Query (`M`)** correspondentes na importação e exportação de projetos `.pbip`.

---

## `v0.10.0` — Fidelidade Avançada de Relatórios & Interatividade Profissional
- [ ] **Formatação Condicional** em Tabelas, Matrizes, Cartões e Barras (regras, gradientes por medida DAX e barras de dados).
- [ ] **Bookmarks (Indicadores)** e botões de ação vinculados a Bookmarks para alternar estado de visuais, filtros e páginas com 1 clique.
- [ ] **Report Page Tooltips** (páginas configuradas como tooltip dinâmico ao passar o cursor nos gráficos).
- [ ] **Slicers Avançados:** filtro de intervalo de datas (*Between* / *Relative Date*), busca interna nos itens e **Sincronização de Slicers** entre páginas.
- [ ] **Drill-Down hierárquico em gráficos** (ex.: Ano → Trimestre → Mês → Dia) e suporte a **3+ níveis expansíveis na Matriz**.

---

## `v0.11.0` — Escalabilidade Real (1M–10M+ linhas): Persistência Colunar & SQL Pushdown
- [ ] Formato de projeto colunar comprimido (cache local DuckDB/Parquet ao lado dos metadados) para abrir milhões de linhas em menos de 1 segundo com baixo consumo de RAM.
- [ ] **Pushdown para DuckDB SQL** em agregações, joins e filtros pesados antes da camada DAX.
- [ ] **Tabelas Calculadas e Colunas Calculadas** (`CALENDAR`, `SUMMARIZE`, `ADDCOLUMNS`, `UNION`) executadas nativamente em Rust.
- [ ] Migração para Rust das funções avançadas de tabela e janela (`WINDOW`, `OFFSET`, `INDEX`, `RANK`, `ROWNUMBER`, `CALCULATETABLE`, `SUMMARIZECOLUMNS`).

---

## `v0.12.0` — Integração Desktop Linux, Produtividade & CLI/Automação
- [ ] **Associação nativa de arquivos no Linux:** abrir `.pbip`, `.pbix` e `.insight` com duplo clique no gerenciador de arquivos (via argumentos CLI no Tauri).
- [ ] **Command Palette (`Ctrl+K`)** para busca rápida de páginas, medidas, tabelas, visuais e ações, além de suporte a **múltiplas janelas**.
- [ ] **Exportação Profissional & CLI Headless:** PDF multi-página em alta resolução, PNG por visual, exportação CSV/Excel dos dados filtrados de qualquer visual e subcomandos CLI (`prismbi refresh` / `prismbi export`) para automação via cron/servidor.
- [ ] **IA 100% Local (Ollama) + BYOK:** gerar/explicar medidas DAX e diagnosticar modelos usando LLMs locais sem enviar dados para a nuvem.

---

## `v0.13.0` — Hardening, Testes E2E, Benchmarks & Canais Oficiais (Pré-1.0)
- [ ] Suite completa de **testes E2E automatizados** cobrindo importação `.pbip`/`.pbix`, atualização de dados, filtros, temas claro/escuro e exportação.
- [ ] Benchmarks reproduzíveis para 10k, 100k e 1M+ linhas, zero warnings em `tsc --strict` e `cargo clippy`, e revisão 100% da localização (`EN`, `PT`, `ES`).
- [ ] Publicação oficial no **Flathub** e atualização automatizada do **AUR (`prismbi-bin`)** no pipeline de release.

---

## `v1.0.0` — Release Estável (Production-Ready)
- [ ] Estabilidade garantida do formato de projeto, paridade consolidada para uso diário em produção e ciclo de manutenção sem breaking changes imprevistos.
