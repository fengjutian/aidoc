# AIDoc Desktop (Tauri 2 + React)

The Tauri application lives in `apps/desktop`. The React/Vite frontend is in
`ui`, and the Rust backend is in `src-tauri`.

## Prerequisites

- Node.js 20 or newer and npm
- Rust stable with the Windows MSVC toolchain
- Visual Studio C++ Build Tools and WebView2 Runtime on Windows

## Development

From `apps/desktop`:

```sh
npm install
npm --prefix ui install
npm run tauri dev
```

The Tauri CLI runs `npm run dev` automatically before starting the Rust app.
The first Rust build may take several minutes. To run only the frontend in a
browser, use `npm run dev` from `apps/desktop`.

## Build

```sh
npm run tauri build
```

`npm run build` builds only the frontend. Tauri bundles are written under the
workspace `target/release/bundle` directory.

The older `run.ps1` helper and `cargo tauri` commands remain available for
developers who use pnpm and the Cargo Tauri CLI.
