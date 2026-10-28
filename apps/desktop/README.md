# Valet desktop app

This folder holds the Valet desktop app: a Tauri 2 shell (`src-tauri/`, Rust) and a React 19 + TypeScript + Vite front end (`src/`).

```
pnpm install
pnpm tauri:dev    # run the app in development (runs `tauri dev`)
pnpm build        # build the front end into dist/
```

The app targets Windows. See the [root README](../../README.md) for the setup steps, the current status and the test commands.
