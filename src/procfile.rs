use std::{
    collections::HashMap,
    fs::File,
    io::{prelude::*, BufReader},
    path::PathBuf,
};

use regex::Regex;

type Commands = HashMap<String, String>;

pub fn parse_procfile_in_dir(directory: &str) -> Commands {
    let path = PathBuf::from(directory);

    // Prefer .dev suffixed file if it exists
    for filename in &["Procfile.dev", "Procfile"] {
        let file_path = path.join(filename);

        if file_path.is_file() {
            if let Ok(file) = File::open(file_path) {
                return parse_procfile(file);
            };
        };
    }

    HashMap::new()
}

fn parse_procfile<T>(file: T) -> Commands
where
    T: Read,
{
    let lines = BufReader::new(file).lines();

    // Valid Procfile lines have an alphanumeric name followed by a colon, then any sequence of
    // characters as the command. Any lines not matching this pattern are ignored.
    // Regex copied from https://github.com/strongloop/node-foreman/blob/782cf090d4917ff137e9980a36803b93df818b96/lib/procfile.js#L18
    let pattern = Regex::new(r"^([A-Za-z0-9_-]+):\s*(.+)$").unwrap();

    lines
        .filter_map(|line| {
            line.ok().and_then(|line| {
                pattern
                    .captures(&line)
                    .map(|captures| (captures[1].to_string(), captures[2].to_string()))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;

    #[test]
    fn test_single_command() {
        let input = b"web: bin/start_server".as_ref();
        let result = parse_procfile(input);

        let mut expected = HashMap::new();
        expected.insert("web".to_string(), "bin/start_server".to_string());

        assert_eq!(result, expected);
    }

    #[test]
    fn test_multiple_commands() {
        let input = b"test: command\nhello: world args\n".as_ref();
        let result = parse_procfile(input);

        let mut expected = HashMap::new();
        expected.insert("test".to_string(), "command".to_string());
        expected.insert("hello".to_string(), "world args".to_string());

        assert_eq!(result, expected);
    }

    #[test]
    fn test_comment() {
        // Not really a comment, but invalid lines are ignored
        let input = b"#Hi there\nweb: server -e test .\n".as_ref();
        let result = parse_procfile(input);

        let mut expected = HashMap::new();
        expected.insert("web".to_string(), "server -e test .".to_string());

        assert_eq!(result, expected);
    }

    #[test]
    fn test_procfile_in_dir() {
        let temp_dir = test_utils::temp_dir();

        let mut file = std::fs::File::create(temp_dir.join("Procfile")).unwrap();
        file.write_all(b"proc_name: some command\n").unwrap();

        let result = parse_procfile_in_dir(
            temp_dir
                .to_str()
                .expect("Temp directory is an invalid string"),
        );

        let mut expected = HashMap::new();
        expected.insert("proc_name".to_string(), "some command".to_string());

        assert_eq!(expected, result);
    }

    #[test]
    fn test_dir_no_procfile() {
        let temp_dir = test_utils::temp_dir();

        let result = parse_procfile_in_dir(
            temp_dir
                .to_str()
                .expect("Temp directory is an invalid string"),
        );

        let expected = HashMap::new();

        assert_eq!(expected, result);
    }

    #[test]
    fn test_dev_procfile() {
        let temp_dir = test_utils::temp_dir();
        let mut dev_file = std::fs::File::create(temp_dir.join("Procfile.dev")).unwrap();
        dev_file.write_all(b"dev: development command\n").unwrap();
        let mut prod_file = std::fs::File::create(temp_dir.join("Procfile")).unwrap();
        prod_file.write_all(b"prod: production command\n").unwrap();

        let result = parse_procfile_in_dir(
            temp_dir
                .to_str()
                .expect("Temp directory is an invalid string"),
        );

        let mut expected = HashMap::new();
        expected.insert("dev".to_string(), "development command".to_string());

        assert_eq!(expected, result);
    }
}
