// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The NYSE trading calendar, and when each trading day's Massive bars are refreshed.
//!
//! **Trading days.** A trading day is a weekday that is not an NYSE full-day holiday and not an
//! unscheduled closure. The holidays are computed from rules (NYSE Rule 7.2), valid from 2000:
//!
//! | Holiday | Date | Observed |
//! | -- | -- | -- |
//! | New Year's Day | 1 January | Sunday: the Monday after. Saturday: **not observed** (the Friday before ends the year, so it stays open) |
//! | Martin Luther King Jr. Day | third Monday of January | |
//! | Washington's Birthday | third Monday of February | |
//! | Good Friday | two days before Easter Sunday (Gregorian) | |
//! | Memorial Day | last Monday of May | |
//! | Juneteenth | 19 June, from 2022 | Saturday: the Friday before. Sunday: the Monday after |
//! | Independence Day | 4 July | as Juneteenth |
//! | Labor Day | first Monday of September | |
//! | Thanksgiving | fourth Thursday of November | |
//! | Christmas | 25 December | as Juneteenth |
//!
//! [`UNSCHEDULED_CLOSURES`] lists the full days the exchange closed outside those rules since
//! 2000 (the 11 September 2001 attacks, national days of mourning, Hurricane Sandy). Add a date
//! there when the NYSE announces a new one. Early closes (13:00 on some days around July 4th,
//! Thanksgiving and Christmas) are trading days.
//!
//! Dates before 2000 use the same rules, which don't match every historical closure.
//!
//! **Refresh slot.** Each trading day `d` has one refresh slot, [`refresh_slot`]: 11:30
//! America/New_York on the calendar day after `d` (a Friday's slot is Saturday 11:30). Massive
//! recalculates a day's bars through the session and finalises them the next morning, with its
//! daily files ready about 11:00 New York time; the extra half hour is settle time. The slot is
//! converted to UTC with the tz database, so EST and EDT are both right, including across the
//! clock changes. The scheduler makes Massive series due once per slot (each run also re-fetches
//! the two previous trading days, in the adapter), and [`latest_settled_trading_day`] is the
//! newest day whose bars should be stored. Weekends and holidays aren't trading days, so they
//! add no slot: nothing new is due and the expected day doesn't advance.
//!
//! The rules are a few lines and need no data file, so this uses no calendar crate. A crate
//! would still need the unscheduled closures added by hand. `chrono-tz` supplies the tz database.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc, Weekday};
use chrono_tz::America::New_York;

/// Time zone of the exchange: trading dates and refresh slots are New York local time.
pub const EXCHANGE_TZ: chrono_tz::Tz = New_York;

/// New York local time, on the calendar day after a trading day, when that day's bars are
/// treated as final and fetched: Massive's daily files are ready about 11:00, plus 30 minutes.
pub const REFRESH_TIME: NaiveTime = match NaiveTime::from_hms_opt(11, 30, 0) {
    Some(t) => t,
    None => panic!("valid time"),
};

/// Most trading days [`observation_lag_trading_days`] counts, about a year. Also the lag
/// reported when nothing is stored yet.
pub const MAX_LAG_TRADING_DAYS: i64 = 260;

/// Full-day closures the holiday rules don't produce, since 2000.
pub const UNSCHEDULED_CLOSURES: &[(i32, u32, u32)] = &[
    // The 11 September 2001 attacks.
    (2001, 9, 11),
    (2001, 9, 12),
    (2001, 9, 13),
    (2001, 9, 14),
    // National days of mourning: Presidents Reagan, Ford, G. H. W. Bush and Carter.
    (2004, 6, 11),
    (2007, 1, 2),
    (2018, 12, 5),
    (2025, 1, 9),
    // Hurricane Sandy.
    (2012, 10, 29),
    (2012, 10, 30),
];

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid calendar date")
}

