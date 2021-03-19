use std::{fs::read_to_string, process::Command};

use lang_tester::LangTester;

fn main() {
    LangTester::new()
        .test_dir("lang_tests")
        // Only use files named `*.txt` as test files.
        .test_file_filter(|p| p.extension().unwrap().to_str().unwrap() == "txt")
        // Extract the first sequence of commented line(s) as the tests.
        .test_extract(|p| {
            read_to_string(p)
                .unwrap()
                .lines()
                // Skip until the first comment
                .skip_while(|l| !l.starts_with("{!"))
                // Extract comment body
                .skip(1)
                .take_while(|l| !l.starts_with("!}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .test_cmds(move |p| {
            let mut runtime = Command::new("target/debug/individual_project");
            runtime.args(&[p]);
            vec![("Run-time", runtime)]
        })
        .run();
}