use super::{App, Message};
use application::editor::DllRow;
use application::{
    DllCandidate, DllScanError, DllScanOutcome, OverrideMode, dll_name_key, scan_dlls,
};
use iced::Task;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

/// Bounds the number of rows laid out at once; iced has no virtualization.
pub const DLL_SCAN_RENDER_LIMIT: usize = 300;

#[derive(Debug)]
pub struct DllScanDialog {
    pub game_id: Uuid,
    pub root: Option<PathBuf>,
    pub phase: DllScanPhase,
    pub query: String,
    pub sort_desc: bool,
    pub selected: HashSet<String>,
    pub mode: OverrideMode,
    pub existing: HashSet<String>,
    pub scan_seq: u64,
    pub cancel: Arc<AtomicBool>,
    pub applying: bool,
    pub apply_error: Option<String>,
    pub pending_apply: Option<PendingApply>,
}

#[derive(Debug)]
pub enum DllScanPhase {
    NeedDirectory,
    Scanning,
    Ready(DllScanOutcome),
    Failed(DllScanError),
}

#[derive(Debug, Clone)]
pub struct PendingApply {
    pub original_rows: Vec<DllRow>,
    pub added: Vec<(String, OverrideMode)>,
    pub revision: u64,
}

pub fn existing_keys(rows: &[DllRow]) -> HashSet<String> {
    rows.iter()
        .map(|row| dll_name_key(&row.name))
        .filter(|key| !key.is_empty())
        .collect()
}

impl DllScanDialog {
    pub fn new(game_id: Uuid, root: Option<PathBuf>, existing: HashSet<String>) -> Self {
        let phase = if root.is_some() {
            DllScanPhase::Scanning
        } else {
            DllScanPhase::NeedDirectory
        };
        Self {
            game_id,
            root,
            phase,
            query: String::new(),
            sort_desc: false,
            selected: HashSet::new(),
            mode: OverrideMode::NativeBuiltin,
            existing,
            scan_seq: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            applying: false,
            apply_error: None,
            pending_apply: None,
        }
    }

    /// Cancels the previous scan and starts a new generation.
    pub fn next_scan(&mut self) -> Option<(u64, PathBuf, Arc<AtomicBool>)> {
        self.cancel.store(true, Ordering::Relaxed);
        let root = self.root.clone()?;
        self.scan_seq = self.scan_seq.wrapping_add(1);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.phase = DllScanPhase::Scanning;
        self.selected.clear();
        self.apply_error = None;
        Some((self.scan_seq, root, Arc::clone(&self.cancel)))
    }

    pub fn cancel_scan(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Filtered, sorted and capped rows used by both the list and the
    /// select-all/invert operations.
    pub fn filtered(&self) -> Vec<&DllCandidate> {
        let DllScanPhase::Ready(outcome) = &self.phase else {
            return Vec::new();
        };
        let needle = self.query.trim().to_ascii_lowercase();
        let mut rows: Vec<&DllCandidate> = outcome
            .entries
            .iter()
            .filter(|entry| needle.is_empty() || entry.name.to_ascii_lowercase().contains(&needle))
            .collect();
        rows.sort_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then_with(|| a.relative_path.cmp(&b.relative_path))
        });
        if self.sort_desc {
            rows.reverse();
        }
        rows.truncate(DLL_SCAN_RENDER_LIMIT);
        rows
    }

    pub fn toggle(&mut self, key: String) {
        if self.existing.contains(&key) {
            return;
        }
        if !self.selected.remove(&key) {
            self.selected.insert(key);
        }
    }

    pub fn select_all_filtered(&mut self) {
        let keys: Vec<String> = self
            .filtered()
            .iter()
            .filter(|entry| !self.existing.contains(&entry.key))
            .map(|entry| entry.key.clone())
            .collect();
        self.selected.extend(keys);
    }

    pub fn invert_filtered(&mut self) {
        let keys: Vec<String> = self
            .filtered()
            .iter()
            .filter(|entry| !self.existing.contains(&entry.key))
            .map(|entry| entry.key.clone())
            .collect();
        for key in keys {
            if !self.selected.remove(&key) {
                self.selected.insert(key);
            }
        }
    }
}

impl App {
    pub(crate) fn open_dll_scan(&mut self) -> Task<Message> {
        if self.dll_scan.is_some() {
            return Task::none();
        }
        let Some(draft) = self.draft.as_ref() else {
            return Task::none();
        };
        if draft.platform.is_native() {
            return Task::none();
        }

        let game_id = draft.id;
        let root = self.scan_dirs.get(&game_id).cloned().or_else(|| {
            draft
                .executable_path
                .as_ref()
                .and_then(|path| path.parent())
                .map(Path::to_path_buf)
        });
        let existing = existing_keys(&draft.dll_rows);
        self.dll_scan = Some(DllScanDialog::new(game_id, root, existing));
        self.start_dll_scan()
    }

