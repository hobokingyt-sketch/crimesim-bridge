//! Deliberately misbehaving executable, built only with fixture-processes.
use std::{env, fs, io::{Read, Write}, path::PathBuf, process::{Command, Stdio},
    sync::atomic::AtomicBool, time::Duration};
fn forever() -> ! { loop { std::thread::sleep(Duration::from_millis(20)); } }
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.first().map(String::as_str).unwrap_or("success") {
        "success" => { println!("ready"); eprintln!("diagnostic"); },
        "fail" => { eprintln!("controlled failure"); std::process::exit(7); },
        "sleep" => forever(),
        "stdin" => { let mut bytes = Vec::new(); std::io::stdin().read_to_end(&mut bytes).unwrap(); println!("{}", bytes.len()); },
        "args" => println!("{}", args[1..].join("\n")),
        "flood" => {
            let mut out = std::io::stdout().lock(); let mut err = std::io::stderr().lock();
            loop { out.write_all(&[b'x'; 8192]).unwrap(); err.write_all(&[b'y'; 8192]).unwrap(); }
        },
        "finite-flood" => {
            let mut out = std::io::stdout().lock();
            out.write_all(b"START\n").unwrap();
            for n in 0..512 {
                if n == 200 { out.write_all(b"ERROR: middle marker\n").unwrap(); }
                out.write_all(&[b'x'; 8192]).unwrap();
            }
            out.write_all(b"\nEND\n").unwrap();
        },
        "tree" | "exit-parent" | "closed-pipe-child" | "leaf" | "grandchild" => {
            let mode = &args[0]; let root = PathBuf::from(&args[1]);
            fs::write(root.join(format!("{mode}.pid")), std::process::id().to_string()).unwrap();
            if mode != "grandchild" {
                let mut child = Command::new(env::current_exe().unwrap());
                child.arg(if mode == "leaf" { "grandchild" } else { "leaf" }).arg(&root);
                if mode == "closed-pipe-child" { child.stdout(Stdio::null()).stderr(Stdio::null()); }
                child.spawn().unwrap();
            }
            if mode == "exit-parent" || mode == "closed-pipe-child" { return; }
            forever();
        },
        "supervisor" => {
            let root = PathBuf::from(&args[1]);
            let request = bridge_worker::Request { executable: env::current_exe().unwrap(),
                args: vec!["tree".into(), root.clone().into_os_string()], cwd: root,
                timeout: Duration::from_secs(60), cleanup_timeout: Duration::from_secs(3),
                capture_limit: 4096, job_name: args[2].clone() };
            bridge_worker::run(&request, &AtomicBool::new(false)).unwrap();
        },
        other => panic!("Unknown fixture mode {other}"),
    }
}
