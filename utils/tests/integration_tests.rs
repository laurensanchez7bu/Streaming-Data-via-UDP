use regex::Regex;
use serial_test::serial;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::prelude::ExitStatusExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use wait_timeout::ChildExt;

const SERVER_PATH: &str = "target/debug/server"; // Tell students that their packages MUST be called client and server
const CLIENT_PATH: &str = "target/debug/client";

fn kill_8080() {
    // Kill any process currently listening on port 8080
    let output = Command::new("sh")
        .arg("-c")
        .arg("kill $(lsof -t -i:8080)")
        .output()
        .expect("failed to execute process");
}
#[test]
#[serial]
fn compile_server() {
    let output = Command::new("sh")
        .arg("-c")
        .arg("cargo build --package server")
        .output()
        .expect("failed to execute process");
    // Assert that output doesn't contain the word "error"
    assert!(!String::from_utf8_lossy(&output.stderr).contains("error"));
}

#[test]
#[serial]
fn compile_client() {
    let output = Command::new("sh")
        .arg("-c")
        .arg("cargo build --package client")
        .output()
        .expect("failed to execute process");
    // Assert that output doesn't contain the word "error"
    assert!(!String::from_utf8_lossy(&output.stderr).contains("error"));
}

/// This test should fail the assert, since the server hasn't been started beforehand
#[test]
#[serial]
fn check_not_hardcoded_output() {
    kill_8080();

    let mut client = Command::new("sh")
        .arg("-c")
        .arg(CLIENT_PATH.to_owned() + " " + "127.0.0.1:8080")
        .current_dir("..")
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn process");

    // Sleep for 1 second
    thread::sleep(Duration::from_secs(1));

    let stdout = client.stdout.take().unwrap();

    // Stream output.
    let mut lines = BufReader::new(stdout).lines();

    // Assert that line 0 contains "You've connected to the BearTV Closed Captioning Service. There are X TV stations available."
    // where X is the number of TV stations available.
    if let Some(Ok(line0)) = lines.next() {
        let re = Regex::new(r"You've connected to the BearTV Closed Captioning Service. There are \d+ TV stations available.").unwrap();
        assert!(!re.is_match(&line0));
    }

    // Kill the processes
    client.kill().expect("failed to kill process");

    kill_8080();
}

fn run_test_client() {
    let mut client = Command::new("sh")
        .arg("-c")
        .arg(CLIENT_PATH.to_owned() + " " + "127.0.0.1:8080")
        .current_dir("..")
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn process");

    // Sleep for 1 second
    thread::sleep(Duration::from_secs(1));

    let stdout = client.stdout.take().unwrap();

    // Stream output.
    let mut lines = BufReader::new(stdout).lines();

    // Assert that a line contains "You've connected to the BearTV Closed Captioning Service. There are X TV stations available."
    // where X is the number of TV stations available.
    let re = Regex::new(r"You've connected to the BearTV Closed Captioning Service. There are \d+ TV stations available.").unwrap();
    let mut matched = false;
    for line in lines.into_iter() {
        if re.is_match(&line.unwrap()) {
            matched = true;
            break;
        }
    }
    assert!(matched);
}

#[test]
#[serial]
fn run_server_client() {
    kill_8080();

    compile_server();
    compile_client();

    let mut server = Command::new("sh")
        .arg("-c")
        .arg(SERVER_PATH.to_owned())
        .current_dir("..")
        .spawn()
        .expect("failed to execute process");

    thread::sleep(Duration::from_secs(1));

    run_test_client();

    // Kill the processes
    server.kill().expect("failed to kill process");

    kill_8080();
}
#[should_panic] // This test should panic, since the server hasn't been started beforehand
#[test]
#[serial]
fn run_test_client_no_server() {
    kill_8080();
    compile_client();

    thread::sleep(Duration::from_secs(1));
    run_test_client();
    kill_8080();
}

#[test]
#[serial]
fn run_server_client_parallel() {
    kill_8080();

    compile_server();
    compile_client();

    let mut server = Command::new("sh")
        .arg("-c")
        .arg(SERVER_PATH.to_owned())
        .current_dir("..")
        .spawn()
        .expect("failed to execute process");

    thread::sleep(Duration::from_secs(1));

    // Spawn this 10 times in parallel
    for _ in 0..10 {
        // Create a thread
        thread::spawn(|| {
            run_test_client();
        });
    }

    // Kill the processes
    server.kill().expect("failed to kill process");

    kill_8080();
}

/// Run the client in auto mode. Should connect to TV channel and receive CC data.
fn run_test_client_auto() {
    let mut client = Command::new("sh")
        .arg("-c")
        .arg(CLIENT_PATH.to_owned() + " " + "127.0.0.1:8080 --auto")
        .current_dir("..")
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn process");

    // Sleep for 1 second
    thread::sleep(Duration::from_secs(10));

    let stdout = client.stdout.take().unwrap();

    // Stream output.
    let mut lines = BufReader::new(stdout).lines();

    // Assert that a line contains "You've connected to the BearTV Closed Captioning Service. There are X TV stations available."
    // where X is the number of TV stations available.
    let re = Regex::new(r"You've connected to the BearTV Closed Captioning Service. There are \d+ TV stations available.").unwrap();
    let mut matched_first = false;
    let mut last_line = "".to_string();

    // Some basic tests on the stdout, to try to ensure that the client is receiving and printing
    // CC data
    let mut line_count = 0;
    for line in lines.into_iter() {
        let line = line.unwrap();
        if re.is_match(&line) {
            matched_first = true;
        }
        assert_ne!(line, last_line);
        assert!(line.len() > 5);
        last_line = line;
        line_count += 1;
    }
    assert!(matched_first);
    assert!(line_count > 4);
}

