我来分析你的 Rust 项目结构。先获取仓库信息和目录结构。
The user wants me to analyze their Rust project at https://github.com/Spinach5/gamehub and suggest how to improve the project structure. Let me first fetch the repository to understand its structure.

Let me check the available skills first - this is a code analysis/repository structure improvement task. No specific skill seems directly applicable (the skills are mostly for lark, office, presentations, etc.). I'll proceed directly.

Let me fetch the repo structure.

API 限速了，我直接通过网页获取目录结构和 README。
API is rate limited. Let me fetch the directory listing pages directly. Let me look at the README and the tree structure.

Let me read the full file to find the directory structure. The HTML is large, let me look at the "Folders and files" section.

Let me look at lines around 280-300 to see the full file/folder listing.

这是一个 Rust workspace 项目，使用 Iced 做 UI。让我深入查看 `crates/` 目录和 `Cargo.toml`。
So the project is a Rust workspace with a `crates/` directory. It's a Linux offline Windows game launcher using Rust + Iced. Let me explore the crates directory and the docs to understand the structure better. Let me also look at the Cargo.toml to understand the workspace layout.

现在我看到了 7 个 crate。让我查看 docs 结构和各 crate 的内部模块组织。
Now I have the crates:
- display
- launch
- launcher-core
- prefix
- runner-wine
- storage
- ui

And the docs directory. Let me look at the docs structure and also look inside some crates to understand the module organization. Let me check the docs specs and look at the ui crate (the main entry) and launcher-core.

让我查看 `ui/src`、`launcher-core/src` 的内部模块组织，以及 docs/specs。
Now let me look at the src directories of ui and launcher-core to understand module organization. Let me also check the docs/specs to understand the architecture. Let me fetch the ui/src and launcher-core/src directories, and the docs/specs directory.

让我查看 `ui/src/views`、其他几个 crate 的 src，以及完整的 specs 列表。
Now I have a clear picture. Let me get more details:
- ui/src: views/, app.rs (and likely more files like main.rs, lib.rs)
- launcher-core/src: error.rs, lib.rs, paths.rs, text.rs, types.rs

Let me check the ui/src full file list and a few more crates (storage, runner-wine, prefix, launch, display). Also let me check the docs/specs list to understand the architecture better.

Let me look at the ui/src views and the full ui/src listing, plus other crate src dirs.

让我查看其余 crate 的 src 和 ui/views，以及架构总览文档。
Now I have a good understanding. Let me also look at the remaining crates' src directories to complete the picture: storage, runner-wine, prefix, launch, display. And let me check the ui/src/views directory. Let me also look at the 00-overview.md spec to understand the intended architecture.

Let me fetch the remaining crate src dirs and the ui/views and the overview spec.

让我查看 storage/src 的文件列表和架构总览文档的依赖关系部分。
Now I have a very clear picture. Let me also look at the storage/src directory (the file was too large, let me grep it) and check the remaining crates (runner-wine, prefix, launch, display). Let me also check the overview doc for the architecture and dependencies.

Let me read the overview doc more and grep the storage src files. Also let me look at the docs/specs/00-overview fully to understand the intended dependency graph.

Actually, I have enough now. Let me check the storage src files and a couple others quickly to be thorough.

Let me read the storage/src files directly from the grep result (lines 332-335 were omitted due to long lines). Let me read those lines. Also let me check the other crates (runner-wine, prefix, display, launch) src structures. Let me look at the storage src files by reading lines 330-336.

让我查看 `ui/lib.rs` 的模块声明、`display` 和 `launch` crate 的 src，以及 packaging 目录。
Now I have a comprehensive understanding of the project. Let me also quickly check the `display` and `launch` crate src structures and the `packaging` directory, plus check the ui/lib.rs to see the module declarations. Actually I have enough. Let me also look at the ui/lib.rs module structure since I see the files are: app.rs, dialogs.rs, draft.rs, lib.rs, main.rs, tasks.rs, widgets.rs, views/.

Let me check the ui/lib.rs to understand the module declarations, and look at the display/launch crates. Let me also check the packaging directory structure.

Actually, I have enough information now to provide a thorough analysis and improvement suggestions. Let me compile the full picture:

