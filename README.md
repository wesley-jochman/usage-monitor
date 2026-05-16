# Codex Usage Monitor

Tauri v2 desktop app (TypeScript + React + Vite) for monitoring Codex usage with:

- RPC-first source: `codex app-server` JSON-RPC
- Fallback source: `~/.codex/sessions` JSONL parsing
- Tray/menu support for macOS and Windows
- Tailwind + shadcn/ui + motion/react UI
- Biome + Husky pre-commit checks

## Development

```bash
pnpm install
pnpm dev
```

## Build

```bash
pnpm build
pnpm tauri:build
```
