use application::{AppError, AppService, DeleteTarget, Game};
use launcher_core::{Arch, DisplayConfig, Platform};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use uuid::Uuid;

fn game(dir: &Path) -> Game {
    let exe = dir.join("game.exe");
    std::fs::write(&exe, b"MZ").unwrap();
    Game {
        id: Uuid::new_v4(),
        name: "Cover Test".into(),
        executable_path: exe,
        working_dir: None,
        runner_id: None,
        prefix_path: None,
        cover_path: None,
        platform: Platform::Windows,
        arch: Arch::Win64,
        display: DisplayConfig::default(),
        env_overrides: BTreeMap::new(),
        dll_overrides: Vec::new(),
        args: Vec::new(),
        created_at: 1,
        last_played_at: None,
        playtime_seconds: 0,
    }
}

fn write_image(path: &Path, width: u32, height: u32) {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 90, 255]));
    image.save(path).unwrap();
}

#[tokio::test]
async fn save_copies_cover_into_covers_dir() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("cover.png");
    write_image(&source, 4, 6);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    let saved = service
        .clone()
        .save_game(value, true, Some(source), covers.clone())
        .await
        .unwrap();

    let expected = covers.join(format!("{id}.png"));
    assert_eq!(saved.cover_path.as_deref(), Some(expected.as_path()));
    assert!(expected.is_file());
}

#[tokio::test]
async fn replacing_cover_removes_the_old_file() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let first = dir.path().join("first.png");
    let second = dir.path().join("second.bmp");
    write_image(&first, 4, 6);
    write_image(&second, 5, 7);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    let saved = service
        .clone()
        .save_game(value, true, Some(first), covers.clone())
        .await
        .unwrap();
    let old = saved.cover_path.clone().unwrap();
    assert!(old.is_file());

    let saved = service
        .clone()
        .save_game(saved, false, Some(second), covers.clone())
        .await
        .unwrap();
    let new = covers.join(format!("{id}.bmp"));
    assert_eq!(saved.cover_path.as_deref(), Some(new.as_path()));
    assert!(new.is_file());
    assert!(!old.is_file());
}

#[tokio::test]
async fn clearing_cover_removes_the_file_and_column() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("cover.png");
    write_image(&source, 4, 6);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let mut saved = service
        .clone()
        .save_game(value, true, Some(source), covers.clone())
        .await
        .unwrap();
    let cover = saved.cover_path.clone().unwrap();
    assert!(cover.is_file());

    saved.cover_path = None;
    let cleared = service
        .clone()
        .save_game(saved, false, None, covers.clone())
        .await
        .unwrap();
    assert_eq!(cleared.cover_path, None);
    assert!(!cover.is_file());
    assert_eq!(
        service.load_initial().await.unwrap().games[0].cover_path,
        None
    );
}

#[tokio::test]
async fn rejects_unsupported_cover_extension_without_writing() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("cover.txt");
    std::fs::write(&source, b"not an image").unwrap();

    let service = AppService::open_in_memory().unwrap();
    let result = service
        .clone()
        .save_game(game(dir.path()), true, Some(source), covers.clone())
        .await;

    assert!(matches!(result, Err(AppError::CoverUnsupportedFormat(_))));
    assert!(service.load_initial().await.unwrap().games.is_empty());
    assert!(!covers.exists());
}

#[tokio::test]
async fn rejects_oversized_cover() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("wide.png");
    write_image(&source, 8193, 1);

    let service = AppService::open_in_memory().unwrap();
    let result = service
        .clone()
        .save_game(game(dir.path()), true, Some(source), covers.clone())
        .await;

    assert!(matches!(
        result,
        Err(AppError::CoverTooLarge {
            width: 8193,
            height: 1
        })
    ));
}

#[tokio::test]
async fn deleting_a_game_removes_its_cover() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("cover.png");
    write_image(&source, 4, 6);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    let saved = service
        .clone()
        .save_game(value, true, Some(source), covers.clone())
        .await
        .unwrap();
    let cover = saved.cover_path.clone().unwrap();

    service
        .clone()
        .delete(
            DeleteTarget::Game(id),
            HashMap::new(),
            None,
            dir.path().join("logs"),
        )
        .await
        .unwrap();

    assert!(!cover.is_file());
    assert!(service.load_initial().await.unwrap().games.is_empty());
}

#[tokio::test]
async fn deleting_a_game_removes_its_log() {
    let dir = tempdir().unwrap();
    let logs = dir.path().join("logs");

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    service
        .clone()
        .save_game(value, true, None, PathBuf::new())
        .await
        .unwrap();

    let log = logs.join("games").join(format!("{id}.log"));
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    std::fs::write(&log, b"launch log").unwrap();

    service
        .clone()
        .delete(DeleteTarget::Game(id), HashMap::new(), None, logs.clone())
        .await
        .unwrap();

    assert!(!log.exists());
    assert!(service.load_initial().await.unwrap().games.is_empty());
}