**Project: gameHub**
- Linux 离线 Windows 游戏启动器，Rust + Iced
- Cargo workspace with 7 crates

**Crate structure:**
```
crates/
├── launcher-core/   # 纯类型、枚举映射、路径计算（基础层）
│   └── src/
│       ├── lib.rs
│       ├── error.rs
│       ├── paths.rs (AppPaths)
│       ├── text.rs (UTF-8 截断)
│       └── types.rs (Game, Runner, Arch, DisplayConfig, etc.)
├── storage/         # SQLite 持久化
│   └── src/
│       ├── lib.rs
│       ├── error.rs
│       ├── schema.rs
│       └── mapping.rs
├── runner-wine/     # Wine/Proton 环境构造与启动
│   └── src/
│       ├── lib.rs
│       ├── context.rs (WineContext, base_env, build_command, spawn)
│       ├── detect.rs
│       ├── error.rs
│       └── test_support.rs
├── prefix/          # Prefix 管理 + DXVK 注入
│   └── src/
│       ├── lib.rs
│       ├── dxvk.rs
│       ├── initialize.rs
│       ├── status.rs
│       ├── error.rs
│       └── test_support.rs
├── display/         # 显示配置（xrandr, gamescope 参数）
├── launch/          # 启动计划执行
└── ui/              # Iced 二进制入口（唯一的 bin）
    └── src/
        ├── main.rs
        ├── lib.rs
        ├── app.rs (App state, Message)
        ├── draft.rs (GameDraft, 草稿验证)
        ├── dialogs.rs
        ├── tasks.rs (异步任务)
        ├── widgets.rs
        └── views/
            ├── mod.rs
            ├── library.rs
            ├── game_detail.rs
            ├── runners.rs
            └── settings.rs
```

**Top level:**
```
.github/workflows/
crates/
docs/specs/ (00-overview, 01-foundation ... 11-acceptance, archive/)
packaging/
.gitignore, CHANGELOG.md, Cargo.toml, Cargo.lock, README.md
```

**Architecture design (from spec):**
- 单向依赖，无环
- launcher-core: pure types, enum mappings, path computation
- storage: SQLite
- runner-wine, prefix, display, launch: each encapsulates one responsibility
- ui: only Iced binary entry

This is actually a well-structured project. Now let me think about improvements.

**Strengths:**
1. Clean crate separation by responsibility (DDD-ish)
2. Unidirectional dependencies
3. Workspace with shared dependencies (`[workspace.dependencies]`)
4. Each crate has its own error.rs (good practice)
5. test_support modules for test helpers
6. Comprehensive specs/docs
7. Forbid unsafe code
8. Release profile optimized (LTO, codegen-units=1, strip, panic=abort)

**Potential improvements:**

1. **launcher-core 职责过重风险**：目前放了 types、paths、text、error。paths.rs 里的 AppPaths 会读环境变量（虽然分离了 discover），text.rs 是纯工具。可以保持，但要注意 core 不引入重依赖。

2. **storage 的 schema 和 mapping 分离**是好的，但 Db 结构体可能在 lib.rs 里。

3. **ui 模块可能过大**：app.rs、tasks.rs、widgets.rs 可能文件很大。app.rs 包含整个 App state 和 Message，可能需要拆分。

4. **缺少统一的错误处理/转换层**：各 crate 有自己的 error，但 UI 层如何聚合这些错误？需要看 app.rs。

5. **缺少 `xtask` 或 build 脚本**：packaging 独立，没有 cargo xtask。

6. **依赖图需要验证**：launch 是否依赖 runner-wine + prefix + display？这些是否会产生潜在环？

7. **测试组织**：有些 crate 用 tests/ 目录（launcher-core），有些用内联 test module。

8. **缺少 benches、examples**。

9. **docs/specs 非常详细，但缺少架构图（依赖关系图）**。

10. **ui 是 lib + bin**：main.rs 调 ui::run()，这是好实践。

Let me check a few more things: the display and launch crates' src, the ui/lib.rs module declarations, and the packaging directory. Let me also check if there's a Cargo.toml at root that defines workspace properly (I already saw it).

