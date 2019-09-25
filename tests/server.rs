//! Integration tests for the `tourist serve` command.

use assert_cmd::prelude::*;
use serde::Deserialize;
use serde_json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

struct Server {
    child: Child,
}

macro_rules! rpc {
    ($server:expr, $method:expr, $args:expr, $return:ty) => {{
        #[derive(Deserialize)]
        struct Result {
            result: $return,
        }
        let result: Result =
            serde_json::from_str(&$server.rpc_call($method, $args)).expect("Call failed.");
        result.result
    }};
}

impl Server {
    pub fn new() -> Self {
        let mut cmd = Command::main_binary().expect("could not build command");
        cmd.stdout(Stdio::piped());
        cmd.stdin(Stdio::piped());
        cmd.arg("serve");
        Server {
            child: cmd.spawn().expect("failed to spawn server"),
        }
    }

    pub fn rpc_call(&mut self, method: &str, args: Vec<&str>) -> String {
        let msg = format!(
            "{{ \"id\": 0, \"jsonrpc\": \"2.0\", \"method\": \"{}\", \"params\": {:?} }}\n",
            method, args
        );
        {
            let stdin = self.child.stdin.as_mut().expect("failed to open stdin");
            stdin
                .write_all(msg.as_bytes())
                .expect("failed to write to stdin");
        }
        let stdout = self.child.stdout.as_mut().expect("failed to open stdout");
        let mut reader = BufReader::new(stdout);
        let mut res = String::new();
        reader
            .read_line(&mut res)
            .expect("failed to read line from stdout");
        res
    }
}

#[test]
fn create_simple_tour() {
    let mut server = Server::new();
    let id = rpc!(server, "create_tour", vec!["A tour"], String);
    let results = rpc!(server, "list_tours", vec![], Vec<(String, String)>);
    let new_tour = results
        .iter()
        .find(|(rid, _)| rid == &id)
        .expect("no tour created with expected id");
    assert_eq!("A tour", new_tour.1);
}
