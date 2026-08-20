pub const LEGACY_RESTORE_DEADLINE: i64 = 1_798_761_599; // 2026-12-31 23:59:59 UTC
/// 01.12.2026 00:00:00 по Москве (UTC+3), граница не включается.
pub const LEGACY_REQUEST_DEADLINE: i64 = 1_796_072_400;

pub fn legacy_requests_open(now: i64) -> bool {
    now < LEGACY_REQUEST_DEADLINE
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * adjusted_month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn year_at(epoch: i64) -> i64 {
    let z = epoch.div_euclid(86_400) + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    year
}

pub fn start_of_december(year: i64) -> i64 {
    days_from_civil(year, 12, 1) * 86_400
}

pub fn end_of_year(year: i64) -> i64 {
    days_from_civil(year + 1, 1, 1) * 86_400 - 1
}

pub fn legacy_renewal_target(now: i64, current_expiry: Option<i64>) -> i64 {
    let current_year = year_at(now);
    let target_year = current_expiry
        .map(year_at)
        .map(|year| year + 1)
        .unwrap_or(current_year + 1)
        .max(current_year);
    end_of_year(target_year)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_legacy_calendar_boundaries() {
        assert_eq!(year_at(1_766_016_000), 2025);
        assert_eq!(end_of_year(2026), LEGACY_RESTORE_DEADLINE);
        assert_eq!(start_of_december(2026), 1_796_083_200);
        assert!(legacy_requests_open(LEGACY_REQUEST_DEADLINE - 1));
        assert!(!legacy_requests_open(LEGACY_REQUEST_DEADLINE));
        assert_eq!(
            legacy_renewal_target(1_796_083_200, Some(LEGACY_RESTORE_DEADLINE)),
            1_830_297_599
        );
        assert_eq!(
            legacy_renewal_target(1_799_000_000, Some(LEGACY_RESTORE_DEADLINE)),
            1_830_297_599
        );
    }
}
