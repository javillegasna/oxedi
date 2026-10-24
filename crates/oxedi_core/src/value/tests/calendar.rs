use super::super::{days_from_civil, seconds_of_day, time_of_day};

#[test]
fn the_epoch_and_a_leap_day_are_found() {
    assert_eq!(days_from_civil(1970, 1, 1), Some(0));
    assert_eq!(days_from_civil(2024, 2, 29), Some(19_782));
}

#[test]
fn a_date_that_does_not_exist_is_none() {
    assert_eq!(days_from_civil(2023, 2, 29), None);
    assert_eq!(days_from_civil(0, 1, 1), None);
    assert_eq!(days_from_civil(10_000, 1, 1), None);
    assert_eq!(days_from_civil(2024, 13, 1), None);
    assert_eq!(days_from_civil(2024, 0, 1), None);
    assert_eq!(days_from_civil(2024, 4, 31), None);
    assert_eq!(days_from_civil(2024, 4, 0), None);
}

#[test]
fn seconds_of_the_day_stop_before_midnight() {
    assert_eq!(seconds_of_day(0, 0, 0), Some(0));
    assert_eq!(seconds_of_day(23, 59, 59), Some(86_399));
    assert_eq!(seconds_of_day(24, 0, 0), None);
    assert_eq!(seconds_of_day(0, 60, 0), None);
    assert_eq!(seconds_of_day(0, 0, 60), None);
    assert_eq!(seconds_of_day(-1, 0, 0), None);
}

#[test]
fn a_time_of_day_is_below_one_day() {
    assert_eq!(time_of_day(0), Some(0));
    assert_eq!(time_of_day(86_399), Some(86_399));
    assert_eq!(time_of_day(86_400), None);
    assert_eq!(time_of_day(-1), None);
}