/// The `n`th (1-based) `weekday` of `month`.
fn nth_weekday(year: i32, month: u32, weekday: Weekday, n: u8) -> NaiveDate {
    NaiveDate::from_weekday_of_month_opt(year, month, weekday, n).expect("nth weekday exists")
}

/// The last `weekday` of `month`.
fn last_weekday(year: i32, month: u32, weekday: Weekday) -> NaiveDate {
    let first_of_next = if month == 12 {
        ymd(year + 1, 1, 1)
    } else {
        ymd(year, month + 1, 1)
    };
    let last = first_of_next.pred_opt().expect("not the first date");
    let back = (7 + last.weekday().num_days_from_monday() - weekday.num_days_from_monday()) % 7;
    last - Duration::days(i64::from(back))
}

/// Easter Sunday, Gregorian calendar (the anonymous Gregorian algorithm).
fn easter_sunday(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    ymd(year, month as u32, day as u32)
}

/// A fixed-date holiday moved off the weekend: Saturday to Friday, Sunday to Monday.
fn observed(d: NaiveDate) -> NaiveDate {
    match d.weekday() {
        Weekday::Sat => d - Duration::days(1),
        Weekday::Sun => d + Duration::days(1),
        _ => d,
    }
}

/// The NYSE full-day holidays observed in `year` (weekdays only), in date order.
pub fn holidays(year: i32) -> Vec<NaiveDate> {
    let mut days = Vec::with_capacity(10);
    // New Year's Day on a Saturday isn't observed on the Friday before.
    let new_year = ymd(year, 1, 1);
    match new_year.weekday() {
        Weekday::Sat => {}
        Weekday::Sun => days.push(new_year + Duration::days(1)),
        _ => days.push(new_year),
    }
    days.push(nth_weekday(year, 1, Weekday::Mon, 3));
    days.push(nth_weekday(year, 2, Weekday::Mon, 3));
    days.push(easter_sunday(year) - Duration::days(2));
    days.push(last_weekday(year, 5, Weekday::Mon));
    if year >= 2022 {
        days.push(observed(ymd(year, 6, 19)));
    }
    days.push(observed(ymd(year, 7, 4)));
    days.push(nth_weekday(year, 9, Weekday::Mon, 1));
    days.push(nth_weekday(year, 11, Weekday::Thu, 4));
    days.push(observed(ymd(year, 12, 25)));
    days.sort_unstable();
    days
}

fn is_holiday(d: NaiveDate) -> bool {
    holidays(d.year()).contains(&d)
}

fn is_unscheduled_closure(d: NaiveDate) -> bool {
    UNSCHEDULED_CLOSURES.contains(&(d.year(), d.month(), d.day()))
}

/// Whether the NYSE holds a regular session on `d`: a weekday that is neither a full-day
/// holiday nor an [unscheduled closure](UNSCHEDULED_CLOSURES).
pub fn is_trading_day(d: NaiveDate) -> bool {
    !matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
        && !is_holiday(d)
        && !is_unscheduled_closure(d)
}

/// The last trading day strictly before `d`.
pub fn previous_trading_day(d: NaiveDate) -> NaiveDate {
    let mut day = d.pred_opt().expect("date after the minimum");
    while !is_trading_day(day) {
        day = day.pred_opt().expect("date after the minimum");
    }
    day
}

/// The trading days from `from` to `to`, both inclusive, in order. Empty when `from > to`.
pub fn trading_days(from: NaiveDate, to: NaiveDate) -> Vec<NaiveDate> {
    from.iter_days()
        .take_while(|d| *d <= to)
        .filter(|d| is_trading_day(*d))
        .collect()
}

/// When trading day `d`'s bars are refreshed: [`REFRESH_TIME`] New York time on the calendar day
/// after `d`, as UTC. Defined for any date; only trading days have a slot in the schedule.
pub fn refresh_slot(d: NaiveDate) -> DateTime<Utc> {
    let next = d.succ_opt().expect("date before the maximum");
    // The clocks change at 02:00, never at 11:30, so the local time is unique.
    EXCHANGE_TZ
        .from_local_datetime(&next.and_time(REFRESH_TIME))
        .single()
        .expect("11:30 New York time is unambiguous")
        .with_timezone(&Utc)
}

