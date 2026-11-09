use super::{Lost, Slot, State, with_beats};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

fn slot(state: State, started: u64, beat: u64) -> Slot {
    Slot {
        state,
        started,
        beat,
        timeout: 10,
        pid: 42,
    }
}

#[test]
fn a_slot_round_trips() {
    let states = [
        State::Running,
        State::Finished,
        State::TimedOut,
        State::Interrupted,
    ];
    for state in states {
        let written = slot(state, 1000, 1004);

        assert_eq!(Slot::parse(&written.render()), Some(written), "{state:?}");
    }
    assert_eq!(
        slot(State::TimedOut, 1000, 1004).render(),
        "weighin-heartbeat 1\nstate timed-out\nstarted 1000\nbeat 1004\ntimeout 10\npid 42\n"
    );
}

#[test]
fn a_torn_or_foreign_slot_reads_as_none() {
    let whole = slot(State::Running, 1000, 1004).render();
    let torn = [
        String::new(),
        whole.trim_end().to_owned(),
        whole.replacen("pid 42\n", "", 1),
        whole.replacen("weighin-heartbeat 1", "weighin-heartbeat 2", 1),
        whole.replacen("state running", "state dead", 1),
        whole.replacen("beat 1004", "beat soon", 1),
        whole.replacen("beat 1004", "beat  1004", 1),
        whole.replacen("timeout 10", "deadline 10", 1),
        format!("{whole}extra 1\n"),
    ];
    for text in torn {
        assert_eq!(Slot::parse(&text), None, "{text:?}");
    }
}

#[test]
fn classify() {
    // Started at 1000 with a 10 s timeout: the deadline is 1010, and 1007 is within the 3 s
    // margin of it.
    let cases = [
        (
            "a beat 5 s old is live",
            slot(State::Running, 1000, 1005),
            1010,
            State::Running,
        ),
        (
            "stale, beat at the margin",
            slot(State::Running, 1000, 1007),
            1013,
            State::TimedOut,
        ),
        (
            "stale, beat 1 s before the margin",
            slot(State::Running, 1000, 1006),
            1012,
            State::Interrupted,
        ),
        (
            "a fresh beat past the deadline: the timeout killed it",
            slot(State::Running, 1000, 1009),
            1011,
            State::TimedOut,
        ),
        (
            "a fresh beat at the deadline is still live",
            slot(State::Running, 1000, 1009),
            1010,
            State::Running,
        ),
        (
            "a beat in the future is left alone",
            slot(State::Running, 1000, 1020),
            1010,
            State::Running,
        ),
        (
            "a finished slot stays finished",
            slot(State::Finished, 1000, 1002),
            9999,
            State::Finished,
        ),
        (
            "a lost slot stays as it was rewritten",
            slot(State::Interrupted, 1000, 1002),
            9999,
            State::Interrupted,
        ),
    ];
    for (case, slot, now, state) in cases {
        assert_eq!(slot.state_at(now), state, "{case}");
    }
}

#[test]
fn a_slot_reads_one_way_as_time_passes() {
    // Deadline 1010: past it the margin rule alone decides, whatever the beat's age; before it
    // only a stale beat is dead.
    let cases = [
        (
            "a fresh beat 4 s before the deadline, 1 s past it: the margin rule",
            slot(State::Running, 1000, 1006),
            1011,
            State::Interrupted,
        ),
        (
            "the same slot 1 s later, its beat now stale, reads the same",
            slot(State::Running, 1000, 1006),
            1012,
            State::Interrupted,
        ),
        (
            "a stale beat before the deadline: interrupted",
            slot(State::Running, 1000, 1001),
            1007,
            State::Interrupted,
        ),
    ];
    for (case, slot, now, state) in cases {
        assert_eq!(slot.state_at(now), state, "{case}");
    }
}

#[test]
fn every_slot_on_a_grid_reads_one_way_as_time_passes() {
    // The property `lost` promises, over every beat from the start to past the deadline (1010)
    // and every reading from the beat to well past it: as `now` grows a running slot is read as
    // running until it reads as dead once, and from then on always the same way.
    for beat in 1000..=1013 {
        let slot = slot(State::Running, 1000, beat);
        let mut answer = None;
        for now in beat..=1030 {
            let state = slot.state_at(now);
            match answer {
                None if state != State::Running => answer = Some((state, now)),
                None => {}
                Some((first, at)) => assert_eq!(
                    state, first,
                    "beat {beat}: read {first:?} at {at}, then {state:?} at {now}"
                ),
            }
        }
        assert!(
            answer.is_some(),
            "beat {beat}: a running slot past its deadline is dead"
        );
    }
}

