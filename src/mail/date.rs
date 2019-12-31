use std::convert::TryInto;

enum DateParseState {
    Date,
    Month,
    Year,
    Hour,
    Minute,
    Second,
    Timezone,
}

fn days_in_month(month: u64, year: u64) -> u64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 400) == 0 {
                29
            } else if (year % 100) == 0 {
                28
            } else if (year % 4) == 0 {
                29
            } else {
                28
            }
        }
        _ => panic!("Invalid month: {}", month),
    }
}

fn seconds_to_date(year: u64, month: u64, day: u64) -> Result<u64, &'static str> {
    assert!(year >= 1970, "Invalid year: {}", year);
    assert!(month >= 1 && month <= 12, "Invalid month: {}", month);
    assert!(day >= 1, "Invalid day: {}", day);
    assert!(day <= days_in_month(month, year), "Invalid day in month: {}", day);

    // we assume that we operate on unix ts.
    // Unix ts does not care about leap seconds or stuff like that.
    let month = month - 1;
    let mut result: u64 = 0;
    for y in 1970..2001 {
        if y == year {
            break;
        }
        result += 86400 * 365;
        if (y % 4) == 0 {
            result += 86400;
        }
    }
    let mut y = 2001;
    while y < year {
        if year - y >= 400 {
            result += (86400 * 365 * 400) + (86400 * 97);
            y += 400;
            continue;
        }
        if year - y >= 100 {
            result += (86400 * 365 * 100) + (86400 * 24);
            y += 100;
            continue;
        }
        if year - y >= 4 {
            result += (86400 * 365 * 4) + (86400);
            y += 4;
            continue;
        }
        result += 86400 * 365;
        y += 1;
    }
    for m in 0..month {
        result += u64::from(86400 * days_in_month(m + 1, year));
    }
    result += u64::from(86400 * (day - 1));
    let r: u64 = result.try_into().map_err(|_| "Target number does not fit in u64")?;
    Ok(r)
}