/// The newest trading day whose [refresh slot](refresh_slot) is at or before `now`: the day whose
/// bars should already be stored.
pub fn latest_settled_trading_day(now: DateTime<Utc>) -> NaiveDate {
    let today = now.with_timezone(&EXCHANGE_TZ).date_naive();
    // The newest candidate is the last trading day before today in New York, whose slot is
    // today at the earliest.
    let day = previous_trading_day(today);
    if refresh_slot(day) > now {
        previous_trading_day(day)
    } else {
        day
    }
}

/// The refresh slot of [`latest_settled_trading_day`]: the most recent slot at or before `now`.
/// A series last refreshed before this is due.
pub fn latest_refresh_slot(now: DateTime<Utc>) -> DateTime<Utc> {
    refresh_slot(latest_settled_trading_day(now))
}

/// The refresh slot before [`latest_refresh_slot`]. A series last refreshed before this missed a
/// whole trading day's refresh and is overdue.
pub fn previous_refresh_slot(now: DateTime<Utc>) -> DateTime<Utc> {
    refresh_slot(previous_trading_day(latest_settled_trading_day(now)))
}

/// How many trading days after `newest` (the newest stored trading day) have had their refresh
/// slot by `now`: 0 when the data is current, 1 when one trading day's bars are missing.
/// Weekends, holidays and closures never add to it. Capped at [`MAX_LAG_TRADING_DAYS`], which is
/// also the lag when nothing is stored (`newest` is `None`).
pub fn observation_lag_trading_days(newest: Option<NaiveDate>, now: DateTime<Utc>) -> i64 {
    let Some(newest) = newest else {
        return MAX_LAG_TRADING_DAYS;
    };
    let mut lag = 0;
    let mut day = latest_settled_trading_day(now);
    while day > newest && lag < MAX_LAG_TRADING_DAYS {
        lag += 1;
        day = previous_trading_day(day);
    }
    lag
}