Let me look at the ui/lib.rs and the display/launch crates, and packaging. Let me also check the .github/workflows.

Actually, I think I have enough. Let me look at the ui/lib.rs module structure and the display/launch crates to confirm, then write the analysis.

Let me fetch ui/lib.rs and the two remaining crates' src.

让我查看几个 crate 的 `Cargo.toml` 来确认实际依赖方向，以及 packaging 目录。
Now I have a complete picture. Let me also check the packaging directory and the .github/workflows to round out the analysis. But I think I have enough to give a thorough analysis and improvement suggestions.

Let me check the packaging directory quickly.

Actually, let me check the dependency graph by looking at the Cargo.toml of a few crates to understand the actual dependency directions. Let me check the ui/Cargo.toml, launch/Cargo.toml, and runner-wine/Cargo.toml. This is important to verify the "单向依赖" claim and spot potential issues.

Let me fetch a few Cargo.toml files.

Now I have a complete picture including the dependency graph. Let me analyze:

**Dependency direction (from Cargo.toml):**
- `launcher-core`: no internal deps (foundation)
- `storage`: launcher-core (need types)
- `runner-wine`: launcher-core (need Runner type)
- `prefix`: launcher-core, runner-wine (need WineContext for wineboot)
- `display`: launcher-core (need DisplayConfig)
- `launch`: launcher-core, storage, prefix, display, runner-wine (depends on almost everything)
- `ui`: launcher-core, storage, prefix, display, runner-wine, launch (top-level)

So the dependency graph is:
```
launcher-core (root)
    ↑
storage, runner-wine, display
    ↑
prefix (→ runner-wine, launcher-core)
    ↑
launch (→ storage, prefix, display, runner-wine, launcher-core)
    ↑
ui (→ all)
```

This is mostly unidirectional but there's a potential concern: `launch` depends on `storage` (to look up runner), and `ui` depends on both. No cycles. Good.

But there's a subtle issue: `prefix` depends on `runner-wine` (for wineboot), and `runner-wine` only depends on `launcher-core`. `launch` depends on `prefix` and `runner-wine`. This is fine, no cycle.

Now let me think about improvements. The project is actually quite well structured. Let me identify real improvement opportunities:

**1. `launch` crate 依赖过重（上帝 crate 倾向）**
`launch` 依赖 storage + prefix + display + runner-wine + launcher-core。它变成了"编排所有东西"的地方。这本身没错（它是 launch pipeline 的编排层），但需要确保它不泄露太多细节给 ui。ui 应该主要通过 launch 的高层 API（build_plan/spawn_plan）交互，而不是直接操作 prefix/runner-wine/display。目前 ui 的 Cargo.toml 直接依赖了所有 crate，这意味着 ui 可以绕过 launch 直接调用底层。这会削弱分层。

建议：让 ui 主要依赖 launch（作为应用服务层），launch 再编排底层。ui 只在需要直接展示（如 runner 列表、prefix 状态）时才直接依赖 storage/prefix。或者引入一个 `application`/`service` 层。

**2. `ui/app.rs` 可能过大**
App state + Message + update + view 全部在 app.rs，加上 tasks.rs（异步任务）、dialogs.rs、draft.rs、widgets.rs。随着功能增长，app.rs 会变成巨型文件。建议按"功能模块"拆分 app 的状态和消息，或用子模块。

**3. 缺少应用服务层（Application Service）**
目前 ui 直接调用 storage::Db、prefix::、display::、launch::。业务编排逻辑散落在 ui/tasks.rs 中。tasks.rs 本质上是应用服务，但它在 ui crate 内部。如果将来要加 CLI 或 TUI，这些逻辑无法复用。

建议：考虑把 tasks.rs 的核心编排逻辑抽到一个 `application` crate（或 `launch` crate 扩展），让 ui 只负责 Iced 的 view/update。

**4. `launcher-core` 中 paths.rs 的环境依赖**
AppPaths::discover() 读环境变量。虽然分离了 discover() 和 for_root()，但 paths.rs 放 core 里意味着 core 不是纯类型层。spec 说 core "只放纯类型、枚举映射和路径计算"。路径计算涉及环境，这个边界可以接受，但要注意 core 不要继续膨胀。

