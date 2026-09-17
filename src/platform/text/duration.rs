use chrono::Duration;

pub fn phrase(duration: Duration) -> String {
    if duration.is_zero() {
        return String::from("permanent");
    }

    let days = duration.num_days();
    let hours = duration.num_hours();
    let minutes = duration.num_minutes();

    let (amount, unit) = match () {
        _ if days >= 365 && days % 365 == 0 => (days / 365, "year"),
        _ if days >= 30 && days % 30 == 0 => (days / 30, "month"),
        _ if days != 0 => (days, "day"),
        _ if hours != 0 => (hours, "hour"),
        _ if minutes != 0 => (minutes, "minute"),
        _ => (duration.num_seconds(), "second"),
    };

    if amount == 1 {
        return format!("{amount} {unit}");
    }

    format!("{amount} {unit}s")
}

pub fn precise(duration: Duration) -> String {
    let units = [
        (duration.num_days(), "day"),
        (duration.num_hours() % 24, "hour"),
        (duration.num_minutes() % 60, "minute"),
        (duration.num_seconds() % 60, "second"),
    ];

    let mut out = String::new();

    for (amount, unit) in units {
        if amount == 0 {
            continue;
        }

        if !out.is_empty() {
            out.push_str(", ");
        }

        out.push_str(&format!("{amount} {unit}"));

        if amount != 1 {
            out.push('s');
        }
    }

    if out.is_empty() {
        return String::from("0 seconds");
    }

    out
}

pub fn compact(duration: Duration) -> String {
    let seconds = duration.num_seconds();

    if seconds == 0 {
        return String::from("0");
    }

    let (amount, unit) = match () {
        _ if seconds % 86_400 == 0 => (seconds / 86_400, 'd'),
        _ if seconds % 3_600 == 0 => (seconds / 3_600, 'h'),
        _ if seconds % 60 == 0 => (seconds / 60, 'm'),
        _ => (seconds, 's'),
    };

    format!("{amount}{unit}")
}

pub fn parse(input: &str) -> Option<Duration> {
    if input.chars().all(|c| c.is_ascii_digit()) {
        return input.parse::<i64>().ok().map(Duration::seconds);
    }

    let mut chars = input.chars();
    let unit = chars.next_back()?;

    if !"smhdwMy".contains(unit) {
        return None;
    }

    let amount = chars.as_str().parse::<i64>().ok()?;

    match unit {
        's' => Some(Duration::seconds(amount)),
        'm' => Some(Duration::minutes(amount)),
        'h' => Some(Duration::hours(amount)),
        'd' => Some(Duration::days(amount)),
        'w' => Some(Duration::weeks(amount)),
        'M' => Some(Duration::days(amount * 30)),
        'y' => Some(Duration::days(amount * 365)),
        _ => None,
    }
}

pub fn words(amount: &str, unit: &str) -> Option<Duration> {
    let unit = unit.to_lowercase();
    let unit = unit.strip_suffix('s').unwrap_or(&unit);

    let initial = match unit {
        "second" => 's',
        "minute" => 'm',
        "hour" => 'h',
        "day" => 'd',
        "week" => 'w',
        "month" => 'M',
        "year" => 'y',
        _ => return None,
    };

    parse(&format!("{amount}{initial}"))
}