#[test]
#[serial]
fn run_server_client_parallel_auto() {
    kill_8080();

    compile_server();
    compile_client();

    let mut server = Command::new("sh")
        .arg("-c")
        .arg(SERVER_PATH.to_owned())
        .current_dir("..")
        .spawn()
        .expect("failed to execute process");

    thread::sleep(Duration::from_secs(1));

    // Spawn this 10 times in parallel
    for _ in 0..10 {
        // Create a thread
        thread::spawn(|| {
            run_test_client_auto();
        });
    }

    thread::sleep(Duration::from_secs(10));

    // Kill the processes
    server.kill().expect("failed to kill process");

    kill_8080();
}

fn run_test_client_input() -> std::process::Child {
    let mut client = Command::new("sh")
        .arg("-c")
        .arg(CLIENT_PATH.to_owned() + " " + "127.0.0.1:8080")
        .current_dir("..")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn process");

    // Sleep for 1 second
    thread::sleep(Duration::from_secs(1));

    dbg!("ad");

    let client_stdin = client.stdin.as_mut().unwrap();
    let stdout = client.stdout.as_mut().expect("Failed to open stdout");

    // let mut reader = BufReader::new(stdout);
    client_stdin.write_all(b"4\n").unwrap();

    thread::sleep(Duration::from_secs(5));

    // Example expected output:
    // Received from 127.0.0.1:59544: combine sweet chili sauce rice wine vinegar fish sauce sesame seeds coriander and mint and blend together
    let re = Regex::new(r"Received from 127.0.0.1").unwrap();
    let mut buffer = Vec::new();
    let mut temp_buf = [0; 1024]; // Temporary buffer for reading chunks of stdout
    let mut matched = false;
    for _ in 0..10 {
        let bytes_read = stdout.read(&mut temp_buf).unwrap();
        if bytes_read == 0 {
            break; // End of stream
        }
        buffer.extend_from_slice(&temp_buf[..bytes_read]);

        while let Some(newline_pos) = buffer.iter().position(|&b| b == b'\n') {
            let line = buffer.drain(..=newline_pos).collect::<Vec<u8>>();
            // Convert bytes to String, handling invalid UTF-8
            match String::from_utf8(line) {
                Ok(line_str) => {
                    println!("Line: {}", line_str);
                    // Check if the line contains the search term

                    if re.is_match(&line_str[..23.min(line_str.len())]) {
                        matched = true;
                    }
                    break;
                }
                Err(e) => eprintln!("Error decoding UTF-8: {}", e),
            }
        }
    }

    assert!(matched);
    client
}

fn disconnect_client(client: &mut std::process::Child) {
    let client_stdin = client.stdin.as_mut().unwrap();

    // Issue the disconnect command for a connected TV channel
    println!("Disconnecting in test...");
    client_stdin.write_all(b"d\n").unwrap();
    thread::sleep(Duration::from_secs(1));
    // let mut reader = BufReader::new(stdout);
    client_stdin.write_all(b"5\n").unwrap();

    println!("Reconnecting in test...");

    thread::sleep(Duration::from_secs(5));

    let mut stdout = client.stdout.as_mut().expect("Failed to open stdout");

    let re = Regex::new(r"You've connected to the BearTV Closed Captioning Service. There are \d+ TV stations available.").unwrap();

    let mut buffer = Vec::new();
    let mut temp_buf = [0; 1024]; // Temporary buffer for reading chunks of stdout
    let mut matched = false;
    for _ in 0..10 {
        let bytes_read = stdout.read(&mut temp_buf).unwrap();
        if bytes_read == 0 {
            break; // End of stream
        }
        buffer.extend_from_slice(&temp_buf[..bytes_read]);

        while let Some(newline_pos) = buffer.iter().position(|&b| b == b'\n') {
            let line = buffer.drain(..=newline_pos).collect::<Vec<u8>>();
            // Convert bytes to String, handling invalid UTF-8
            match String::from_utf8(line) {
                Ok(line_str) => {
                    println!("Line: {}", line_str);
                    // Check if the line contains the search term

                    if re.is_match(&line_str) {
                        matched = true;
                    }
                    break;
                }
                Err(e) => eprintln!("Error decoding UTF-8: {}", e),
            }
        }
        if matched {
            break;
        }
    }

    assert!(matched);
}

#[test]
#[serial]
fn run_server_client_input() {
    kill_8080();

    compile_server();
    compile_client();

    let mut server = Command::new("sh")
        .arg("-c")
        .arg(SERVER_PATH.to_owned())
        .current_dir("..")
        .spawn()
        .expect("failed to execute process");

    thread::sleep(Duration::from_secs(1));

    run_test_client_input();

    // Kill the processes
    server.kill().expect("failed to kill process");

    kill_8080();
}

#[test]
#[serial]
fn run_server_client_input_multiple() {
    kill_8080();

    compile_server();
    compile_client();

    let mut server = Command::new("sh")
        .arg("-c")
        .arg(SERVER_PATH.to_owned())
        .current_dir("..")
        .spawn()
        .expect("failed to execute process");

    thread::sleep(Duration::from_secs(1));

    let mut child = run_test_client_input();
    disconnect_client(&mut child);

    // Kill the processes
    server.kill().expect("failed to kill process");

    kill_8080();
}
