# Check matrix

| Changed area | Required additional checks |
|---|---|
| Any source or config | Base quality gate |
| `package.json`, lockfiles, `Cargo.toml`, `Cargo.lock` | Dependency security audit |
| `providers/`, storage, database, security, permissions, updater | Dependency audit plus relevant security tests; manual credential and network review where applicable |
| React components, styles, themes, layout | Rendered UI regression at 460x720 in light and dark modes |
| Window mode, tray, Tauri commands | Real Tauri desktop smoke test; verify Full, Mini, Ball, tray, close-to-tray, and restart persistence |
| Tauri config, capabilities, icons, platform code | Target-platform `pnpm tauri build` |
| Release/version/updater | Base gate, dependency audit, signed package verification, clean-machine install, and upgrade test |

The base gate consists of environment/version checks, Git whitespace checks, tracked secret-file detection, `pnpm check`, and `pnpm build`.
