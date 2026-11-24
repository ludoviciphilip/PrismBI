# PrismBI

Aplicação desktop nativa para Linux (Tauri v2 + Rust + DuckDB + React/TypeScript + Apache ECharts) com interface familiar ao Power BI, motor analítico DuckDB nativo em processo, suporte a importação de `.csv`/`.xlsx` e leitura de projetos `.pbip` (TMDL + PBIR) e `.pbix`.

> **Licença / License:** [The Mutualist License v1.3](./LICENSE.md) — baseado originalmente em componentes de UI e engine DAX do [InsightHub](https://github.com/ludoviciphilip/Insight-Hub).

## Como executar em desenvolvimento

```bash
pnpm install
pnpm tauri dev
```

## Build de produção

```bash
pnpm tauri build
```
