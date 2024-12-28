use procspawn;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

// Enable procspawn test support
#[cfg(test)]
procspawn::enable_test_support!();

//--------------------------------------------------------------------------------------------------
// Function: Helper
//--------------------------------------------------------------------------------------------------

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

    // Verify state.db was created
    let db_path = menv_path.join("state.db");
    assert!(db_path.exists(), "state.db should exist");
    assert!(db_path.is_file(), "state.db should be a file");

    // Verify monocore.yaml was created
    let config_path = dir_path.join("monocore.yaml");
    assert!(config_path.exists(), "monocore.yaml should exist");
    assert!(config_path.is_file(), "monocore.yaml should be a file");

    // Verify monocore.yaml contents
    let config_contents = fs::read_to_string(config_path).unwrap();
    assert!(
        config_contents.contains("meta:"),
        "config should have meta section"
    );
    assert!(
        config_contents.contains("sandboxes:"),
        "config should have sandboxes section"
    );
    assert!(
        config_contents.contains("groups:"),
        "config should have groups section"
    );
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[test]
fn test_init_command_with_path() {
    procspawn::init();

    // Create a temporary directory for the test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_path_buf();

    // Get the path to the monocore binary from environment variable
    let monocore_bin =
        std::env::var("MONOCORE_BIN").expect("MONOCORE_BIN environment variable not set");

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
fn test_init_command_current_dir() {
    procspawn::init();

    // Create a temporary directory for the test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_path_buf();

    // Get the path to the monocore binary from environment variable
    let monocore_bin =
        std::env::var("MONOCORE_BIN").expect("MONOCORE_BIN environment variable not set");

    // Run the monocore init command without path in a separate process
    let handle = procspawn::spawn((monocore_bin, temp_path.clone()), |(bin, path)| {
        let output = Command::new(bin)
            .arg("init")
            .current_dir(&path) // Set working directory instead of changing process working dir
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
fn test_init_command_existing_dir() {
    procspawn::init();

    // Create a temporary directory for the test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_path_buf();

    // Create a file in the directory that shouldn't be affected
    let test_file = temp_path.join("test.txt");
    fs::write(&test_file, "test content").unwrap();

    // Get the path to the monocore binary from environment variable
    let monocore_bin =
        std::env::var("MONOCORE_BIN").expect("MONOCORE_BIN environment variable not set");

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
fn test_init_command_idempotent() {
    procspawn::init();

    // Create a temporary directory for the test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_path_buf();

    // Get the path to the monocore binary from environment variable
    let monocore_bin =
        std::env::var("MONOCORE_BIN").expect("MONOCORE_BIN environment variable not set");

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
