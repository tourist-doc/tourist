//! Integration tests for the tourist CLI. Some more complex commands are
//! broken out into their own files.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn no_subcommand_fails() {
    let mut cmd = Command::main_binary().expect("command failed");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("subcommand"));
}
