use chrono::{Datelike, NaiveDate};

const MONTHS: [(&str, u32); 24] = [
    ("JAN", 1), ("JANV", 1), ("FEB", 2), ("FEV", 2), ("FEVR", 2), ("MAR", 3), ("MARS", 3),
    ("APR", 4), ("AVR", 4), ("AVRIL", 4), ("MAY", 5), ("MAI", 5), ("JUN", 6), ("JUIN", 6),
    ("JUL", 7), ("JUIL", 7), ("AUG", 8), ("AOU", 8), ("AOUT", 8), ("SEP", 9), ("SEPT", 9),
    ("OCT", 10), ("NOV", 11), ("DEC", 12),
];

const KEYWORDS: [&str; 14] = [
    "EXP", "BBE", "BBD", "DLC", "DLUO", "MHD", "BEST BEFORE", "USE BY", "A CONSOMMER",
    "CONSOMMER", "AVANT", "UTILISER", "PERIME", "VERW",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub date: NaiveDate,
    pub score: i32,
    pub source: String,
}

fn last_day(year: i32, month: u32) -> Option<NaiveDate> {
    let (y, m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(y, m, 1)?.pred_opt()
}

fn year4(y: i32) -> i32 {
    if y < 100 { 2000 + y } else { y }
}

fn plausible(d: NaiveDate, today: NaiveDate) -> bool {
    let y = d.year();
    y >= today.year() - 2 && y <= today.year() + 15
}

fn fix_digits(c: char) -> Option<char> {
    match c {
        '0'..='9' => Some(c),
        'O' | 'o' | 'Q' | 'D' => Some('0'),
        'I' | 'l' | '|' | 'i' | 'L' => Some('1'),
        'S' | 's' => Some('5'),
        'B' => Some('8'),
        'Z' | 'z' => Some('2'),
        'T' => Some('7'),
        _ => None,
    }
}

fn is_sep(c: char) -> bool {
    matches!(c, '.' | '/' | '-' | ' ' | ',' | ':' | '\'' | '_')
}

fn year_ok(p: &str) -> bool {
    p.len() == 2 || p.len() == 4
}

fn parse_numeric(run: &str, today: NaiveDate) -> Vec<(NaiveDate, i32)> {
    if run.contains(':') && run.chars().filter(|c| matches!(c, '.' | '/' | '-')).count() == 0 {
        return Vec::new();
    }
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in run.chars() {
        if is_sep(c) {
            if !cur.is_empty() {
                parts.push(std::mem::take(&mut cur));
            }
        } else if let Some(d) = fix_digits(c) {
            cur.push(d);
        } else {
            return Vec::new();
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    let mut out: Vec<(NaiveDate, i32)> = Vec::new();
    let num = |s: &str| s.parse::<i32>().unwrap_or(-1);
    let ymd = |y: i32, m: i32, d: i32| {
        if m < 1 || d < 1 {
            None
        } else {
            NaiveDate::from_ymd_opt(y, m as u32, d as u32)
        }
    };
    let my = |y: i32, m: i32| if m < 1 { None } else { last_day(y, m as u32) };
    match parts.len() {
        1 => {
            let p = &parts[0];
            match p.len() {
                8 => {
                    out.extend(ymd(num(&p[4..8]), num(&p[2..4]), num(&p[0..2])).map(|d| (d, 0)));
                    out.extend(ymd(num(&p[0..4]), num(&p[4..6]), num(&p[6..8])).map(|d| (d, 3)));
                }
                6 => {
                    out.extend(ymd(year4(num(&p[4..6])), num(&p[2..4]), num(&p[0..2])).map(|d| (d, 0)));
                    out.extend(my(num(&p[2..6]), num(&p[0..2])).map(|d| (d, 4)));
                }
                4 => {
                    out.extend(my(year4(num(&p[2..4])), num(&p[0..2])).map(|d| (d, 8)));
                }
                _ => {}
            }
        }
        2 => {
            let (pa, pb) = (&parts[0], &parts[1]);
            let (a, b) = (num(pa), num(pb));
            if pa.len() == 4 {
                out.extend(my(a, b).map(|d| (d, 0)));
            } else if pb.len() == 4 {
                out.extend(my(b, a).map(|d| (d, 0)));
            } else if pa.len() <= 2 && pb.len() <= 2 {
                out.extend(my(year4(b), a).map(|d| (d, 4)));
            }
        }
        3 => {
            let (pa, pb, pc) = (&parts[0], &parts[1], &parts[2]);
            let (a, b, c) = (num(pa), num(pb), num(pc));
            if pa.len() == 4 && pb.len() <= 2 && pc.len() <= 2 {
                out.extend(ymd(a, b, c).map(|d| (d, 0)));
            } else if pa.len() <= 2 && pb.len() <= 2 && year_ok(pc) {
                out.extend(ymd(year4(c), b, a).map(|d| (d, 0)));
                if a != b {
                    out.extend(ymd(year4(c), a, b).map(|d| (d, 4)));
                }
            }
        }
        _ => {}
    }
    out.retain(|(d, _)| plausible(*d, today));
    out
}

fn month_from_word(w: &str) -> Option<u32> {
    MONTHS.iter().find(|(n, _)| *n == w).map(|(_, m)| *m)
}

fn parse_named(text: &str, today: NaiveDate) -> Vec<NaiveDate> {
    let toks: Vec<String> = text
        .to_uppercase()
        .replace(['É', 'È'], "E")
        .replace('Û', "U")
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect();
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let (word, glued_day, glued_year) = split_glued(t);
        let Some(month) = month_from_word(&word) else { continue };
        let day = glued_day.or_else(|| i.checked_sub(1).and_then(|j| toks[j].parse::<u32>().ok()).filter(|d| (1..=31).contains(d)));
        let year = glued_year
            .or_else(|| toks.get(i + 1).and_then(|n| n.parse::<i32>().ok()).filter(|y| (*y >= 100 && *y < 10000) || (0..100).contains(y)));
        if let Some(y) = year {
            let y = year4(y);
            match day {
                Some(d) => out.extend(NaiveDate::from_ymd_opt(y, month, d)),
                None => out.extend(last_day(y, month)),
            }
        }
    }
    out.retain(|d| plausible(*d, today));
    out
}

fn split_glued(t: &str) -> (String, Option<u32>, Option<i32>) {
    let lead: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let rest = &t[lead.len()..];
    let word: String = rest.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let tail = &rest[word.len()..];
    let day = lead.parse::<u32>().ok().filter(|d| (1..=31).contains(d));
    let year = tail.parse::<i32>().ok();
    (word, day, year)
}

fn numeric_runs(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut runs = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if fix_digits(chars[i]).is_some() && chars[i].is_ascii_digit() {
            let mut j = i;
            while j < chars.len() && (fix_digits(chars[j]).is_some() || is_sep(chars[j])) {
                j += 1;
            }
            let run: String = chars[i..j].iter().collect();
            runs.push(run.trim().to_string());
            i = j;
        } else {
            i += 1;
        }
    }
    runs
}

fn split_run_variants(run: &str) -> Vec<String> {
    let mut v = vec![run.to_string()];
    let words: Vec<&str> = run.split(' ').filter(|w| !w.is_empty()).collect();
    if words.len() > 1 {
        for n in (1..=3.min(words.len())).rev() {
            for w in words.windows(n) {
                v.push(w.join(" "));
            }
        }
    }
    v
}

pub fn extract(text: &str, today: NaiveDate) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for (li, line) in lines.iter().enumerate() {
        let upper = line.to_uppercase().replace(['É', 'È'], "E");
        let kw = KEYWORDS.iter().any(|k| upper.contains(k))
            || li.checked_sub(1).is_some_and(|p| {
                let prev = lines[p].to_uppercase().replace(['É', 'È'], "E");
                KEYWORDS.iter().any(|k| prev.contains(k))
            });
        for run in numeric_runs(line) {
            for variant in split_run_variants(&run) {
                for (d, pen) in parse_numeric(&variant, today) {
                    let mut score = 10 - pen;
                    if kw {
                        score += 20;
                    }
                    if variant.chars().filter(|c| is_sep(*c)).count() > 0 {
                        score += 5;
                    }
                    if variant == run {
                        score += 3;
                    }
                    out.push(Candidate { date: d, score, source: variant.clone() });
                }
            }
        }
        for d in parse_named(line, today) {
            out.push(Candidate { date: d, score: 25 + if kw { 20 } else { 0 }, source: (*line).to_string() });
        }
    }
    out.sort_by(|a, b| b.score.cmp(&a.score).then(b.date.cmp(&a.date)));
    let mut seen = Vec::new();
    out.retain(|c| {
        if seen.contains(&c.date) {
            false
        } else {
            seen.push(c.date);
            true
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 8).unwrap()
    }

    fn best(text: &str) -> Option<NaiveDate> {
        extract(text, today()).first().map(|c| c.date)
    }

    fn d(y: i32, m: u32, day: u32) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(y, m, day)
    }

    #[test]
    fn numeric_formats() {
        assert_eq!(best("15.11.2026"), d(2026, 11, 15));
        assert_eq!(best("15/11/26"), d(2026, 11, 15));
        assert_eq!(best("2026-11-15"), d(2026, 11, 15));
        assert_eq!(best("15112026"), d(2026, 11, 15));
        assert_eq!(best("151126"), d(2026, 11, 15));
        assert_eq!(best("11.2026"), d(2026, 11, 30));
        assert_eq!(best("EXP 11/26"), d(2026, 11, 30));
        assert_eq!(best("12/2028"), d(2028, 12, 31));
    }

    fn score_of(text: &str, date: Option<NaiveDate>) -> Option<i32> {
        extract(text, today()).iter().find(|c| Some(c.date) == date).map(|c| c.score)
    }

    #[test]
    fn short_forms_are_weak_but_kept() {
        assert_eq!(best("11/26"), d(2026, 11, 30));
        assert_eq!(best("12/28"), d(2028, 12, 31));
        assert_eq!(best("223\n1.27"), d(2027, 1, 31));
        assert!(score_of("223\n1.27", d(2027, 1, 31)).unwrap() < score_of("15.03.2027", d(2027, 3, 15)).unwrap());
        assert_eq!(best("EXP 15.03.2027\n1.27"), d(2027, 3, 15));
    }

    #[test]
    fn impossible_readings_are_eliminated() {
        assert_eq!(best("31.02.2027"), None);
        assert_eq!(score_of("12.27", d(2026, 12, 12)), None);
        assert_eq!(score_of("12.27", d(2027, 12, 31)).is_some(), true);
        assert_eq!(score_of("11.26", d(2026, 11, 30)).is_some(), true);
    }

    #[test]
    fn ambiguity_keeps_several_entries() {
        let c = extract("05.06.27", today());
        let dates: Vec<_> = c.iter().map(|c| c.date).collect();
        assert_eq!(dates[0], d(2027, 6, 5).unwrap());
        assert!(dates.contains(&d(2027, 5, 6).unwrap()));
        let c = extract("12/2028", today());
        assert_eq!(c[0].date, d(2028, 12, 31).unwrap());
    }

    #[test]
    fn keyword_wins() {
        assert_eq!(best("LOT 120425\nEXP 15.03.2027"), d(2027, 3, 15));
        assert_eq!(best("A consommer de preference avant le 20.08.2027\nL4125"), d(2027, 8, 20));
    }

    #[test]
    fn ocr_confusions() {
        assert_eq!(best("EXP 1S.1l.2O26"), d(2026, 11, 15));
        assert_eq!(best("BBE O1/12/2026"), d(2026, 12, 1));
    }

    #[test]
    fn named_months() {
        assert_eq!(best("BBE 15 NOV 2026"), d(2026, 11, 15));
        assert_eq!(best("EXP 15NOV26"), d(2026, 11, 15));
        assert_eq!(best("A consommer avant fin AOUT 2027"), d(2027, 8, 31));
        assert_eq!(best("DLC 03 FEV 2027"), d(2027, 2, 3));
    }

    #[test]
    fn real_photos() {
        assert_eq!(best("Best Fore\nMinOsenot med\n06/01/27 08:34 14"), d(2027, 1, 6));
        assert_eq!(best("entro il:\n12-12.202T\nAPE4 1 F346 07:26 M3/3"), d(2027, 12, 12));
        assert_eq!(best("12.12.202\nAPE4 L R346 07:26 M3/3"), None);
        assert_eq!(best("28/02/27\n07:43\nRS 6Ss01"), d(2027, 2, 28));
        assert_eq!(best("Mindestens haltbar bis:\n31 12.2026 L245.R."), d(2026, 12, 31));
        assert_eq!(best("25.11.2027\n1 2603121007"), d(2027, 11, 25));
    }

    #[test]
    fn rejects_noise() {
        assert_eq!(best("1234567890123"), None);
        assert_eq!(best("Poids net 500 g"), None);
        assert_eq!(best("31.02.2027"), None);
    }
}
