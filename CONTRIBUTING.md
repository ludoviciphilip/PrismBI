# Contribuindo com o VetroBI

Obrigado pelo interesse em contribuir com o **VetroBI**!

## Ambiente de Desenvolvimento

### Pré-requisitos
- **Node.js 20+** e **pnpm 9+**
- **Rust** (`rustup` / `cargo` stable)
- Dependências Linux do **Tauri v2**:
  - **Arch Linux:** `sudo pacman -S webkit2gtk-4.1 gtk3 libayatana-appindicator librsvg base-devel`
  - **Ubuntu / Debian:** `sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev libayatana-appindicator3-dev librsvg2-dev patchelf`

### Comandos Úteis

```bash
# Instalar dependências JS
pnpm install

# Rodar app desktop em modo desenvolvimento
pnpm tauri dev

# Verificação de tipos, build web e suíte Vitest
pnpm exec tsc --noEmit && pnpm run build && pnpm test

# Testes unitários e checagem do backend Rust
cd src-tauri && cargo test --lib && cargo check --bins
```

## Diretrizes de Pull Request

1. **Testes obrigatórios:** garanta que `pnpm exec tsc --noEmit`, `pnpm test`, `cargo test --lib` e `cargo check --bins` passem sem warnings.
2. **Privacidade:** nunca inclua planilhas, arquivos `.pbix`/`.pbip` reais de terceiros ou dados sensíveis em commits ou fixtures de teste.
3. **Conformidade DAX:** ao adicionar funções em `src/engine/functions/` e `src-tauri/src/dax/`, adicione testes unitários correspondentes e atualize `src/engine/compliance/daxCatalog.generated.ts` (o teste `daxCatalogFreshness.test.ts` valida consistência).
4. **Licença:** o projeto é distribuído sob [The Mutualist License v1.3](./LICENSE.md).
