use procspawn;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

//--------------------------------------------------------------------------------------------------
// Function: Helper
//--------------------------------------------------------------------------------------------------

/// Get the path to the monocore binary from build directory
fn get_monocore_bin() -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let project_root = Path::new(&manifest_dir)
        .parent()
        .expect("Failed to get project root");
    project_root
        .join("build")
        .join("monocore")
        .to_string_lossy()
        .into_owned()
}

/// Create a temporary directory for testing
fn create_temp_dir() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_path_buf();
    (temp_dir, temp_path)
}

/// Helper function to run monocore init command and verify its results
fn verify_init_results(dir_path: &std::path::Path) {
    // Verify .menv directory was created
    let menv_path = dir_path.join(".menv");
    assert!(menv_path.exists(), ".menv directory should exist");
    assert!(menv_path.is_dir(), ".menv should be a directory");

    // Verify log directory was created
    let log_path = menv_path.join("log");
    assert!(log_path.exists(), "log directory should exist");
    assert!(log_path.is_dir(), "log should be a directory");

    // Verify active.db was created
    let db_path = menv_path.join("active.db");
    assert!(db_path.exists(), "active.db should exist");
    assert!(db_path.is_file(), "active.db should be a file");

    // Verify monocore.yaml was created
    let config_path = dir_path.join("monocore.yaml");
    assert!(config_path.exists(), "monocore.yaml should exist");
    assert!(config_path.is_file(), "monocore.yaml should be a file");

    // Verify monocore.yaml contents
    let config_contents = fs::read_to_string(config_path).unwrap();
    assert!(
        config_contents.contains("sandboxes:"),
        "config should have sandboxes section"
    );
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[test]
fn integration_test_init_command_with_path() {
    procspawn::init();

    // Create temp directory and get binary path
    let (_temp_dir, temp_path) = create_temp_dir();
    let monocore_bin = get_monocore_bin();

    // Run the monocore init command with explicit path in a separate process
    let handle = procspawn::spawn((monocore_bin, temp_path.clone()), |(bin, path)| {
        let output = Command::new(bin)
            .arg("init")
            .arg(path)
            .output()
            .expect("Failed to execute monocore init command");

        assert!(
            output.status.success(),
            "monocore init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    });

    // Wait for the process to complete
    handle.join().expect("Process failed");

    verify_init_results(&temp_path);
}

#[test]
fn integration_test_init_command_current_dir() {
    procspawn::init();

    // Create temp directory and get binary path
    let (_temp_dir, temp_path) = create_temp_dir();
    let monocore_bin = get_monocore_bin();

    // Run the monocore init command without path in a separate process
    let handle = procspawn::spawn((monocore_bin, temp_path.clone()), |(bin, path)| {
        let output = Command::new(bin)
            .arg("init")
            .current_dir(&path)
            .output()
            .expect("Failed to execute monocore init command");

        assert!(
            output.status.success(),
            "monocore init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    });

    // Wait for the process to complete
    handle.join().expect("Process failed");

    verify_init_results(&temp_path);
}

#[test]
fn integration_test_init_command_existing_dir() {
    procspawn::init();

    // Create temp directory and get binary path
    let (_temp_dir, temp_path) = create_temp_dir();
    let monocore_bin = get_monocore_bin();

    // Create a file in the directory that shouldn't be affected
    let test_file = temp_path.join("test.txt");
    fs::write(&test_file, "test content").unwrap();

    // Run the monocore init command in a separate process
    let handle = procspawn::spawn(
        (monocore_bin, temp_path.clone(), test_file.clone()),
        |(bin, path, test_file)| {
            let output = Command::new(bin)
                .arg("init")
                .arg(&path)
                .output()
                .expect("Failed to execute monocore init command");

            assert!(
                output.status.success(),
                "monocore init failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );

            // Verify the test file still exists and is unchanged
            assert!(test_file.exists(), "test file should still exist");
            let contents = fs::read_to_string(&test_file).unwrap();
            assert_eq!(
                contents, "test content",
                "test file contents should be unchanged"
            );
        },
    );

    // Wait for the process to complete
    handle.join().expect("Process failed");

    // Verify monocore files were created
    assert!(temp_path.join(".menv").exists());
    assert!(temp_path.join("monocore.yaml").exists());
}

#[test]
fn integration_test_init_command_idempotent() {
    procspawn::init();

    // Create temp directory and get binary path
    let (_temp_dir, temp_path) = create_temp_dir();
    let monocore_bin = get_monocore_bin();

    // Run the first init in a separate process
    let handle = procspawn::spawn((monocore_bin, temp_path.clone()), |(bin, path)| {
        // First init
        let output = Command::new(&bin)
            .arg("init")
            .arg(&path)
            .output()
            .expect("Failed to execute first monocore init command");

        assert!(
            output.status.success(),
            "First monocore init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Modify the config file to verify it's not overwritten
        let config_path = path.join("monocore.yaml");
        fs::write(&config_path, "modified content").unwrap();

        // Run the second init
        let output = Command::new(&bin)
            .arg("init")
            .arg(&path)
            .output()
            .expect("Failed to execute second monocore init command");

        assert!(
            output.status.success(),
            "Second monocore init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify the config file wasn't overwritten
        let contents = fs::read_to_string(&config_path).unwrap();
        assert_eq!(
            contents, "modified content",
            "config file should not be overwritten"
        );
    });

    // Wait for the process to complete
    handle.join().expect("Process failed");
}
