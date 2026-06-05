# SupaZip — Implementation Plan

## Stack

| Компонент | Crate | Версия | Обоснование |
|-----------|-------|--------|-------------|
| **7z** | `sevenz-rust` | 0.6.1 | Pure Rust, AES encryption, streaming |
| **ZIP** | `zip` | 8.4.0 | 154M+ downloads, AES, deflate/lzma/zstd |
| **GUI** | `egui` + `eframe` | 0.34.1 | Immediate mode, best performance, cross-platform |
| **File dialogs** | `rfd` | 0.17.2 | Native dialogs Windows/macOS/Linux |
| **Async** | `tokio` | 1.50.0 | Non-blocking archive operations |
| **CLI parser** | `clap` | — | Стандарт для Rust CLI |
| **Errors** | `thiserror` | — | Типобезопасные ошибки |

---

## Project Structure (Workspace)

```
supazip/
├── Cargo.toml              # Workspace root
├── SPEC.md                 # Detailed specification
├── supazip-core/           # Core engine (no GUI dependency)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── error.rs        # ArchiverError enum
│       ├── traits.rs       # ArchiveFormat trait
│       ├── formats/
│       │   ├── mod.rs
│       │   ├── sevenz.rs   # 7z implementation
│       │   └── zip.rs      # ZIP implementation
│       ├── operations/
│       │   ├── mod.rs
│       │   ├── list.rs
│       │   ├── extract.rs
│       │   ├── create.rs
│       │   └── test.rs
│       └── progress.rs     # ProgressCallback trait
├── supazip-gui/            # eframe GUI application
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── app.rs          # AppState, AppMode
│       ├── ui/
│       │   ├── mod.rs
│       │   ├── toolbar.rs
│       │   ├── tabs.rs
│       │   ├── addressbar.rs
│       │   ├── filelist.rs
│       │   ├── statusbar.rs
│       │   ├── progress.rs
│       │   └── password.rs
│       └── commands.rs     # IPC to backend
└── supazip-cli/            # CLI interface (optional, same binary)
    └── Cargo.toml
```

**Ключевое решение:** `supazip-core` — отдельный crate без GUI. Позволяет:
- GUI и CLI используют одну и ту же библиотеку
- Core можно тестировать отдельно
- Нет циклических зависимостей

---

## Core Traits

### ArchiveFormat trait

```rust
pub trait ArchiveFormat: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &[&str];

    fn list<R: Read>(&self, reader: R, password: Option<&str>)
        -> Result<Vec<ArchiveEntry>, ArchiverError>;

    fn extract<R: Read, W: Write + Seek>(
        &self, reader: R, dest: &mut W, entries: &[&str],
        password: Option<&str>, progress: &dyn ProgressCallback
    ) -> Result<(), ArchiverError>;

    fn create<W: Write + Seek>(
        &self, writer: W, entries: &[PathBuf],
        options: &CreateOptions, password: Option<&str>,
        progress: &dyn ProgressCallback
    ) -> Result<(), ArchiverError>;

    fn test<R: Read>(
        &self, reader: R, password: Option<&str>,
        progress: &dyn ProgressCallback
    ) -> Result<bool, ArchiverError>;
}

pub trait ProgressCallback: Send {
    fn set_progress(&self, current: u64, total: u64);
    fn set_message(&self, message: &str);
    fn is_cancelled(&self) -> bool;
}
```

### ArchiveEntry

```rust
pub struct ArchiveEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub compressed_size: u64,
    pub modified: Option<DateTime>,
    pub compression_method: String,
    pub crc32: Option<u32>,
    pub encrypted: bool,
}
```

---

## GUI Layout (PeaZip-style)

```
┌─────────────────────────────────────────────────────┐
│ Toolbar: [Add] [Extract] [Test] [Convert]  │ [▼]  │
├─────────────────────────────────────────────────────┤
│ Tabs: [Archive] [Browser] [Jobs]                   │
├─────────────────────────────────────────────────────┤
│ Address: /path/to/archive.7z         [▼] [..]     │
├─────────────────────────────────────────────────────┤
│ Name │ Type │ Size │ Compressed │ Modified │ Method │
│─────────────────────────────────────────────────────│
│ file1.txt │ txt │ 1KB │ 0.5KB │ ... │ Deflate  │
│ folder/   │ dir │  -  │   -  │ ... │   -     │
├─────────────────────────────────────────────────────┤
│ Status: 5 items, 1.5GB, 60% ratio    │ Progress:  │
└─────────────────────────────────────────────────────┘
```

### UI Components

| Component | Egui Widget | PeaZip Equivalent |
|-----------|-------------|------------------|
| Toolbar | `TopBottomPanel::top()` + `Button` | `PanelToolBar` |
| Tabs | `Ui::horizontal()` + click detection | `PageControl` |
| Address Bar | `TextEdit` + dropdown | `PanelAddress` |
| File List | Custom table with scrolling | `StringGridArchive` |
| Status Bar | `TopBottomPanel::bottom()` | `StatusBar` |
| Progress | Modal overlay with `ProgressBar` | `PanelProgressAdd` |

---

## Implementation Order

### Phase 1: Core Engine (Weeks 1-2)

1. **Project Setup**
   - Create workspace structure
   - `supazip-core` с зависимостями
   - Error types

2. **ZIP Backend** (`supazip-core/src/formats/zip.rs`)
   - list, extract, create через `zip` crate
   - Streaming operations
   - Progress callback

3. **7z Backend** (`supazip-core/src/formats/sevenz.rs`)
   - list, extract, create через `sevenz-rust`
   - AES-256 decryption
   - Progress callback

4. **Unit tests**
   - ZIP round-trip
   - 7z round-trip
   - Password-protected archives

### Phase 2: CLI (Week 3)

```bash
supazip list <archive>       # List contents
supazip extract <archive> [files]  # Extract
supazip create <archive> [files]  # Create
supazip test <archive>        # Test integrity
```

### Phase 3: GUI (Weeks 4-6)

1. **Basic Window** — eframe setup, `rfd` dialogs, themes
2. **Archive Browser** — file list table, sorting, selection
3. **Toolbar Actions** — Add, Extract, Test, Convert
4. **Progress Dialog** — cancelable, speed/ETA display
5. **Password Dialog** — modal for encrypted archives

### Phase 4: Polish (Week 7)

- Virtual scrolling для больших архивов
- Drag & drop
- Recent archives
- Keyboard shortcuts

---

## Risks

| Risk | Mitigation |
|------|------------|
| `rfd` + egui integration | Test early, use async dialogs |
| Large archive (100K+ entries) | Virtual scrolling, lazy loading |
| 7z AES support in `sevenz-rust` | Test encrypted 7z early |
| PeaZip UI complexity | Focus on core operations first |

---

## Performance

- **Streaming** для list/extract (не грузим весь архив в память)
- **Buffered** для create (нужны headers, checksums)
- **64KB buffer** для file I/O
- **Tokio** для async, `spawn_blocking` для CPU-bound compression
