#![cfg(test)]

use std::{
    collections::HashMap,
    env::{self, var},
    fs::{self, File},
    io::Write,
};

use tempfile::TempDir;

use crate::{
    dotenv, dotenv_iter, from_filename, from_filename_iter, from_path, from_path_iter, vars,
};

fn init(value: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    env::set_current_dir(&dir).unwrap();
    let dotenv_path = dir.path().join(".env");
    let mut dotenv_file = File::create(dotenv_path).unwrap();
    dotenv_file.write_all(value.as_bytes()).unwrap();
    dotenv_file.flush().unwrap();
    dir
}

fn init_default() -> TempDir {
    init("TESTKEY=test_val")
}

#[test]
fn test_child_dir() {
    let _guard = init_default();

    fs::create_dir("child").unwrap();

    env::set_current_dir("child").unwrap();

    dotenv().ok();
    assert_eq!(var("TESTKEY").unwrap(), "test_val");
}

#[test]
fn test_default_location() {
    let _guard = init_default();

    dotenv().ok();
    assert_eq!(var("TESTKEY").unwrap(), "test_val");
}

#[test]
fn test_dotenv_iter() {
    let _guard = init_default();

    let iter = dotenv_iter().unwrap();

    assert!(var("TESTKEY").is_err());

    iter.load().ok();
    assert_eq!(var("TESTKEY").unwrap(), "test_val");
}

#[test]
fn test_from_filename_iter() {
    let _guard = init_default();

    let iter = from_filename_iter(".env").unwrap();

    assert!(var("TESTKEY").is_err());

    iter.load().ok();
    assert_eq!(var("TESTKEY").unwrap(), "test_val");
}

#[test]
fn test_from_filename() {
    let _guard = init_default();

    from_filename(".env").ok();
    assert_eq!(var("TESTKEY").unwrap(), "test_val");
}

#[test]
fn test_from_path_iter() {
    let _guard = init_default();

    let mut path = env::current_dir().unwrap();
    path.push(".env");

    let iter = from_path_iter(&path).unwrap();

    assert!(var("TESTKEY").is_err());

    iter.load().ok();
    assert_eq!(var("TESTKEY").unwrap(), "test_val");
}

#[test]
fn test_from_path() {
    let _guard = init_default();

    let mut path = env::current_dir().unwrap();
    path.push(".env");

    from_path(&path).ok();

    assert_eq!(var("TESTKEY").unwrap(), "test_val");
}

#[test]
fn test_var() {
    let _guard = init_default();

    dotenv().ok();

    assert_eq!(var("TESTKEY").unwrap(), "test_val");
}

#[test]
fn test_variable_substitutions() {
    std::env::set_var("KEY", "value");
    std::env::set_var("KEY1", "value1");

    let substitutions_to_test = [
        "$ZZZ", "$KEY", "$KEY1", "${KEY}1", "$KEY_U", "${KEY_U}", "\\$KEY",
    ];

    let common_string = substitutions_to_test.join(">>");

    let _guard = init(&format!(
        r#"
    KEY1=new_value1
    KEY_U=$KEY+valueU
    
    SUBSTITUTION_FOR_STRONG_QUOTES='{common_string}'
    SUBSTITUTION_FOR_WEAK_QUOTES="{common_string}"
    SUBSTITUTION_WITHOUT_QUOTES={common_string}
    "#,
    ));

    dotenv().ok();

    assert_eq!(var("KEY").unwrap(), "value");
    assert_eq!(var("KEY1").unwrap(), "value1");
    assert_eq!(var("KEY_U").unwrap(), "value+valueU");
    assert_eq!(
        var("SUBSTITUTION_FOR_STRONG_QUOTES").unwrap(),
        common_string
    );
    assert_eq!(
        var("SUBSTITUTION_FOR_WEAK_QUOTES").unwrap(),
        [
            "",
            "value",
            "value1",
            "value1",
            "value_U",
            "value+valueU",
            "$KEY"
        ]
        .join(">>")
    );
    assert_eq!(
        var("SUBSTITUTION_WITHOUT_QUOTES").unwrap(),
        [
            "",
            "value",
            "value1",
            "value1",
            "value_U",
            "value+valueU",
            "$KEY"
        ]
        .join(">>")
    );
}

#[test]
fn test_vars() {
    let _guard = init_default();

    let vars: HashMap<String, String> = vars().collect();

    assert_eq!(vars["TESTKEY"], "test_val");
}
