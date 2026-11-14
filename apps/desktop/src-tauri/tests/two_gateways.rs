//! Review PR-01 with real operating-system processes: two `gateway serve` on one store. The second is
//! refused before it listens or sends anything; once the first is killed (no clean shutdown), the operating
//! system releases its lock, the next one starts, and the attempt the dead one left unsettled becomes an
//! interrupted ledger entry with its held amount still counted.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const GATEWAY: &str = env!("CARGO_BIN_EXE_gateway");

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn serve(db: &std::path::Path) -> Child {
    Command::new(GATEWAY)
        .args(["--db", db.to_str().unwrap(), "serve", "--port", &free_port().to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

/// Waits for the "listening" line, or returns what the process printed before it exited.
fn wait_listening(child: &mut Child) -> Result<(), String> {
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    out.read_line(&mut line).unwrap();
    if line.starts_with("Gateway listening") {
        return Ok(());
    }
    let status = child.wait().unwrap();
    let mut err = String::new();
    std::io::Read::read_to_string(child.stderr.as_mut().unwrap(), &mut err).unwrap();
    Err(format!("exit {status}: {err}"))
}

#[test]
fn a_second_process_is_refused_and_a_killed_owner_leaves_no_stale_lock() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("store.sqlite3");
    let created = Command::new(GATEWAY)
        .args(["--db", db.to_str().unwrap(), "create-client", "--name", "Tests"])
        .output()
        .unwrap();
    assert!(created.status.success());

    let mut first = serve(&db);
    wait_listening(&mut first).expect("the first process serves the store");

    // An attempt the first process journaled and never settled (it is about to be killed mid-request).
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.busy_timeout(Duration::from_secs(5)).unwrap();
        conn.execute("INSERT INTO budget_reservations (id, client_id, reserved_micro, created_at, provider_id, model_id, \
            requested_model, route_mode) SELECT 'a1', id, 2500, ?1, 'main', 'fast', 'main/fast', 'native' FROM gateway_clients",
            [pygg_desktop_lib::db::now_ms()]).unwrap();
    }

    let mut second = serve(&db);
    let refused = wait_listening(&mut second).expect_err("a second process on the same store is refused");
    assert!(refused.contains("Another gateway is already serving this store"), "{refused}");

    // Killed, not stopped: the lock goes with the process.
    first.kill().unwrap();
    first.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut third = loop {
        let mut next = serve(&db);
        match wait_listening(&mut next) {
            Ok(()) => break next,
            Err(e) if Instant::now() < deadline => {
                let _ = e;
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => panic!("the store stayed locked after its owner died: {e}"),
        }
    };

    let conn = rusqlite::Connection::open(&db).unwrap();
    let (outcome, at_risk): (String, Option<i64>) = conn
        .query_row("SELECT outcome, at_risk_micro_usd FROM usage_events WHERE model = 'fast'", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((outcome.as_str(), at_risk), ("interrupted", Some(2500)));
    let journal: i64 = conn.query_row("SELECT COUNT(*) FROM budget_reservations", [], |r| r.get(0)).unwrap();
    assert_eq!(journal, 0);

    third.kill().unwrap();
    third.wait().unwrap();
}