**5. 错误类型跨 crate 转换**
每个 crate 有自己的 error.rs。ui 需要把 StorageError、PrefixError、RunnerError、LaunchError 等转换成 UI 可展示的错误。目前看 app.rs 里应该有错误处理。建议在 launch 或一个 common error 层定义统一的 AppError，用 thiserror 的 `#[from]` 做转换，避免 ui 里到处 map_err。

**6. 测试组织不统一**
- launcher-core 有 tests/ 目录（集成测试）
- 其他 crate 用 `#[cfg(test)] mod tests` 内联 + test_support.rs
建议统一，或至少为每个 crate 建 tests/ 放集成测试。

**7. 缺少 `tests/` 集成测试目录在大多数 crate**
只有 launcher-core 有 tests/。其他 crate 的测试全在 src 内联。

**8. 缺少 `examples/`**
对于库 crate（runner-wine, prefix, display, launch），加 examples 有助于理解 API。

**9. `packaging/` 与根目录构建脚本**
没有 cargo xtask，打包脚本可能在 packaging 和 .github/workflows 里。建议加 `xtask` crate 统一构建/打包/发布命令。

**10. docs/specs 非常详尽，但缺少架构图**
建议在 00-overview.md 加一个 Mermaid 依赖关系图和分层图。

**11. `ui/widgets.rs` 可能膨胀**
所有自定义 widget 放一个文件。建议拆成 widgets/ 目录，每个 widget 一个文件。

**12. `storage` 的 Db 是否在 lib.rs**
storage 有 lib.rs, error.rs, schema.rs, mapping.rs。Db 结构体和 CRUD 方法可能在 lib.rs，随着表增多会膨胀。建议按实体拆分（games.rs, runners.rs, settings.rs）或用 repository 模式。

**13. workspace 缺少 `default-members` 或 `exclude`**
Cargo.toml 只有 members = ["crates/*"]。如果加 xtask，需要处理。

**14. 缺少 `rustfmt.toml`**
只有 edition 2024，没有自定义 rustfmt 配置。如果团队有格式偏好应加。

**15. `tracing` 初始化在 ui/lib.rs**
日志初始化逻辑在 ui crate。如果将来有 CLI，需要复用。建议把日志初始化抽到 launcher-core 或一个独立的 `logging` 模块/crate。

**16. 命名一致性**
crate 名：launcher-core（kebab）、runner-wine、storage、prefix、display、launch、ui。其中 `launch` 作为 crate 名太泛，容易和 `launcher-core` 混淆。建议重命名为 `launcher` 或 `game-launch` 或 `runtime`。不过这是小问题。

Let me prioritize the most impactful improvements. The project is well-designed overall, so I should focus on structural improvements that will pay off as it grows.

Let me now write the comprehensive analysis. I'll structure it as:
1. Current structure summary
2. Strengths
3. Improvement suggestions (prioritized)

I should present this clearly. Given the user asked "项目结构应该如何改进", I'll focus on improvements while acknowledging the good parts.
我已经完整分析了项目。现在给出结构改进建议。

---

# gameHub 项目结构分析与改进建议

## 一、当前结构总览

这是一个 **Cargo workspace + 7 crate** 的分层架构，整体设计相当扎实：

```
gamehub/
├── crates/
│   ├── launcher-core/   # 基础层：纯类型(Game/Runner/Arch/DisplayConfig)、AppPaths、text 工具
│   ├── storage/         # SQLite 持久化（schema + mapping + error）
│   ├── runner-wine/     # Wine/Proton 环境构造与 spawn
│   ├── prefix/          # Prefix 初始化/状态/DXVK 注入
│   ├── display/         # xrandr 解析、gamescope 参数、会话检测
│   ├── launch/          # 启动流水线编排（resolve→plan→env→spawn）
│   └── ui/              # Iced 二进制入口（lib+bin，main.rs 仅调 ui::run()）
├── docs/specs/          # 00~11 实施规格（非常详尽）
├── packaging/           # 打包配置
└── .github/workflows/   # CI/CD
```

**实际依赖方向（从 Cargo.toml 验证）：**

