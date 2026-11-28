<div align="center">

<img src="./src-tauri/icons/128x128.png" width="96" height="96" alt="VetroBI" />

# VetroBI

**Alternativa open-source ao Power BI para Linux & Windows — app desktop de BI nativo e leve que abre projetos Power BI (`.pbip` / `.pbix`), com motor DAX em Rust, Power Query visual e DuckDB**

🇺🇸 [Read in English](./README.md) • 🇧🇷 **Português (Brasil)**

[![Versão](https://img.shields.io/badge/vers%C3%A3o-0.11.1-f59e0b?style=flat-square)](./package.json)
[![CI](https://github.com/ludoviciphilip/VetroBI/actions/workflows/ci.yml/badge.svg)](https://github.com/ludoviciphilip/VetroBI/actions/workflows/ci.yml)
[![Release](https://github.com/ludoviciphilip/VetroBI/actions/workflows/release.yml/badge.svg)](https://github.com/ludoviciphilip/VetroBI/actions/workflows/release.yml)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8DB?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-Engine_DAX-000000?style=flat-square&logo=rust&logoColor=white)](./src-tauri)
[![DuckDB](https://img.shields.io/badge/DuckDB-Embarcado-FFF000?style=flat-square&logo=duckdb&logoColor=black)](https://duckdb.org/)
[![Licença](https://img.shields.io/badge/Licen%C3%A7a-Mutualist_v1.3-10b981?style=flat-square)](./LICENSE.md)

[Funcionalidades](#-principais-funcionalidades) • [Prévia Visual](#-prévia-visual) • [Compatibilidade](./docs/COMPATIBILITY.md) • [Arquitetura](#-arquitetura) • [Como Executar](#-como-executar) • [Contribuindo](./CONTRIBUTING.md) • [Roadmap](./ROADMAP.md)

</div>

---

O **VetroBI** nasceu para resolver um dos maiores gargalos de quem usa Power BI no Linux (e, desde a `v0.9.1`, também no Windows x64): analisar modelos semânticos, relatórios e dashboards com a mesma fluidez e familiaridade do **Power BI Desktop**, sem depender de máquinas virtuais pesadas nem de um tenant online.

Construído com **Tauri v2 + Rust + DuckDB** no backend e **React + TypeScript + Apache ECharts** na interface, ele entrega inicialização rápida, baixo consumo de RAM e execução nativa em GTK/WebKitGTK.

---

## 📸 Prévia Visual

### Visão de Relatório (Modo Escuro & Modo Claro)

| Modo Escuro | Modo Claro |
| :---: | :---: |
| ![VetroBI Report View Dark Mode](./docs/images/report-dark.png) | ![VetroBI Report View Light Mode](./docs/images/report-light.png) |

### Modelo Semântico (Esquema Estrela) & Consultas DAX

| Modelo Semântico & Relacionamentos | Estúdio de Consultas DAX |
| :---: | :---: |
| ![VetroBI Semantic Model View](./docs/images/model-view.png) | ![VetroBI DAX Query View](./docs/images/dax-query.png) |

---

## ✨ Principais Funcionalidades

### 📂 Compatibilidade com Power BI (`.pbip` / `.pbix`)
- **Importação nativa em Rust:** leitura direta de pastas `.pbip` (`*.SemanticModel` em formato **TMDL** + `*.Report` em formato **PBIR**) e arquivos `.pbix` (`DataModelSchema`, `Report/Layout` UTF-16LE e extração multi-query do `DataMashup`).
- **Round-Trip `.pbip` (`v0.6.0`):** exportação preservando scripts Power Query (M), `columnCount` em cartões múltiplos, `advancedSlicerVisual` (`tiles`), `textbox`, `actionButton`, títulos, `filterConfig` em páginas/visuais e temas (`VetroTheme.json`), além de snapshots `.insight` e PDF.
- **Fidelidade PBIR:** resolução correta de medidas renomeadas (`field.*.Property`), `columnCount` em cartões múltiplos, `filterConfig` em páginas/visuais, hierarquias de data, botões de ação (`actionButton`) e caixas de texto (`textbox`).

### 📊 Catálogo de Visuais Interativos
- **Cartões & KPIs:** Cartão KPI simples e Cartão Múltiplo (`cardVisual`) com grade dinâmica (`columnCount`), unidades compactas (`Mil`, `Mi`, `Bi`) e coloração condicional por sinal positivo/negativo.
- **Gráficos Analíticos (Apache ECharts):** Colunas, Colunas Empilhadas, Barras, Barras Empilhadas, Linhas, Área, Combinado (Coluna + Linha), Treemap, Dispersão e Medidor (Gauge).
- **Pizza & Rosca:** total formatado exibido no centro da rosca, rótulos externos com valor e percentual e tooltips instantâneos.
- **Funil & Cascata estilo Power BI:** Funil com `minSize` legível, topo `100%` e taxa de conversão no rodapé; Cascata real com variações positivas/negativas cruzando zero, linhas tracejadas conectoras e coluna `Total` final.
- **Tabelas, Matrizes & Segmentadores:** Matriz Pivot com totais por linha/coluna, Tabela interativa com Drill-Through e Segmentadores no modo lista suspensa (`dropdown`) ou blocos clicáveis (`tiles`).

### ⚡ Motor Analítico em Rust + DuckDB (`v0.9.0`)
- **DuckDB embarcado:** ingestão nativa rápida de **Excel (`.xlsx`/`.xls`)**, **CSV**, **Parquet**, **JSON** e endpoints **API REST (JSON)**, além de console SQL nativo.
- **Atualização Real & Gestão de Tabelas:** botão **Atualizar Dados** que recarrega arquivos/fontes vinculados sem perder visuais, medidas ou relacionamentos — incluindo mapeamento de abas com nomes diferentes (ex.: aba `Fato` → `fLançamentos`), substituição de dados por outra planilha e renomeação completa de tabelas.
- **Power Query visual (`v0.9.0`):** editor no estilo do Power BI (Consultas, abas Página Inicial/Transformar/Adicionar Coluna/Exibir, ~80 operações, Etapas Aplicadas editáveis, barra de fórmulas por etapa, Editor Avançado, qualidade/distribuição/perfil de colunas). Cada consulta é reexecutada no DuckDB a partir da fonte bruta (40 tipos de etapa, não destrutivo), e o **M é lido e gerado de verdade** (round-trip na importação e exportação de `.pbip`), com resolução automática de caminhos Windows → Linux.
- **Modelo com todas as linhas (`v0.9.0`):** o motor Rust lê as tabelas direto do DuckDB (1 milhão de linhas em ~0,13 s); a prévia de 5.000 linhas fica só para as grades.
- **Conformidade DAX medida: 364/368 (98,9%) no motor Rust nativo,** num [corpus verificado de forma independente](./docs/DAX_CONFORMANCE.md) de 368 casos (valores esperados calculados por oráculos em Python, executado em todo build do CI). **Motor Rust tipado (+165 funções),** cerca de 3× mais rápido que a v0.7 num benchmark de 200 mil linhas: avaliação nativa de `CALCULATE`, `SWITCH`, `RANKX`, `TREATAS`, `CONTAINSROW`, `GENERATESERIES`, `COALESCE`, `CONVERT`, família `IS*`, bit-shifts, matemática/trigonometria, texto (`FORMAT`, `MID`, `FIND`, `SEARCH`, `SUBSTITUTE`, `CONTAINSSTRING`), estatísticas/iteradores (`MEDIAN`, `PERCENTILE.*`, `STDEV.*`, `VAR.*`, `CONCATENATEX`), relacionamentos (`VALUES`, `DISTINCT`, `TOPN`, `LOOKUPVALUE`, `USERELATIONSHIP`, `CROSSFILTER`), financeiro (`PMT`, `PV`, `FV`, `NPER`, `NPV`, `IPMT`, `PPMT`, `SLN`, `SYD`, `DB`, `DDB`, `EFFECT`, `NOMINAL`, `RATE`), `PATH*` e Time Intelligence completo (`TOTALYTD`, `TOTALQTD`, `TOTALMTD`, `PARALLELPERIOD`, `DATESBETWEEN`, `DATESINPERIOD`, `FIRSTNONBLANK`/`LASTNONBLANK`, `OPENINGBALANCE*`/`CLOSINGBALANCE*`), além de 60+ DMVs `INFO.*` no motor TS. Veja a [Matriz de Compatibilidade](./docs/COMPATIBILITY.md).
- **Análise Interativa:** Cross-filtering bidirecional ao clicar em qualquer visual, Matriz com hierarquia expansível em 2 níveis, Drill-Through entre páginas, Cálculos Visuais e telemetria real no Performance Analyzer.

### 🗄️ Conectores Nativos de Banco de Dados (`v0.8.0`)
- **PostgreSQL, MySQL/MariaDB, SQLite, arquivos `.duckdb` e Parquet particionado (local / HTTP / S3)** via extensões do DuckDB (`ATTACH … READ_ONLY`), sem gateway externo. PostgreSQL 16 e MariaDB 11 testados contra servidores reais no CI.
- **Navegador de schemas e tabelas, pré-visualização e editor SQL** que roda dentro do banco conectado; tabelas carregadas de banco são recarregadas pelo **Atualizar Dados**.
- **Credenciais no chaveiro nativo do SO** (Secret Service no Linux, Credential Manager no Windows): nunca em arquivos `.insight`/`.pbip` nem no `localStorage` (a chave do Gemini também migra).

### 🔐 Segurança (`v0.8.0`)
- **Atualizações assinadas:** o atualizador só instala pacotes cujo `SHA256SUMS.txt` tenha assinatura Ed25519 válida da chave embutida no app; downloads restritos às releases deste repositório. Instalações `.deb`/`.rpm` recebem o pacote certo; AUR/Flatpak são orientados ao gerenciador de pacotes.
- **Content Security Policy** ativa no webview do Tauri.

### 🛠️ Produtividade, Tela Inicial & Distribuição
- **Tela Inicial (`WelcomeScreen`) & Auto-Updater:** escolha começar um projeto em branco ou abrir `.pbip`, `.pbix`, `.insight` e sessões recentes ao iniciar, com **atualizador integrado no app** via GitHub Releases e validação SHA-256 + assinatura Ed25519 (in-place para **AppImage**, download direto para `.deb`/`.rpm`, `msiexec /passive` para o **`.msi`** do Windows e download manual para o ZIP portátil do Windows).
- **GitHub Actions CI/CD:** builds automáticas para Linux (**AppImage**, **`.deb`**, **`.rpm`**) e Windows x64 (**`.msi`** + **`.zip`** portátil), publicadas juntas sob um único `SHA256SUMS.txt` assinado, além de manifestos em `packaging/` para **AUR (`vetrobi-bin`)** e **Flatpak**.
- **Edição Profissional no Canvas:** **Undo / Redo** (`Ctrl+Z`, `Ctrl+Y` e botões na Ribbon), **seleção múltipla** (`Shift`/`Ctrl` + clique, arraste em grupo, nudge com setas, `Delete` e barra flutuante de alinhamento/distribuição) e **Smart Snap Guides** a 60fps.
- **Visualização de Dados & Modelo:** inspeção tabular, gerenciador de relacionamentos com autodetecção e suporte a Tabelas Calendário.
- **Medidas Rápidas & Biblioteca UDF:** assistente de medidas rápidas, ordenação de visuais ("Classificar por", cabeçalhos de tabela/matriz, segmentações) e editor DAX com IntelliSense.
- **Governança & Temas:** simulação de RLS/OLS, parâmetros What-If, gerenciador de temas claros/escuros e suporte a i18n (`PT`, `EN`, `ES`) e moedas (`BRL`, `USD`, `EUR`).
- **Atalhos de Canvas:** zoom fluido com `Ctrl + =`, `Ctrl + -`, `Ctrl + 0` e `Ctrl + Scroll`, além de movimentação livre no canvas e modelo mantendo o botão do scroll pressionado e `Esc` para sair do Modo Apresentação.

---

## 🏗️ Arquitetura

| Camada | Tecnologia | Responsabilidade |
| :--- | :--- | :--- |
| **Desktop Shell** | Tauri v2 (GTK/WebKitGTK no Linux, WebView2 no Windows) | Janela nativa leve e diálogos de arquivos do sistema |
| **Core Backend** | Rust (`src-tauri/`) | Parsers `.pbip`/`.pbix`/TMDL/PBIR, carregador M e motor DAX nativo |
| **Motor Colunar / SQL** | DuckDB (`bundled` + `parquet` + `json`) | Leitura nativa de `.xlsx`/`.csv`/`.parquet`/`.json`, armazenamento e SQL |
| **Frontend & Canvas** | React 19 + TypeScript + Tailwind v4 + Zustand | Ribbon estilo Power BI, painéis de campos/filtros, canvas multi-página |
| **Visualização** | Apache ECharts 6 + Lucide | Gráficos interativos responsivos em tema claro e escuro |

---

## 🚀 Como Executar

### Pacotes prontos
Baixe o pacote mais recente direto das [GitHub Releases](https://github.com/ludoviciphilip/VetroBI/releases):
- **Linux:** **AppImage**, **`.deb`** ou **`.rpm`**.
- **Windows x64:** instalador **`.msi`** (recomendado — habilita o auto-updater integrado) ou **`.zip`** portátil (descompacte e rode `vetrobi.exe`, sem instalação/privilégios de administrador; nesse caso as atualizações precisam ser baixadas manualmente). Desde a `v0.10.0` o instalador é um único MSI em pt-BR (`VetroBI_<versão>_x64_pt-BR.msi`) — o `tauri-bundler`/WiX gera um instalador por idioma configurado, e o app tem seu próprio i18n (seletor de idioma dentro do app), então o idioma do instalador importa pouco; um único build mantém a release e o auto-updater determinísticos (ver [Lacunas Conhecidas](#lacunas-conhecidas)).

> **SmartScreen do Windows:** o binário ainda não é assinado digitalmente (ver [Lacunas Conhecidas](#lacunas-conhecidas) abaixo), então o Windows pode mostrar *"O Windows protegeu o computador"* na primeira execução; clique em **Mais informações → Executar assim mesmo**. Isso é esperado e não significa que o instalador seja inseguro — veja o `SHA256SUMS.txt` assinado publicado em cada release.

### Compilando a partir do código-fonte

#### Pré-requisitos (todas as plataformas)
- **Node.js** + **pnpm**
- **Rust** (`rustup` / `cargo`)

#### Linux
- Dependências de sistema do Tauri v2 (`webkit2gtk-4.1`, `libgtk-3-dev`, `librsvg2-dev`, etc. — consulte o [guia oficial do Tauri](https://v2.tauri.app/start/prerequisites/#linux)).

#### Windows
- **Microsoft Visual Studio C++ Build Tools** (workload "Desktop development with C++") e o **WebView2 Runtime** (já vem instalado no Windows 10/11 atuais; o instalador também embute um bootstrapper) — consulte o [guia oficial do Tauri](https://v2.tauri.app/start/prerequisites/#windows).
- Para gerar o `.msi` você também precisa do **WiX Toolset** v3, que o `tauri-bundler` baixa automaticamente no primeiro `pnpm tauri build --bundles msi`.

### Desenvolvimento

```bash
pnpm install
pnpm tauri dev
```

### Build de Produção

```bash
# Linux: AppImage / .deb / .rpm / binário
pnpm tauri build

# Windows: instalador MSI
pnpm tauri build --bundles msi
```

---

## 🐧 Dicas para GPU NVIDIA + Wayland no Linux

Por padrão, o VetroBI utiliza aceleração por hardware (DMA-BUF) do WebKitGTK para manter o canvas fluido. Caso seu ambiente utilize drivers proprietários NVIDIA no Wayland e apresente tela branca ou falha de sincronização ao abrir a janela nativa, tente iniciar com:

```bash
__NV_DISABLE_EXPLICIT_SYNC=1 pnpm tauri dev
```

Ou, em último caso, via XWayland ou renderização por software:

```bash
GDK_BACKEND=x11 pnpm tauri dev
# ou:
WEBKIT_DISABLE_DMABUF_RENDERER=1 pnpm tauri dev
```

**AppImage e WebKitGTK do sistema:** o AppImage usa automaticamente o `webkit2gtk-4.1` do sistema quando ele é compatível (canvas bem mais fluido que o embutido, mais antigo). Para forçar o embutido: `VETROBI_USE_BUNDLED_WEBKIT=1 ./VetroBI.AppImage`. No Arch, prefira o pacote AUR `vetrobi-bin`, que sempre usa o WebKitGTK do sistema.

---

## Lacunas Conhecidas

- **Assinatura de código:** os binários Windows ainda não têm certificado EV/OV, então o SmartScreen avisa na primeira execução (ver acima). A segurança do updater em si (assinatura Ed25519 do `SHA256SUMS.txt`, verificação SHA-256, allowlist de URL) é independente disso e não é afetada.
- Algumas coisas só uma pessoa com uma máquina Windows real pode validar: a experiência de instalação/upgrade do MSI, a fidelidade de renderização do WebView2, os diálogos nativos de arquivo e o prompt do Credential Manager do Windows no primeiro uso do chaveiro.

## 🗺️ Roadmap & Licença

- O VetroBI é um projeto independente, sem afiliação nem endosso da Microsoft. Power BI é marca registrada da Microsoft Corporation.
- Veja as próximas entregas (`v0.8.0` a `v1.0.0`) em [ROADMAP.md](./ROADMAP.md).
- Distribuído sob [The Mutualist License v1.3](./LICENSE.md) — originalmente inspirado na UI e engine DAX do [InsightHub](https://github.com/ludoviciphilip/Insight-Hub).