#[tokio::test]
async fn replacing_with_same_extension_keeps_a_valid_file() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let first = dir.path().join("first.png");
    let second = dir.path().join("second.png");
    write_image(&first, 4, 6);
    write_image(&second, 8, 9);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    let saved = service
        .clone()
        .save_game(value, true, Some(first), covers.clone())
        .await
        .unwrap();
    let cover = saved.cover_path.clone().unwrap();

    let saved = service
        .clone()
        .save_game(saved, false, Some(second), covers.clone())
        .await
        .unwrap();
    assert_eq!(saved.cover_path.as_deref(), Some(cover.as_path()));
    assert!(cover.is_file());
    assert!(std::fs::metadata(&cover).unwrap().len() > 0);
    assert_eq!(image::image_dimensions(&cover).unwrap(), (8, 9));
    assert_eq!(cover_file_names(&covers), vec![format!("{id}.png")]);
}

#[tokio::test]
async fn repicking_the_current_cover_does_not_truncate_it() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("cover.png");
    write_image(&source, 4, 6);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    let saved = service
        .clone()
        .save_game(value, true, Some(source), covers.clone())
        .await
        .unwrap();
    let cover = saved.cover_path.clone().unwrap();
    let original_len = std::fs::metadata(&cover).unwrap().len();

    let saved = service
        .clone()
        .save_game(saved, false, Some(cover.clone()), covers.clone())
        .await
        .unwrap();
    assert_eq!(saved.cover_path.as_deref(), Some(cover.as_path()));
    assert_eq!(std::fs::metadata(&cover).unwrap().len(), original_len);
    assert_eq!(image::image_dimensions(&cover).unwrap(), (4, 6));
    assert_eq!(cover_file_names(&covers), vec![format!("{id}.png")]);
}

#[tokio::test]
async fn rejects_corrupt_image_with_allowed_extension() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("cover.png");
    std::fs::write(&source, b"not really a png").unwrap();

    let service = AppService::open_in_memory().unwrap();
    let result = service
        .clone()
        .save_game(game(dir.path()), true, Some(source), covers.clone())
        .await;

    assert!(matches!(result, Err(AppError::CoverInvalidImage { .. })));
    assert!(!covers.exists());
}

#[tokio::test]
async fn duplicate_insert_rolls_back_without_losing_existing_cover() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let first = dir.path().join("first.png");
    let second = dir.path().join("second.png");
    write_image(&first, 4, 6);
    write_image(&second, 8, 9);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    let saved = service
        .clone()
        .save_game(value.clone(), true, Some(first), covers.clone())
        .await
        .unwrap();
    let cover = saved.cover_path.clone().unwrap();
    let original_len = std::fs::metadata(&cover).unwrap().len();

    // Same id inserted again as "new" with a same-extension source:
    // insert fails, temp/imported file must not destroy the existing cover.
    let result = service
        .clone()
        .save_game(value, true, Some(second), covers.clone())
        .await;
    assert!(result.is_err());
    assert!(cover.is_file());
    assert_eq!(std::fs::metadata(&cover).unwrap().len(), original_len);
    assert_eq!(image::image_dimensions(&cover).unwrap(), (4, 6));
    assert_eq!(cover_file_names(&covers), vec![format!("{id}.png")]);
    assert_eq!(
        service.load_initial().await.unwrap().games[0].cover_path,
        Some(cover)
    );
}

#[tokio::test]
async fn duplicate_insert_with_new_extension_rolls_back_the_new_file() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let first = dir.path().join("first.png");
    let second = dir.path().join("second.bmp");
    write_image(&first, 4, 6);
    write_image(&second, 8, 9);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    let saved = service
        .clone()
        .save_game(value.clone(), true, Some(first), covers.clone())
        .await
        .unwrap();
    let cover = saved.cover_path.clone().unwrap();

    let result = service
        .clone()
        .save_game(value, true, Some(second), covers.clone())
        .await;
    assert!(result.is_err());

    assert!(cover.is_file());
    assert_eq!(image::image_dimensions(&cover).unwrap(), (4, 6));
    assert!(!covers.join(format!("{id}.bmp")).exists());
    assert_eq!(cover_file_names(&covers), vec![format!("{id}.png")]);
    assert_eq!(
        service.load_initial().await.unwrap().games[0].cover_path,
        Some(cover)
    );
}

fn cover_file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}