// shamelessly stolen(and modified) from:
// https://github.com/staktrace/mailparse
/// parse_date tries to parse date in some way which makes some sense
pub fn parse_date(date: &str) -> Result<u64, &'static str> {
    let mut result = 0;
    let mut month = 0u32;
    let mut day_of_month = 0;
    let mut state = DateParseState::Date;
    for tok in date.split(|c| c == ' ' || c == ':') {
        if tok.is_empty() {
            continue;
        }
        match state {
            DateParseState::Date => {
                if let Ok(v) = tok.parse::<u8>() {
                    day_of_month = v;
                    state = DateParseState::Month;
                };
                continue;
            }
            DateParseState::Month => {
                month = match tok.to_uppercase().as_str() {
                    "JAN" | "JANUARY" => 1,
                    "FEB" | "FEBRUARY" => 2,
                    "MAR" | "MARCH" => 3,
                    "APR" | "APRIL" => 4,
                    "MAY" => 5,
                    "JUN" | "JUNE" => 6,
                    "JUL" | "JULY" => 7,
                    "AUG" | "AUGUST" => 8,
                    "SEP" | "SEPTEMBER" => 9,
                    "OCT" | "OCTOBER" => 10,
                    "NOV" | "NOVEMBER" => 11,
                    "DEC" | "DECEMBER" => 12,
                    _ => return Err("Unrecognized month"),
                };
                state = DateParseState::Year;
                continue;
            }
            DateParseState::Year => {
                let year = match tok.parse::<u16>() {
                    Ok(v) if v < 70 => 2000 + v,
                    Ok(v) if v < 100 => 1900 + v,
                    Ok(v) if v < 1970 => return Err("Disallowed year(can't be expressed as unix timestamp"),
                    Ok(v) => v,
                    Err(_) => return Err("Invalid year"),
                };
                // eprintln!("DIM: {} DOM: {}", days_in_month(u64::from(month), u64::from(year)), day_of_month);
                if day_of_month < 1 || u64::from(day_of_month) > days_in_month(u64::from(month), u64::from(year)) {
                    return Err("Invalid day of month");
                }
                result = seconds_to_date(u64::from(year), u64::from(month), u64::from(day_of_month))?;
                state = DateParseState::Hour;
                continue;
            }
            DateParseState::Hour => {
                let hour = match tok.parse::<u8>() {
                    Ok(v) => v,
                    Err(_) => return Err("Invalid hour"),
                };
                result += 3600 * u64::from(hour);
                state = DateParseState::Minute;
                continue;
            }
            DateParseState::Minute => {
                let minute = match tok.parse::<u8>() {
                    Ok(v) => v,
                    Err(_) => return Err("Invalid minute"),
                };
                result += 60 * u64::from(minute);
                state = DateParseState::Second;
                continue;
            }
            DateParseState::Second => {
                let second = match tok.parse::<u8>() {
                    Ok(v) => v,
                    Err(_) => return Err("Invalid second"),
                };
                result += u64::from(second);
                state = DateParseState::Timezone;
                continue;
            }
            DateParseState::Timezone => {
                let (tz, tz_sign) = match tok.parse::<i32>() {
                    Ok(v) if v < 0 => {
                        if v == std::i32::MIN {
                            return Err("Invalid value for timezone");
                        }
                        (-v, -1)
                    } // it's int overflow when v == std::i32::min
                    Ok(v) => (v, 1),
                    Err(_) => {
                        match tok.to_uppercase().as_str() {
                            // This list taken from IETF RFC 822
                            "UTC" | "UT" | "GMT" | "Z" => (0, 1),
                            "EDT" => (400, -1),
                            "EST" | "CDT" => (500, -1),
                            "CST" | "MDT" => (600, -1),
                            "MST" | "PDT" => (700, -1),
                            "PST" => (800, -1),
                            "A" => (100, -1),
                            "M" => (1200, -1),
                            "N" => (100, 1),
                            "Y" => (1200, 1),
                            _ => return Err("Invalid timezone"),
                        }
                    }
                };
                let tz_hours = i64::from(tz / 100);
                let tz_mins = i64::from(tz % 100);
                let tz_delta = (tz_hours * 3600) + (tz_mins * 60);
                if tz_sign < 0 {
                    result += tz_delta as u64;
                } else {
                    if tz_delta as u64 > result {
                        return Err("Invalid time and tz delta");
                    }
                    result -= tz_delta as u64;
                }
                break;
            }
        }
    }
    Ok(result)
}
/*
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Date {
    unix_timestamp: u64,
    timezone_shift: i32,
}

impl FromStr for Date {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {

    }
}
*/


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dates() {

        // fuzzer cases that crash/give < 0 value(is second one intended?)
        // let res = parse_date("0 JANUARY 70").unwrap();
        // assert!(res >= 0);
        // let _ = parse_date("Fri, 31 Dec 2400 00:00:00 +0000111111111");

        assert_eq!(
            parse_date("Sun, 25 Sep 2016 18:36:33 -0400").unwrap(),
            1474842993
        );
        assert_eq!(
            parse_date("Fri, 01 Jan 2100 11:12:13 +0000").unwrap(),
            4102485133
        );
        assert_eq!(
            parse_date("Fri, 31 Dec 2100 00:00:00 +0000").unwrap(),
            4133894400
        );
        assert_eq!(
            parse_date("Fri, 31 Dec 2399 00:00:00 +0000").unwrap(),
            13569379200
        );
        assert_eq!(
            parse_date("Fri, 31 Dec 2400 00:00:00 +0000").unwrap(),
            13601001600
        );
        assert_eq!(
            parse_date("17 Sep 2016 16:05:38 -1000").unwrap(),
            1474164338
        );
        assert_eq!(
            parse_date("Fri, 30 Nov 2012 20:57:23 GMT").unwrap(),
            1354309043
        );
    }
}