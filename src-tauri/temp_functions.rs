pub fn cross_validate_amounts(entries: &mut [Itinerary], fallback_texts: &[OcrTextItem]) {
    let all_text: String = fallback_texts
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let ref_amounts = extract_reference_amounts_ordered(&all_text);
    if !ref_amounts.is_empty() && ref_amounts.len() == entries.len() {
        for (i, entry) in entries.iter_mut().enumerate() {
            let ref_amt = ref_amounts[i];
            if ref_amt > 0.0 && (entry.amount - ref_amt).abs() > 0.005 {
                entry.amount = ref_amt;
            }
        }
    }

    let ref_providers = extract_reference_providers_ordered(&all_text);
    if !ref_providers.is_empty() && ref_providers.len() == entries.len() {
        for (i, entry) in entries.iter_mut().enumerate() {
            let ref_pv = &ref_providers[i];
            if !ref_pv.is_empty()
                && (entry.provider.is_empty()
                    || entry.provider.chars().all(|c| c.is_ascii_digit())
                    || entry.provider.chars().count() <= 1
                    || ref_pv.starts_with(&entry.provider.as_str()))
            {
                entry.provider = ref_pv.clone();
            }
        }
    }

    let ref_times = extract_reference_times_ordered(&all_text);
    if !ref_times.is_empty() && ref_times.len() == entries.len() {
        for (i, entry) in entries.iter_mut().enumerate() {
            if is_time_garbled(&entry.date_time) {
                entry.date_time = ref_times[i].clone();
            }
        }
    }
}

fn extract_reference_amounts_ordered(all_text: &str) -> Vec<f64> {
    let mut results = Vec::new();

    let re_didi = Regex::new(
        r"(?m)^\d+\s+\S+\s+\d{2}-\d{2}\s+\d{1,2}[:锛歖.*?([\d.]+)\s*$"
    ).unwrap();
    for cap in re_didi.captures_iter(all_text) {
        if let Ok(amount) = cap[1].parse::<f64>() {
            results.push(amount);
        }
    }

    if !results.is_empty() {
        return results;
    }

    let re_gaode = Regex::new(
        r"(?m)(?:^|\n)\d+\s+\S+.*?([\d.]+)鍏?
    ).unwrap();
    for cap in re_gaode.captures_iter(all_text) {
        if let Ok(amount) = cap[1].parse::<f64>() {
            results.push(amount);
        }
    }

    results
}

fn extract_reference_providers_ordered(all_text: &str) -> Vec<String> {
    let re_didi_main = Regex::new(
        r"(?m)^(\d+)\s+(\S+)\s+\d{2}-\d{2}\s+\d{1,2}[:锛歖"
    ).unwrap();
    let re_didi_cont = Regex::new(
        r"^\s*(杞讳韩|鐗瑰揩|鐢勯€墊蹇溅)\s"
    ).unwrap();

    let main_matches: Vec<(u32, String)> = re_didi_main
        .captures_iter(all_text)
        .filter_map(|cap| {
            let seq: u32 = cap[1].parse().ok()?;
            let pv = cap[2].to_string();
            Some((seq, pv))
        })
        .collect();

    if !main_matches.is_empty() {
        let cont_suffixes: HashMap<u32, String> = {
            let mut map = HashMap::new();
            let lines: Vec<&str> = all_text.lines().collect();
            let mut last_seq: Option<u32> = None;
            for line in &lines {
                if let Some(cap) = re_didi_main.captures(line) {
                    if let Ok(seq) = cap[1].parse::<u32>() {
                        last_seq = Some(seq);
                    }
                    continue;
                }
                if let Some(cap) = re_didi_cont.captures(line) {
                    if let Some(seq) = last_seq {
                        map.insert(seq, cap[1].to_string());
                    }
                }
            }
            map
        };

        let mut results = Vec::new();
        for (seq, main_pv) in &main_matches {
            if let Some(suffix) = cont_suffixes.get(seq) {
                results.push(format!("{}{}", main_pv, suffix));
            } else {
                results.push(main_pv.clone());
            }
        }
        return results;
    }

    let mut results = Vec::new();
    let re_gaode = Regex::new(
        r"(?m)^\d+\s+(\S+)\s+(\S+)\s+\d{4}-\d{2}-\d{2}"
    ).unwrap();
    for cap in re_gaode.captures_iter(all_text) {
        results.push(format!("{}{}", &cap[1], &cap[2]));
    }

    results
}

fn extract_reference_times_ordered(all_text: &str) -> Vec<String> {
    let mut results = Vec::new();

    let re_main = Regex::new(
        r"(?m)^(\d+)\s+\S+\s+(\d{2}-\d{2})\s+(\d{1,2})(:\d{2})?[:锛歖?"
    ).unwrap();
    let re_cont_min = Regex::new(
        // ponytail: 2+ tokens to handle "39 鍛ㄤ簩" continuation lines (was 3+)
        r"^(?:\S+\s+)?(\d{1,2})\s+\S+"
    ).unwrap();

    let lines: Vec<&str> = all_text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if let Some(cap) = re_main.captures(lines[i]) {
            let date = &cap[2];
            let hour = &cap[3];
            let mut minutes = cap.get(4).map(|m| m.as_str().to_string());
            if minutes.is_none() && i + 1 < lines.len() {
                if let Some(cm) = re_cont_min.captures(lines[i + 1]) {
                    let m = &cm[1];
                    if m.len() <= 2 && m.parse::<u32>().map_or(false, |n| n < 60) {
                        minutes = Some(format!(":{}", m));
                    }
                }
            }
            match minutes {
                Some(m) => results.push(format!("{} {}{}", date, hour, m)),
                None => results.push(format!("{} {}:??", date, hour)),
            }
        }
        i += 1;
    }

    if !results.is_empty() {
        return results;
    }

    let re_gaode = Regex::new(
        r"(?m)^\d+\s+\S+\s+\S+\s+(\d{4}-\d{2}-\d{2})\s+(\d{2}:\d{2})"
    ).unwrap();
    for cap in re_gaode.captures_iter(all_text) {
        results.push(format!("{} {}", &cap[1], &cap[2]));
    }

    results
}

fn is_time_garbled(dt: &str) -> bool {
    // 妫€鏌ユ槸鍚︿负鍚堟硶鐨勬棩鏈?鏃堕棿鏍煎紡锛堝厑璁哥煭鏍煎紡 "MM-DD HH:MM" 鍜屽畬鏁存牸寮?"YYYY-MM-DD HH:MM"锛?    // 鍙湁鐪熸涔辩爜鐨勬椂闂达紙OCR 閿欒濡?"鎴愰兘A428"銆?042708"锛夋墠闇€鏇挎崲
    let re_valid = Regex::new(r"\d{1,2}:\d{2}").unwrap();
    let re_short = Regex::new(r"\d{2}-\d{2}\s+\d{1,2}:\d{2}").unwrap();
    let re_full = Regex::new(r"\d{2,4}-\d{2}-\d{2}").unwrap();
    !(re_short.is_match(dt) || (re_full.is_match(dt) && re_valid.is_match(dt)))
}

