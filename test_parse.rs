fn parse_time(time_str: &str) -> Option<i32> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() == 2 {
        let hours: i32 = parts[0].parse().ok()?;
        let minutes: i32 = parts[1].parse().ok()?;
        Some(hours * 60 + minutes)
    } else {
        None
    }
}

fn format_time(minutes: i32) -> String {
    let sign = if minutes < 0 { "-" } else { "" };
    let abs_minutes = minutes.abs();
    let h = abs_minutes / 60;
    let m = abs_minutes % 60;
    format!("{}{}:{:02}", sign, h, m)
}

fn main() {
    let s = "7:30";
    let t = "8:00";
    let sm = parse_time(s).unwrap();
    let tm = parse_time(t).unwrap();
    println!("spent: {}, total: {}, left: {}", sm, tm, format_time(tm - sm));
    
    let s2 = "8:30";
    let tm2 = parse_time(t).unwrap();
    let sm2 = parse_time(s2).unwrap();
    println!("spent: {}, total: {}, left: {}", sm2, tm2, format_time(tm2 - sm2));
}
