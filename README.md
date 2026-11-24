# PrismBI

Aplicação desktop nativa para Linux de Business Intelligence (**Tauri v2 + Rust + DuckDB + React/TypeScript + Apache ECharts**) com interface familiar ao Power BI, motor analítico DuckDB embarcado, avaliação DAX nativa em Rust e compatibilidade com projetos `.pbip` (TMDL + PBIR) e `.pbix`.

> **Licença / License:** [The Mutualist License v1.3](./LICENSE.md) — baseado originalmente em componentes de UI e engine DAX do [InsightHub](https://github.com/ludoviciphilip/Insight-Hub).

## Principais Recursos

- **Nativo no Linux:** roda diretamente sobre GTK/WebKitGTK sem VM Windows.
- **Compatibilidade Power BI:** leitura de `.pbip` (`*.SemanticModel` em TMDL + `*.Report` em PBIR) e `.pbix`, além de exportação `.pbip`.
- **Motor Analítico em Rust + DuckDB:** carga de `.xlsx`/`.csv`, propagação de relacionamentos, medidas DAX com `CALCULATE`, `SWITCH`, `VAR`/`RETURN` e inteligência de tempo.
- **Visuais Interativos:** Cartões KPI e Multi-Card (com suporte a `columnCount` e coloração por sinal), gráficos de Colunas, Barras, Linhas, Área, Combinado, Pizza/Rosca (com total central), Funil (com taxa de conversão), Cascata, Treemap, Dispersão, Gauge, Matriz, Tabela, Segmentadores (`dropdown` e `tiles`), caixas de texto e botões de ação.

Consulte o planejamento completo em [ROADMAP.md](./ROADMAP.md).

## Como executar em desenvolvimento

```bash
pnpm install
pnpm tauri dev
```

## Build de produção

```bash
pnpm tauri build
```

## Dicas para GPU NVIDIA + Wayland no Linux

Por padrão, o PrismBI utiliza aceleração por hardware (DMA-BUF) do WebKitGTK. Em sistemas com drivers NVIDIA proprietários no Wayland que apresentarem tela branca ou falha de sincronização, tente iniciar com:

```bash
__NV_DISABLE_EXPLICIT_SYNC=1 pnpm tauri dev
# ou, em último caso, via XWayland / fallback por software:
GDK_BACKEND=x11 pnpm tauri dev
WEBKIT_DISABLE_DMABUF_RENDERER=1 pnpm tauri dev
```