#[test]
fn only_a_dead_running_slot_is_lost() {
    let cases = [
        (slot(State::Running, 1000, 1007), 1013, Some(Lost::TimedOut)),
        (
            slot(State::Running, 1000, 1006),
            1012,
            Some(Lost::Interrupted),
        ),
        (slot(State::Running, 1000, 1005), 1010, None),
        (slot(State::TimedOut, 1000, 1007), 1013, None),
        (slot(State::Finished, 1000, 1002), 9999, None),
        (slot(State::Running, 1000, 1009), 1011, Some(Lost::TimedOut)),
        (slot(State::Running, 1000, 1009), 1010, None),
        (
            slot(State::Running, 1000, 1006),
            1011,
            Some(Lost::Interrupted),
        ),
        (
            slot(State::Running, 1000, 1006),
            1012,
            Some(Lost::Interrupted),
        ),
    ];
    for (slot, now, lost) in cases {
        assert_eq!(slot.lost(now), lost, "{slot:?} at {now}");
    }
}

#[test]
fn the_lost_line_says_how_the_hook_ended() {
    // 1_762_560_000 is 2025-11-08T00:00:00Z.
    let timed_out = slot(State::Running, 1_762_560_000, 1_762_560_008);
    let interrupted = slot(State::Running, 1_762_560_000, 1_762_560_002);

    assert_eq!(
        timed_out.lost_line(Lost::TimedOut),
        "the hook started at 2025-11-08T00:00:00Z was killed by its 10 s timeout"
    );
    assert_eq!(
        interrupted.lost_line(Lost::Interrupted),
        "the hook started at 2025-11-08T00:00:00Z ended 8 s before its 10 s timeout \
         without finishing (interrupted)"
    );
}

#[test]
fn status_sees_a_slot_as_the_next_hook_would_and_when_it_last_beat() {
    let dead = slot(State::Running, 1_762_560_000, 1_762_560_002);
    let live = slot(State::Running, 1_762_560_000, 1_762_560_002);
    let logged = slot(State::Interrupted, 1_762_560_000, 1_762_560_002);

    assert_eq!(
        dead.lost_report(1_762_560_020).as_deref(),
        Some(
            "the hook started at 2025-11-08T00:00:00Z ended 8 s before its 10 s timeout \
             without finishing (interrupted)"
        )
    );
    assert_eq!(live.lost_report(1_762_560_003), None, "still beating");
    assert_eq!(logged.lost_report(1_762_560_020), None, "a hook logged it");
    assert_eq!(dead.at(), "2025-11-08T00:00:02Z", "its last beat");
}

#[test]
fn beats_run_while_the_work_runs() {
    let (tx, rx) = mpsc::sync_channel::<()>(1);

    let (beat_seen, failed) = with_beats(
        Duration::ZERO,
        || {
            // Full or gone is fine: one beat is all the work waits for.
            match tx.try_send(()) {
                Ok(())
                | Err(mpsc::TrySendError::Full(()) | mpsc::TrySendError::Disconnected(())) => {
                    Ok(())
                }
            }
        },
        || rx.recv_timeout(Duration::from_secs(30)).is_ok(),
    );

    assert!(beat_seen, "a beat came while the work ran");
    assert!(failed.is_none(), "{failed:?}");
}

#[test]
fn no_beat_comes_before_its_period_and_the_beater_ends_with_the_work() {
    let beats = AtomicUsize::new(0);

    // An hour's period: only the work's end, dropping the sender, wakes the beater.
    let ((), failed) = with_beats(
        Duration::from_secs(3600),
        || {
            // Relaxed: the scope's join orders this before the read below.
            beats.fetch_add(1, Ordering::Relaxed);
            Ok(())
        },
        || (),
    );

    assert_eq!(
        beats.load(Ordering::Relaxed),
        0,
        "no beat before its period"
    );
    assert!(failed.is_none(), "{failed:?}");
}

#[test]
fn a_failed_beat_stops_the_beats_and_is_returned() {
    let beats = AtomicUsize::new(0);
    let (tx, rx) = mpsc::sync_channel::<()>(1);

    let (beat_seen, failed) = with_beats(
        Duration::ZERO,
        || {
            // Relaxed: the scope's join orders this before the read below.
            beats.fetch_add(1, Ordering::Relaxed);
            match tx.try_send(()) {
                Ok(())
                | Err(mpsc::TrySendError::Full(()) | mpsc::TrySendError::Disconnected(())) => {}
            }
            Err(anyhow::anyhow!("the disk is full"))
        },
        || rx.recv_timeout(Duration::from_secs(30)).is_ok(),
    );

    assert!(beat_seen, "the beat ran");
    assert_eq!(
        beats.load(Ordering::Relaxed),
        1,
        "no beat after a failed one"
    );
    assert_eq!(
        failed.map(|e| e.to_string()),
        Some("the disk is full".to_owned())
    );
}