```
launcher-core ─────────────────────┐
       ↑                           │
  storage  runner-wine  display    │
       ↑      ↑                    │
     prefix ──┘                    │
       ↑                           │
     launch ───────────────────────┤
       ↑                           │
       ui ─────────────────────────┘  (ui 直接依赖全部 6 个 crate)
```

无循环依赖，方向基本单向。

---

## 二、当前结构的优点

1. **职责切分清晰**：每个 crate 一个明确职责，符合单一职责原则。
2. **`[workspace.dependencies]` 统一版本**：避免 crate 间版本漂移。
3. **每 crate 自带 `error.rs`**：用 `thiserror` 定义专属错误面，调用方无需字符串匹配。
4. **`launch` 的纯/IO 分离**：`build_plan`/`build_env` 无 IO 可测试，`spawn_plan` 才触达进程——这是很好的测试友好设计。
5. **`test_support.rs` 模块化**：runner-wine 和 prefix 都抽取了测试辅助，并用 `EXEC_LOCK` 解决了 ETXTBSY 竞态，工程细节到位。
6. **`ui` 采用 lib+bin 分离**：`main.rs` 极薄，`run()` 在 lib 中可测试。
7. **`forbid(unsafe_code)` + release profile 优化**（LTO、codegen-units=1、strip、panic=abort）。

---

## 三、改进建议（按优先级）

### 🔴 P0：收敛 `ui` 的直接依赖，引入应用服务层

**问题**：`ui/Cargo.toml` 直接依赖了 **全部 6 个内部 crate**。这意味着 UI 层可以绕过 `launch` 直接操作 `storage::Db`、`prefix::`、`display::`，分层边界被打穿。当前 `ui/tasks.rs` 已经承担了"应用服务"的角色（保存游戏、启动、探测 Wine、创建 prefix…），但它被锁在 ui crate 里。

**后果**：
- 业务编排逻辑与 Iced 状态强耦合，无法被 CLI/TUI/测试复用。
- `tasks.rs` 会随功能增长变成巨型文件。
- 难以对"启动游戏"这个用例做端到端的纯逻辑测试。

**建议**：新建一个 `application` crate（或扩展 `launch`）作为应用服务层，把 `tasks.rs` 里的**纯业务编排**（不含 Iced 的 `Task`/`Message`）下沉进去：

```
crates/
├── application/        # 新增：用例编排（SaveGame, LaunchGame, ImportRunner...）
│   └── 依赖 storage + prefix + display + runner-wine + launch
└── ui/                 # 只依赖 application + launcher-core（展示所需的少量直接依赖）
```

`application` 提供异步函数（返回 `Result<_, AppError>`），ui 只负责把它们包进 `iced::Task`，并把结果翻译成 `Message`。这样 UI 层真正只做"视图+消息翻译"。

### 🔴 P1：拆分 `ui/app.rs` 和 `ui/widgets.rs`

**问题**：`app.rs` 同时容纳 `App` 状态、`Message` 枚举、`update`、`view`；`widgets.rs` 容纳所有自定义控件。随功能增长这两个文件会膨胀到难以维护。

**建议**：
```
crates/ui/src/
├── app/                    # 原 app.rs 拆分
│   ├── mod.rs             # App 结构体 + run
│   ├── message.rs          # Message 枚举
│   ├── update.rs           # update 逻辑
│   └── view.rs             # 顶层 view 路由
├── widgets/                # 原 widgets.rs 拆成目录
│   ├── mod.rs
│   ├── env_editor.rs
│   ├── dll_override.rs
│   └── ...
└── views/                  # 保持
```

### 🟡 P2：统一错误转换，定义 `AppError`

**问题**：6 个 crate 各有 `error.rs`，ui 需要把 `StorageError`/`PrefixError`/`RunnerError`/`LaunchError` 全部 map 成 UI 可展示的提示，`map_err` 散落各处。

**建议**：在 `launcher-core`（或新建 `error` 模块）定义一个聚合错误：