/// `d` as 00:00 UTC, in Unix seconds: the encoding of the latest-observation gauge.
pub fn date_timestamp_seconds(d: NaiveDate) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let secs = d.and_time(NaiveTime::MIN).and_utc().timestamp() as f64;
    secs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().expect("test date")
    }

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .expect("test timestamp")
            .with_timezone(&Utc)
    }

    /// Weekdays of `year` that aren't trading days.
    fn closed_weekdays(year: i32) -> Vec<NaiveDate> {
        ymd(year, 1, 1)
            .iter_days()
            .take_while(|day| day.year() == year)
            .filter(|day| !matches!(day.weekday(), Weekday::Sat | Weekday::Sun))
            .filter(|day| !is_trading_day(*day))
            .collect()
    }

    fn dates(list: &[&str]) -> Vec<NaiveDate> {
        list.iter().map(|s| d(s)).collect()
    }

    /// Every weekday closure of a year, as the NYSE published it (nyse.com "Holidays & Trading
    /// Hours" and its press releases for the unscheduled closures).
    #[test]
    fn closures_match_published_nyse_lists() {
        let published: &[(i32, &[&str])] = &[
            // 1 January 2000 was a Saturday: not observed.
            (
                2000,
                &[
                    "2000-01-17",
                    "2000-02-21",
                    "2000-04-21",
                    "2000-05-29",
                    "2000-07-04",
                    "2000-09-04",
                    "2000-11-23",
                    "2000-12-25",
                ],
            ),
            (
                2001,
                &[
                    "2001-01-01",
                    "2001-01-15",
                    "2001-02-19",
                    "2001-04-13",
                    "2001-05-28",
                    "2001-07-04",
                    "2001-09-03",
                    "2001-09-11",
                    "2001-09-12",
                    "2001-09-13",
                    "2001-09-14",
                    "2001-11-22",
                    "2001-12-25",
                ],
            ),
            // Reagan's funeral; 4 July a Sunday, Christmas a Saturday.
            (
                2004,
                &[
                    "2004-01-01",
                    "2004-01-19",
                    "2004-02-16",
                    "2004-04-09",
                    "2004-05-31",
                    "2004-06-11",
                    "2004-07-05",
                    "2004-09-06",
                    "2004-11-25",
                    "2004-12-24",
                ],
            ),
            // New Year's Day 2005 a Saturday: 31 December 2004 was a trading day (checked below).
            (
                2005,
                &[
                    "2005-01-17",
                    "2005-02-21",
                    "2005-03-25",
                    "2005-05-30",
                    "2005-07-04",
                    "2005-09-05",
                    "2005-11-24",
                    "2005-12-26",
                ],
            ),
            // Ford's funeral.
            (
                2007,
                &[
                    "2007-01-01",
                    "2007-01-02",
                    "2007-01-15",
                    "2007-02-19",
                    "2007-04-06",
                    "2007-05-28",
                    "2007-07-04",
                    "2007-09-03",
                    "2007-11-22",
                    "2007-12-25",
                ],
            ),
            (
                2010,
                &[
                    "2010-01-01",
                    "2010-01-18",
                    "2010-02-15",
                    "2010-04-02",
                    "2010-05-31",
                    "2010-07-05",
                    "2010-09-06",
                    "2010-11-25",
                    "2010-12-24",
                ],
            ),
            // New Year's Day a Sunday; Hurricane Sandy.
            (
                2012,
                &[
                    "2012-01-02",
                    "2012-01-16",
                    "2012-02-20",
                    "2012-04-06",
                    "2012-05-28",
                    "2012-07-04",
                    "2012-09-03",
                    "2012-10-29",
                    "2012-10-30",
                    "2012-11-22",
                    "2012-12-25",
                ],
            ),
            // 4 July a Saturday.
            (
                2015,
                &[
                    "2015-01-01",
                    "2015-01-19",
                    "2015-02-16",
                    "2015-04-03",
                    "2015-05-25",
                    "2015-07-03",
                    "2015-09-07",
                    "2015-11-26",
                    "2015-12-25",
                ],
            ),
            // Christmas a Sunday.
            (
                2016,
                &[
                    "2016-01-01",
                    "2016-01-18",
                    "2016-02-15",
                    "2016-03-25",
                    "2016-05-30",
                    "2016-07-04",
                    "2016-09-05",
                    "2016-11-24",
                    "2016-12-26",
                ],
            ),
            // G. H. W. Bush's funeral.
            (
                2018,
                &[
                    "2018-01-01",
                    "2018-01-15",
                    "2018-02-19",
                    "2018-03-30",
                    "2018-05-28",
                    "2018-07-04",
                    "2018-09-03",
                    "2018-11-22",
                    "2018-12-05",
                    "2018-12-25",
                ],
            ),
            // No Juneteenth before 2022.
            (
                2021,
                &[
                    "2021-01-01",
                    "2021-01-18",
                    "2021-02-15",
                    "2021-04-02",
                    "2021-05-31",
                    "2021-07-05",
                    "2021-09-06",
                    "2021-11-25",
                    "2021-12-24",
                ],
            ),
            // New Year's Day a Saturday; Juneteenth a Sunday; Christmas a Sunday.
            (
                2022,
                &[
                    "2022-01-17",
                    "2022-02-21",
                    "2022-04-15",
                    "2022-05-30",
                    "2022-06-20",
                    "2022-07-04",
                    "2022-09-05",
                    "2022-11-24",
                    "2022-12-26",
                ],
            ),
            (
                2023,
                &[
                    "2023-01-02",
                    "2023-01-16",
                    "2023-02-20",
                    "2023-04-07",
                    "2023-05-29",
                    "2023-06-19",
                    "2023-07-04",
                    "2023-09-04",
                    "2023-11-23",
                    "2023-12-25",
                ],
            ),
            (
                2024,
                &[
                    "2024-01-01",
                    "2024-01-15",
                    "2024-02-19",
                    "2024-03-29",
                    "2024-05-27",
                    "2024-06-19",
                    "2024-07-04",
                    "2024-09-02",
                    "2024-11-28",
                    "2024-12-25",
                ],
            ),
            // Carter's national day of mourning.
            (
                2025,
                &[
                    "2025-01-01",
                    "2025-01-09",
                    "2025-01-20",
                    "2025-02-17",
                    "2025-04-18",
                    "2025-05-26",
                    "2025-06-19",
                    "2025-07-04",
                    "2025-09-01",
                    "2025-11-27",
                    "2025-12-25",
                ],
            ),
            // 4 July a Saturday.
            (
                2026,
                &[
                    "2026-01-01",
                    "2026-01-19",
                    "2026-02-16",
                    "2026-04-03",
                    "2026-05-25",
                    "2026-06-19",
                    "2026-07-03",
                    "2026-09-07",
                    "2026-11-26",
                    "2026-12-25",
                ],
            ),
            // Juneteenth and Christmas Saturdays, 4 July a Sunday.
            (
                2027,
                &[
                    "2027-01-01",
                    "2027-01-18",
                    "2027-02-15",
                    "2027-03-26",
                    "2027-05-31",
                    "2027-06-18",
                    "2027-07-05",
                    "2027-09-06",
                    "2027-11-25",
                    "2027-12-24",
                ],
            ),
        ];
        for (year, list) in published {
            assert_eq!(closed_weekdays(*year), dates(list), "{year}");
        }
    }

    /// Published counts of NYSE trading days per year.
    #[test]
    fn trading_day_counts_per_year() {
        for (year, count) in [
            (2001, 248),
            (2012, 250),
            (2021, 252),
            (2022, 251),
            (2023, 250),
            (2024, 252),
            (2025, 250),
        ] {
            assert_eq!(
                trading_days(ymd(year, 1, 1), ymd(year, 12, 31)).len(),
                count,
                "{year}"
            );
        }
    }

    #[test]
    fn saturday_new_year_leaves_the_friday_before_open() {
        for day in ["2004-12-31", "2010-12-31", "2021-12-31", "2027-12-31"] {
            assert!(is_trading_day(d(day)), "{day}");
        }
    }

    #[test]
    fn juneteenth_starts_in_2022() {
        assert!(is_trading_day(d("2021-06-18")));
        assert!(!is_trading_day(d("2022-06-20")));
        assert!(!is_trading_day(d("2023-06-19")));
    }

    #[test]
    fn every_unscheduled_closure_is_a_weekday_and_closed() {
        for &(y, m, day) in UNSCHEDULED_CLOSURES {
            let date = ymd(y, m, day);
            assert!(
                !matches!(date.weekday(), Weekday::Sat | Weekday::Sun),
                "{date}"
            );
            assert!(!is_holiday(date), "{date} is already a holiday");
            assert!(!is_trading_day(date), "{date}");
        }
    }

    #[test]
    fn easter_dates() {
        for (year, easter) in [
            (2000, "2000-04-23"),
            (2008, "2008-03-23"),
            (2011, "2011-04-24"),
            (2019, "2019-04-21"),
            (2038, "2038-04-25"),
        ] {
            assert_eq!(easter_sunday(year), d(easter), "{year}");
        }
    }

    #[test]
    fn previous_trading_day_skips_weekends_holidays_and_closures() {
        // Monday to Friday.
        assert_eq!(previous_trading_day(d("2024-07-08")), d("2024-07-05"));
        // Tuesday after a Monday holiday to Friday.
        assert_eq!(previous_trading_day(d("2024-09-03")), d("2024-08-30"));
        // Monday after Good Friday to Thursday.
        assert_eq!(previous_trading_day(d("2026-04-06")), d("2026-04-02"));
        // Hurricane Sandy: Wednesday 31 October 2012 back to Friday the 26th.
        assert_eq!(previous_trading_day(d("2012-10-31")), d("2012-10-26"));
        // 17 September 2001 back to 10 September.
        assert_eq!(previous_trading_day(d("2001-09-17")), d("2001-09-10"));
        // Strictly before, even from a trading day.
        assert_eq!(previous_trading_day(d("2024-07-03")), d("2024-07-02"));
        // From a weekend.
        assert_eq!(previous_trading_day(d("2024-07-07")), d("2024-07-05"));
    }

    #[test]
    fn trading_days_is_inclusive_and_ordered() {
        assert_eq!(
            trading_days(d("2012-10-25"), d("2012-11-01")),
            dates(&["2012-10-25", "2012-10-26", "2012-10-31", "2012-11-01"])
        );
        assert_eq!(trading_days(d("2024-07-04"), d("2024-07-04")), vec![]);
        assert_eq!(
            trading_days(d("2024-07-05"), d("2024-07-05")),
            dates(&["2024-07-05"])
        );
        assert!(trading_days(d("2024-07-10"), d("2024-07-01")).is_empty());
    }

    #[test]
    fn refresh_slot_follows_new_york_daylight_saving() {
        // Friday 6 March 2026: Saturday 11:30 EST (UTC-5) is 16:30 UTC.
        assert_eq!(refresh_slot(d("2026-03-06")), utc("2026-03-07T16:30:00Z"));
        // The clocks go forward on Sunday 8 March: Monday's slot is Tuesday 11:30 EDT (UTC-4).
        assert_eq!(refresh_slot(d("2026-03-09")), utc("2026-03-10T15:30:00Z"));
        // Back to EST on Sunday 1 November 2026.
        assert_eq!(refresh_slot(d("2026-10-30")), utc("2026-10-31T15:30:00Z"));
        assert_eq!(refresh_slot(d("2026-11-02")), utc("2026-11-03T16:30:00Z"));
    }

    #[test]
    fn latest_settled_trading_day_waits_for_the_next_morning() {
        // Tuesday 10 March 2026 (EDT): Monday settles at 15:30 UTC.
        assert_eq!(
            latest_settled_trading_day(utc("2026-03-10T15:29:59Z")),
            d("2026-03-06")
        );
        assert_eq!(
            latest_settled_trading_day(utc("2026-03-10T15:30:00Z")),
            d("2026-03-09")
        );
        // Friday 6 March (EST) settles on Saturday at 16:30 UTC, and stays the latest over the
        // weekend of the clock change until Monday's slot on Tuesday morning.
        assert_eq!(
            latest_settled_trading_day(utc("2026-03-07T16:29:00Z")),
            d("2026-03-05")
        );
        for now in [
            "2026-03-07T16:30:00Z",
            "2026-03-08T06:59:00Z",
            "2026-03-08T07:00:00Z",
            "2026-03-09T23:59:00Z",
            "2026-03-10T15:29:00Z",
        ] {
            assert_eq!(
                latest_settled_trading_day(utc(now)),
                d("2026-03-06"),
                "{now}"
            );
        }
        // 03:00 UTC on Tuesday 10 November is still Monday evening in New York, so Monday's slot
        // (Tuesday 11:30) hasn't come: the latest is Friday 6 November.
        assert_eq!(
            latest_settled_trading_day(utc("2026-11-10T03:00:00Z")),
            d("2026-11-06")
        );
    }

    #[test]
    fn latest_settled_trading_day_over_hurricane_sandy() {
        // Friday 26 October 2012 settles on Saturday and stays the latest until Wednesday 31
        // October's slot, Thursday 1 November 11:30 EDT.
        for now in [
            "2012-10-27T15:30:00Z",
            "2012-10-29T21:00:00Z",
            "2012-10-31T15:30:00Z",
            "2012-11-01T15:29:00Z",
        ] {
            assert_eq!(
                latest_settled_trading_day(utc(now)),
                d("2012-10-26"),
                "{now}"
            );
        }
        let now = utc("2012-11-01T15:30:00Z");
        assert_eq!(latest_settled_trading_day(now), d("2012-10-31"));
        assert_eq!(previous_refresh_slot(now), refresh_slot(d("2012-10-26")));
    }

    #[test]
    fn refresh_slots_bracket_good_friday() {
        // Good Friday 2026 (3 April): Thursday's slot is Friday morning, and the next one, for
        // Monday, is Tuesday morning. In between, the latest slot is Thursday's and the previous
        // one Wednesday's.
        let thursday = refresh_slot(d("2026-04-02"));
        assert_eq!(thursday, utc("2026-04-03T15:30:00Z"));
        for now in [
            thursday,
            utc("2026-04-05T12:00:00Z"),
            utc("2026-04-07T15:29:00Z"),
        ] {
            assert_eq!(latest_refresh_slot(now), thursday);
            assert_eq!(previous_refresh_slot(now), refresh_slot(d("2026-04-01")));
        }
    }

    #[test]
    fn lag_over_good_friday() {
        let newest = Some(d("2026-04-02"));
        // Thursday's bars are stored; nothing more is expected until Tuesday 7 April 11:30 EDT.
        for now in [
            "2026-04-03T16:00:00Z",
            "2026-04-06T12:00:00Z",
            "2026-04-07T15:29:00Z",
        ] {
            assert_eq!(observation_lag_trading_days(newest, utc(now)), 0, "{now}");
        }
        assert_eq!(
            observation_lag_trading_days(newest, utc("2026-04-07T15:30:00Z")),
            1
        );
        // Only Wednesday stored over the long weekend: one missed trading day, not four days.
        assert_eq!(
            observation_lag_trading_days(Some(d("2026-04-01")), utc("2026-04-06T12:00:00Z")),
            1
        );
    }

    #[test]
    fn lag_over_the_sandy_closure() {
        let newest = Some(d("2012-10-26"));
        assert_eq!(
            observation_lag_trading_days(newest, utc("2012-10-31T20:00:00Z")),
            0
        );
        assert_eq!(
            observation_lag_trading_days(newest, utc("2012-11-01T15:30:00Z")),
            1
        );
        assert_eq!(
            observation_lag_trading_days(newest, utc("2012-11-02T15:30:00Z")),
            2
        );
    }

    #[test]
    fn lag_across_a_clock_change() {
        // Friday 30 October 2026 stored; the clocks go back on Sunday 1 November. Monday 2
        // November's slot is Tuesday 11:30 EST, 16:30 UTC (it would be 15:30 UTC in EDT).
        let newest = Some(d("2026-10-30"));
        assert_eq!(
            observation_lag_trading_days(newest, utc("2026-11-03T15:45:00Z")),
            0
        );
        assert_eq!(
            observation_lag_trading_days(newest, utc("2026-11-03T16:30:00Z")),
            1
        );
    }

    #[test]
    fn lag_with_nothing_stored_long_gaps_and_early_data() {
        let now = utc("2026-11-09T12:00:00Z");
        assert_eq!(
            observation_lag_trading_days(None, now),
            MAX_LAG_TRADING_DAYS
        );
        assert_eq!(
            observation_lag_trading_days(Some(d("2000-01-03")), now),
            MAX_LAG_TRADING_DAYS
        );
        // A bar stored before its slot (an early run) isn't negative lag.
        assert_eq!(observation_lag_trading_days(Some(d("2026-11-09")), now), 0);
    }

    #[test]
    fn date_timestamps_are_midnight_utc() {
        assert_eq!(date_timestamp_seconds(d("1970-01-02")), 86_400.0);
        assert_eq!(date_timestamp_seconds(d("2026-11-09")), 1_794_182_400.0);
    }
}