    pub(crate) fn start_dll_scan(&mut self) -> Task<Message> {
        let Some(dialog) = self.dll_scan.as_mut() else {
            return Task::none();
        };
        let Some((seq, root, cancel)) = dialog.next_scan() else {
            dialog.phase = DllScanPhase::NeedDirectory;
            return Task::none();
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || scan_dlls(&root, &cancel))
                    .await
                    .unwrap_or_else(|error| Err(DllScanError::Join(error.to_string())))
            },
            move |result| Message::DllScanFinished { seq, result },
        )
    }

    pub(crate) fn close_dll_scan(&mut self) {
        if let Some(dialog) = self.dll_scan.as_mut() {
            if dialog.applying {
                return;
            }
            dialog.cancel_scan();
        }
        self.dll_scan = None;
    }

    pub(crate) fn dll_scan_finished(
        &mut self,
        seq: u64,
        result: Result<DllScanOutcome, DllScanError>,
    ) -> Task<Message> {
        let Some(dialog) = self.dll_scan.as_mut() else {
            return Task::none();
        };
        if dialog.scan_seq != seq || !matches!(dialog.phase, DllScanPhase::Scanning) {
            return Task::none();
        }
        match result {
            Ok(outcome) => {
                let game_id = dialog.game_id;
                let root = dialog.root.clone();
                dialog.phase = DllScanPhase::Ready(outcome);
                dialog.apply_error = None;
                let Some(root) = root else {
                    return Task::none();
                };
                self.scan_dirs.insert(game_id, root.clone());
                self.persist_dll_scan_dir(game_id, &root)
            }
            Err(DllScanError::Cancelled) => Task::none(),
            Err(error) => {
                dialog.phase = DllScanPhase::Failed(error);
                Task::none()
            }
        }
    }

    fn persist_dll_scan_dir(&self, game_id: Uuid, root: &Path) -> Task<Message> {
        let Some(service) = self.service.clone() else {
            return Task::none();
        };
        let Some(path) = root.to_str().map(str::to_owned) else {
            return Task::none();
        };
        let key = application::dll_scan_dir_setting_key(game_id);
        let values = BTreeMap::from([(key, Some(path))]);
        let lang = self.lang;
        Task::perform(service.save_settings(values), move |result| {
            Message::DllScanDirSaved(
                result.map_err(|error| crate::i18n::errors::app_error(lang, &error)),
            )
        })
    }

    pub(crate) fn confirm_dll_scan(&mut self) -> Task<Message> {
        if self.pending.saving || self.pending.deleting {
            return Task::none();
        }
        let lang = self.lang;
        let Some(dialog) = self.dll_scan.as_ref() else {
            return Task::none();
        };
        if dialog.applying {
            return Task::none();
        }
        let DllScanPhase::Ready(outcome) = &dialog.phase else {
            return Task::none();
        };
        let game_id = dialog.game_id;
        let mode = dialog.mode;
        let mut added: Vec<(String, OverrideMode)> = outcome
            .entries
            .iter()
            .filter(|entry| {
                dialog.selected.contains(&entry.key) && !dialog.existing.contains(&entry.key)
            })
            .map(|entry| (entry.name.clone(), mode))
            .collect();
        added.sort_by_key(|a| dll_name_key(&a.0));

        if added.is_empty() {
            if let Some(dialog) = self.dll_scan.as_mut() {
                dialog.apply_error =
                    Some(crate::i18n::tr(lang, "detail.dll_scan.no_selection").to_owned());
            }
            return Task::none();
        }

        let Some(draft) = self.draft.as_mut() else {
            return Task::none();
        };
        if draft.id != game_id {
            return Task::none();
        }
        let original_rows = draft.dll_rows.clone();
        draft
            .dll_rows
            .extend(added.iter().map(|(name, mode)| DllRow {
                name: name.clone(),
                mode: *mode,
            }));
        self.touch_draft();

        let validation = self.draft.as_ref().map(|draft| draft.to_game());
        if let Some(Err(error)) = validation {
            if let Some(draft) = self.draft.as_mut() {
                draft.dll_rows = original_rows;
            }
            self.touch_draft();
            if let Some(dialog) = self.dll_scan.as_mut() {
                dialog.apply_error = Some(crate::i18n::errors::draft_error(lang, &error));
            }
            return Task::none();
        }
        if self.service.is_none() {
            if let Some(draft) = self.draft.as_mut() {
                draft.dll_rows = original_rows;
            }
            self.touch_draft();
            if let Some(dialog) = self.dll_scan.as_mut() {
                dialog.apply_error =
                    Some(crate::i18n::tr(lang, "app.error.database_unavailable").to_owned());
            }
            return Task::none();
        }

        let revision = self.draft_revision;
        if let Some(dialog) = self.dll_scan.as_mut() {
            dialog.pending_apply = Some(PendingApply {
                original_rows,
                added,
                revision,
            });
            dialog.applying = true;
            dialog.apply_error = None;
        }
        self.save_draft()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::i18n::LanguagePref;
    use application::editor::GameDraft;
    use application::{AppPaths, AppService, Platform};

    fn app() -> App {
        let paths = AppPaths::for_root(PathBuf::from("/tmp/gamehub-dll-scan-test"));
        let (mut app, _) = App::new(paths, Err(crate::app::ServiceError::DatabaseLocked));
        app.settings.language = LanguagePref::Fixed(&crate::i18n::ZH_CN);
        app.lang = &crate::i18n::ZH_CN;
        app
    }

    fn candidate(name: &str, relative: &str) -> DllCandidate {
        DllCandidate {
            name: name.to_owned(),
            key: dll_name_key(name),
            relative_path: PathBuf::from(relative),
        }
    }

    fn ready_dialog(entries: Vec<DllCandidate>) -> DllScanDialog {
        let mut dialog = DllScanDialog::new(
            Uuid::new_v4(),
            Some(PathBuf::from("/games")),
            HashSet::new(),
        );
        dialog.phase = DllScanPhase::Ready(DllScanOutcome {
            entries,
            skipped_non_utf8: 0,
            unreadable_dirs: 0,
            truncated: false,
            depth_limited: false,
        });
        dialog
    }

    #[test]
    fn existing_keys_use_the_shared_normalizer() {
        let rows = vec![
            DllRow {
                name: "D3D11.dll".into(),
                mode: OverrideMode::Native,
            },
            DllRow {
                name: "  ".into(),
                mode: OverrideMode::Native,
            },
        ];
        let keys = existing_keys(&rows);
        assert!(keys.contains("d3d11"));
        assert_eq!(keys.len(), 1);
    }

    #[test]
    fn search_sort_and_selection_respect_visibility_and_existing() {
        let mut dialog = ready_dialog(vec![
            candidate("dxgi", "a/dxgi.dll"),
            candidate("d3d11", "d3d11.dll"),
            candidate("winmm", "b/winmm.dll"),
        ]);
        dialog.existing.insert("winmm".into());

        assert_eq!(
            dialog
                .filtered()
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["d3d11", "dxgi", "winmm"]
        );
        dialog.sort_desc = true;
        assert_eq!(dialog.filtered()[0].name, "winmm");

        dialog.query = "d3".into();
        assert_eq!(dialog.filtered().len(), 1);
        dialog.select_all_filtered();
        assert_eq!(dialog.selected, HashSet::from(["d3d11".to_string()]));

        dialog.query.clear();
        dialog.select_all_filtered();
        assert_eq!(dialog.selected.len(), 2);
        assert!(!dialog.selected.contains("winmm"));

        dialog.invert_filtered();
        assert!(dialog.selected.is_empty());
    }

    #[test]
    fn opening_prefers_the_remembered_directory() {
        let mut app = app();
        let mut draft = GameDraft::new(PathBuf::from("/games/one/game.exe"), Platform::Windows);
        draft.name = "One".into();
        let id = draft.id;
        app.scan_dirs.insert(id, PathBuf::from("/remembered"));
        app.draft = Some(draft);

        let _ = app.open_dll_scan();

        let dialog = app.dll_scan.as_ref().expect("dialog opens");
        assert_eq!(dialog.root.as_deref(), Some(Path::new("/remembered")));
        assert!(matches!(dialog.phase, DllScanPhase::Scanning));
        assert_eq!(dialog.scan_seq, 1);
    }

    #[test]
    fn opening_without_any_directory_needs_one() {
        let mut app = app();
        let mut draft = GameDraft::new(PathBuf::from("/game.exe"), Platform::Windows);
        draft.executable_path = None;
        app.draft = Some(draft);

        let _ = app.open_dll_scan();

        let dialog = app.dll_scan.as_ref().expect("dialog opens");
        assert_eq!(dialog.root, None);
        assert!(matches!(dialog.phase, DllScanPhase::NeedDirectory));
    }

    #[test]
    fn stale_scan_results_are_ignored() {
        let mut app = app();
        app.dll_scan = Some(ready_dialog(Vec::new()));
        app.dll_scan.as_mut().unwrap().scan_seq = 2;
        app.dll_scan.as_mut().unwrap().phase = DllScanPhase::Scanning;
        let outcome = DllScanOutcome {
            entries: vec![candidate("d3d11", "d3d11.dll")],
            skipped_non_utf8: 0,
            unreadable_dirs: 0,
            truncated: false,
            depth_limited: false,
        };

        let _ = app.dll_scan_finished(1, Ok(outcome));

        assert!(matches!(
            app.dll_scan.as_ref().unwrap().phase,
            DllScanPhase::Scanning
        ));
    }

    #[test]
    fn confirm_appends_selected_rows_and_marks_applying() {
        let paths = AppPaths::for_root(PathBuf::from("/tmp/gamehub-dll-scan-confirm"));
        let service = AppService::open_in_memory().unwrap();
        let (mut app, _) = App::new(paths, Ok(service));
        app.settings.language = LanguagePref::Fixed(&crate::i18n::ZH_CN);
        app.lang = &crate::i18n::ZH_CN;

        let mut draft = GameDraft::new(PathBuf::from("/games/one/game.exe"), Platform::Windows);
        draft.name = "One".into();
        let id = draft.id;
        app.draft = Some(draft);

        let mut dialog = ready_dialog(vec![
            candidate("d3d11", "d3d11.dll"),
            candidate("dxgi", "dxgi.dll"),
        ]);
        dialog.game_id = id;
        dialog.selected = HashSet::from(["d3d11".to_string(), "dxgi".to_string()]);
        dialog.mode = OverrideMode::Native;
        app.dll_scan = Some(dialog);

        let _ = app.confirm_dll_scan();

        let draft = app.draft.as_ref().unwrap();
        assert_eq!(draft.dll_rows.len(), 2);
        let dialog = app.dll_scan.as_ref().unwrap();
        assert!(dialog.applying);
        assert_eq!(dialog.pending_apply.as_ref().unwrap().added.len(), 2);
    }

    #[test]
    fn failed_save_rolls_back_rows_and_keeps_selection() {
        let mut app = app();
        let mut draft = GameDraft::new(PathBuf::from("/games/one/game.exe"), Platform::Windows);
        draft.name = "One".into();
        let id = draft.id;
        app.draft = Some(draft);
        app.draft_revision = 5;

        let mut dialog = ready_dialog(Vec::new());
        dialog.game_id = id;
        dialog.applying = true;
        dialog.selected = HashSet::from(["d3d11".to_string()]);
        dialog.pending_apply = Some(PendingApply {
            original_rows: Vec::new(),
            added: vec![("d3d11".into(), OverrideMode::Native)],
            revision: 5,
        });
        app.dll_scan = Some(dialog);
        app.draft.as_mut().unwrap().dll_rows.push(DllRow {
            name: "d3d11".into(),
            mode: OverrideMode::Native,
        });

        let _ = app.apply_draft_saved(id, 5, false, Err("db locked".into()));

        assert!(app.draft.as_ref().unwrap().dll_rows.is_empty());
        let dialog = app.dll_scan.as_ref().unwrap();
        assert!(!dialog.applying);
        assert_eq!(dialog.apply_error.as_deref(), Some("db locked"));
        assert!(dialog.selected.contains("d3d11"));
    }

    #[test]
    fn successful_save_closes_the_dialog_and_reports_the_count() {
        let mut app = app();
        let mut draft = GameDraft::new(PathBuf::from("/games/one/game.exe"), Platform::Windows);
        draft.name = "One".into();
        draft.dll_rows.push(DllRow {
            name: "d3d11".into(),
            mode: OverrideMode::Native,
        });
        let id = draft.id;
        let game = draft.to_game().unwrap();
        app.draft = Some(draft);
        app.draft_revision = 5;

        let mut dialog = ready_dialog(Vec::new());
        dialog.game_id = id;
        dialog.applying = true;
        dialog.pending_apply = Some(PendingApply {
            original_rows: Vec::new(),
            added: vec![("d3d11".into(), OverrideMode::Native)],
            revision: 5,
        });
        app.dll_scan = Some(dialog);

        let _ = app.apply_draft_saved(id, 5, false, Ok(game));

        assert!(app.dll_scan.is_none());
        assert_eq!(app.success.as_deref(), Some("已添加 1 个 DLL Override"));
        assert_eq!(app.draft.as_ref().unwrap().dll_rows.len(), 1);
    }

    #[test]
    fn closing_cancels_a_running_scan_but_not_while_saving() {
        let mut app = app();
        let mut dialog = ready_dialog(Vec::new());
        dialog.phase = DllScanPhase::Scanning;
        app.dll_scan = Some(dialog);
        app.close_dll_scan();
        assert!(app.dll_scan.is_none());

        let mut dialog = ready_dialog(Vec::new());
        dialog.applying = true;
        app.dll_scan = Some(dialog);
        app.close_dll_scan();
        assert!(app.dll_scan.is_some());
    }

    #[test]
    fn scan_dir_parser_is_independent_of_loaded_games() {
        let map = BTreeMap::from([(
            application::dll_scan_dir_setting_key(Uuid::nil()),
            "/games".to_string(),
        )]);
        assert_eq!(application::dll_scan_dirs(&map).len(), 1);
    }
}