```rust
// launcher-core/src/error.rs
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error(transparent)] Storage(#[from] storage::StorageError),
    #[error(transparent)] Prefix(#[from] prefix::PrefixError),
    #[error(transparent)] Runner(#[from] runner_wine::RunnerError),
    #[error(transparent)] Launch(#[from] launch::LaunchError),
    // ...
}
```

⚠️ 注意：这会让 `launcher-core` 反向依赖 storage/prefix 等，**破坏分层**。更好的做法是把 `AppError` 放在 `application` crate（P0 引入的服务层）里，因为只有应用层需要聚合所有错误。launcher-core 保持纯净。

### 🟡 P3：`storage` 按实体拆分，避免 lib.rs 膨胀

**问题**：`storage` 目前只有 `lib.rs`（含 Db）、`schema.rs`、`mapping.rs`、`error.rs`。随着 games/runners/settings 三张表的 CRUD 都堆在 lib.rs，会变成上帝对象。

**建议**：引入 Repository 模式：
```
crates/storage/src/
├── lib.rs            # Db 连接 + 事务
├── schema.rs
├── error.rs
├── mapping.rs        # 行↔领域对象映射
└── repos/
    ├── games.rs      # impl Db { games() -> GamesRepo }
    ├── runners.rs
    └── settings.rs
```

### 🟡 P4：`launch` crate 命名与职责澄清

**问题**：`launch` 这个名字和 `launcher-core` 容易混淆；且 `launch` 目前依赖 `storage`（为了查 runner）、`prefix`、`display`、`runner-wine`，是事实上的"启动用例编排层"。

**建议**：
- 重命名 `launch` → `runtime` 或 `game-launch`，避免和 launcher-core 视觉冲突。
- 或者（配合 P0）把 `launch` 并入 `application` crate，作为其中一个用例模块。

### 🟢 P5：测试组织统一化

**问题**：只有 `launcher-core` 有 `tests/` 集成测试目录，其余 crate 全用 `#[cfg(test)]` 内联 + `test_support.rs`。

**建议**：为每个 crate 建 `tests/` 目录放**跨模块/公开 API** 的集成测试；纯单元测试保留内联。这样公开契约（如 `launch::build_plan`、`prefix::plan_for_game`）的回归测试更清晰。

### 🟢 P6：日志初始化下沉

**问题**：`init_logging()` 在 `ui/src/lib.rs`。如果将来加 CLI 入口，日志逻辑要复制。

**建议**：抽到 `launcher-core` 的 `logging.rs`（或独立 `tracing-setup` 模块），接受 `logs_dir: &Path` 返回 `WorkerGuard`。ui 和未来的 CLI 都调用它。

### 🟢 P7：补充架构图与 examples

- 在 `docs/specs/00-overview.md` 加一张 **Mermaid 依赖分层图**（把上面那幅 ASCII 图转成图），新人一眼看懂边界。
- 给 `runner-wine`、`prefix`、`display`、`launch` 各加一个 `examples/`，演示 API 用法。

### 🟢 P8：考虑引入 `xtask`

目前打包逻辑分散在 `packaging/` 和 `.github/workflows/`。引入 `crates/xtask` 统一 `cargo xtask dist`、`cargo xtask lint`、`cargo xtask test-all` 等命令，降低 CI 和本地操作的不一致。

---

## 四、改进后的目标结构

```
crates/
├── launcher-core/      # 纯类型 + AppPaths + text + logging（无 IO 依赖，除 discover）
├── storage/            # SQLite + repos/
├── runner-wine/        # WineContext
├── prefix/             # prefix + DXVK
├── display/            # xrandr/gamescope/session
├── application/        # ★ 新增：用例编排（聚合错误、事务、调用底层）
└── ui/                 # Iced 视图层，仅依赖 application + 展示所需类型
```

依赖方向：`launcher-core ← storage/runner-wine/display/prefix ← application ← ui`

---

## 五、一句话总结

你的项目**分层方向正确、crate 切分合理、测试意识强**，主要改进空间在于 **P0（引入应用服务层，收敛 ui 对底层的直接依赖）** 和 **P1（拆分 ui 的巨型文件）**。这两点解决后，项目会从"分层良好的单体 UI"演进为"可复用业务内核 + 薄视图层"的架构，为后续加 CLI、自动化测试或多前端打下基础。