use std::time::{Duration, Instant};

use super::{
    FLOOR, MIN_GIT_TIMEOUT, OutOfTime, STORE_WAIT, Steps, cap, every_remaining, git_timeout, share,
    step,
};
use crate::hook::HOOK_BUDGET;
use crate::pr_diff::Budget;

/// A budget with `left` of it left now.
fn left(left: Duration) -> Budget {
    Budget::within(Instant::now(), left, Duration::from_secs(1))
}

#[test]
fn a_hook_store_wait_is_two_seconds() {
    assert_eq!(STORE_WAIT, Duration::from_secs(2));
}

#[test]
fn a_step_waits_what_is_left_after_what_follows_it() {
    let after = Duration::from_millis(500);
    let wait = cap(&left(HOOK_BUDGET), after).expect("plenty left");
    // 4.5 s less the 500 ms that follows, less a little for the time this test took.
    assert!(
        wait <= Duration::from_millis(4_000) && wait > Duration::from_millis(3_900),
        "{wait:?}"
    );
    assert_eq!(
        cap(&left(after + Duration::from_millis(50)), after),
        Err(OutOfTime {
            needed: after + FLOOR
        }),
        "under the floor"
    );
    assert_eq!(
        cap(&Budget::per_call(Duration::from_secs(1)), after),
        Ok(Duration::MAX.checked_sub(after).expect("far from zero")),
        "a command has no deadline"
    );
}

#[test]
fn git_and_the_floors_of_the_steps_after_it_never_pass_what_is_left() {
    for count in 1..=5 {
        for after in [Duration::ZERO, Duration::from_secs(1)] {
            let steps = Steps { count, after };
            let overrun = every_remaining(HOOK_BUDGET)
                .filter_map(|left| {
                    git_timeout(left, steps, Duration::from_secs(1))
                        .ok()
                        .map(|git| (left, git))
                })
                .find(|(left, git)| *git + FLOOR * count + after > *left || *git < MIN_GIT_TIMEOUT);
            assert_eq!(overrun, None, "{steps:?}: (time left, git's timeout)");
        }
    }
}

#[test]
fn git_gets_its_cap_unless_the_floors_leave_less() {
    let cap = Duration::from_secs(1);
    let steps = |count| Steps {
        count,
        after: Duration::ZERO,
    };
    assert_eq!(git_timeout(HOOK_BUDGET, steps(5), cap), Ok(cap));
    assert_eq!(
        git_timeout(Duration::from_millis(700), steps(2), cap),
        Ok(Duration::from_millis(500))
    );
    assert_eq!(
        git_timeout(Duration::from_millis(399), steps(2), cap),
        Err(OutOfTime {
            needed: FLOOR * 2 + MIN_GIT_TIMEOUT
        }),
        "less than the least worth giving git"
    );
    assert_eq!(
        git_timeout(
            Duration::from_millis(1_399),
            Steps {
                count: 2,
                after: Duration::from_secs(1)
            },
            cap
        ),
        Err(OutOfTime {
            needed: FLOOR * 2 + Duration::from_secs(1) + MIN_GIT_TIMEOUT
        }),
        "what follows the steps is left too"
    );
}

#[test]
fn the_steps_of_one_open_share_what_is_left_and_never_pass_it() {
    for count in 1..=5 {
        for after in [Duration::ZERO, Duration::from_secs(1)] {
            let steps = Steps { count, after };
            let overrun = every_remaining(HOOK_BUDGET)
                .filter_map(|left| share(left, steps).ok().map(|wait| (left, wait)))
                .find(|(left, wait)| {
                    *wait * count + after > *left || *wait > STORE_WAIT || *wait < FLOOR
                });
            assert_eq!(overrun, None, "{steps:?}: (time left, each step's wait)");
        }
    }
}

#[test]
fn an_open_shares_what_is_left_at_most_a_hook_wait() {
    let steps = |count| Steps {
        count,
        after: Duration::ZERO,
    };
    assert_eq!(share(HOOK_BUDGET, steps(2)), Ok(STORE_WAIT), "2.25 s each");
    assert_eq!(
        share(Duration::from_millis(3_500), steps(5)),
        Ok(Duration::from_millis(700))
    );
    assert_eq!(
        share(Duration::from_millis(499), steps(5)),
        Err(OutOfTime { needed: FLOOR * 5 }),
        "under the floor each"
    );
    assert_eq!(
        share(HOOK_BUDGET, steps(0)),
        Err(OutOfTime {
            needed: Duration::ZERO
        }),
        "no step to share among"
    );
}

#[test]
fn a_later_step_starts_only_when_its_wait_and_what_it_takes_besides_fit() {
    let wait = Duration::from_millis(700);
    assert_eq!(
        step(&left(HOOK_BUDGET), wait, Duration::from_secs(1)),
        Ok(())
    );
    assert_eq!(
        step(
            &left(Duration::from_millis(1_600)),
            wait,
            Duration::from_secs(1)
        ),
        Err(OutOfTime {
            needed: wait + Duration::from_secs(1)
        })
    );
}
