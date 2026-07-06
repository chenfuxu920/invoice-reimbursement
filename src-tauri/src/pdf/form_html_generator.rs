use crate::models::reimbursement::ReimbursementForm;
use std::error::Error;

/// 生成完整的报销单 HTML 文件（含 DOCTYPE / html / head / body）
pub fn generate_reimbursement_html(
    form: &ReimbursementForm,
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let table = build_table_html(form);
    let html = format!(
        r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>差旅费报销单</title>
</head>
<body>
{table}
</body>
</html>"##,
        table = table
    );
    std::fs::write(output_path, html)?;
    Ok(())
}

/// 生成仅包含 <style> + <table> 的 HTML 片段（供前端内联预览）
pub fn generate_reimbursement_html_string(form: &ReimbursementForm) -> String {
    build_table_html(form)
}

fn build_table_html(form: &ReimbursementForm) -> String {
    let transport_rows_html = build_transport_rows(form);

    let total_amount_str = format!("{:.2}", form.total_amount);
    let total_chinese = amount_to_chinese(form.total_amount);

    format!(
        r##"<style>
  @page {{ size: A4 landscape; margin: 10mm; }}
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{ font-family: "SimSun", "宋体", serif; font-size: 14px; padding: 20px; }}
  .report {{ width: 100%; border-collapse: collapse; border: 2px solid #000; table-layout: fixed; }}
  .report td {{ border: 1px solid #000; padding: 4px 6px; text-align: center; vertical-align: middle; }}
  .report .title {{ font-size: 18px; font-weight: bold; letter-spacing: 4px; padding: 10px 0; }}
  .lbl {{ text-align: left; font-weight: normal; white-space: nowrap; background: #f5f5f5; }}
  .val {{ text-align: left; font-weight: bold; }}
  .sec {{ writing-mode: vertical-rl; text-orientation: mixed; letter-spacing: 2px; font-weight: bold; }}
  .amt {{ text-align: right; }}
  .hdr td {{ height: 30px; font-weight: bold; }}
  .row td {{ height: 30px; }}
  .sum td {{ height: 36px; font-weight: bold; }}
</style>
<table class="report">
  <tr>
    <td colspan="12" class="title">差 旅 费 报 销 单</td>
  </tr>
  <tr class="row">
    <td class="lbl" style="width:6%">姓名</td>
    <td class="val" style="width:10%">{name}</td>
    <td class="lbl" style="width:6%">部职别</td>
    <td class="val" style="width:16%" colspan="2">{department}</td>
    <td class="lbl" style="width:6%">同行人数</td>
    <td class="val" colspan="6">{companions}</td>
  </tr>
  <tr class="row">
    <td class="lbl">到达地点</td>
    <td class="val" colspan="2">{destination}</td>
    <td class="lbl" style="width:6%">出差日期</td>
    <td class="val" style="width:10%">{travel_start}</td>
    <td style="width:3%">至</td>
    <td class="val" style="width:10%">{travel_end}</td>
    <td class="val" style="width:4%">{travel_days}</td>
    <td style="width:3%">天</td>
    <td style="width:6%" colspan="3"></td>
  </tr>
  <tr class="hdr">
    <td class="lbl" colspan="2">类 别</td>
    <td class="lbl">单据张数</td>
    <td class="lbl">申报金额</td>
    <td class="lbl">核准金额</td>
    <td class="lbl" colspan="2">类 别</td>
    <td class="lbl">人数</td>
    <td class="lbl">天数</td>
    <td class="lbl">标准</td>
    <td class="lbl">申报金额</td>
    <td class="lbl">核准金额</td>
  </tr>
  {transport_rows_html}
  <tr class="row">
    <td class="lbl" colspan="2">行李托运费</td>
    <td></td>
    <td></td>
    <td></td>
    <td class="lbl" colspan="2">凭据报销伙食费</td>
    <td class="amt" colspan="5">¥: {meal_reimbursement:.2}</td>
  </tr>
  <tr class="sum">
    <td class="lbl" colspan="2">申报金额</td>
    <td class="val" colspan="10" style="text-align:center; font-size:15px;">(¥: {total_amount})</td>
  </tr>
  <tr class="sum">
    <td class="lbl" colspan="2">核准金额</td>
    <td class="val" colspan="10" style="text-align:center; font-size:15px;">{total_chinese} (¥: )</td>
  </tr>
</table>"##,
        name = form.name,
        department = form.department,
        companions = if form.companions > 0 { form.companions.to_string() } else { String::new() },
        destination = form.destination,
        travel_start = form.travel_start,
        travel_end = form.travel_end,
        travel_days = form.travel_days,
        transport_rows_html = transport_rows_html,
        meal_reimbursement = form.meal_reimbursement,
        total_amount = total_amount_str,
        total_chinese = total_chinese,
    )
}

/// 构建城市间交通费行 + 住宿费行（左右对齐）
fn build_transport_rows(form: &ReimbursementForm) -> String {
    let transport_labels = ["车、船票", "飞机票", "保险费", "订（退、改签）票"];
    let mut details = [None, None, None, None];
    for d in &form.transport_details {
        for (i, label) in transport_labels.iter().enumerate() {
            if d.label == *label {
                details[i] = Some(d);
            }
        }
    }

    // 城市间交通费 4行 + 小计1行 = 5行
    // 住宿费 4级别 + 小计 = 5行
    // 取较大值作为总行数
    let transport_row_count = 5; // 4 detail + 1 subtotal
    let hotel_levels = ["战区级以上", "军级", "师级", "其他人员"];
    let hotel_row_count = hotel_levels.len() + 1; // 4 levels + 1 subtotal = 5
    // total_rows must be at least transport_row_count+1 (for 市内交通费 overflow) and hotel_row_count
    let total_rows = (transport_row_count + 1).max(hotel_row_count); // max(6, 5) = 6

    let hotel_actual_total: f64 = form.hotel_levels.iter().map(|h| h.actual_amount).sum();

    let mut html = String::new();

    for row_idx in 0..total_rows {
        html.push_str("<tr class=\"row\">");

        // === 左侧 ===
        if row_idx == 0 {
            html.push_str(&format!(
                r##"<td rowspan="{}" class="sec" style="width:4%; font-size:13px;">城市间<br>交通费</td>"##,
                transport_row_count
            ));
        }

        if row_idx < 4 {
            // 交通费明细行
            if let Some(detail) = &details[row_idx] {
                html.push_str(&format!(
                    r##"<td class="lbl">{}</td><td class="amt">{}</td><td class="amt">{:.2}</td><td></td>"##,
                    detail.label, detail.count, detail.amount
                ));
            } else {
                html.push_str(&format!(
                    r##"<td class="lbl">{}</td><td></td><td></td><td></td>"##,
                    transport_labels[row_idx]
                ));
            }
        } else if row_idx == 4 {
            // 交通费小计行（明细行核准金额为空，小计不填）
            html.push_str(&format!(
                r##"<td class="lbl">小 计</td><td></td><td class="amt">{:.2}</td><td></td>"##,
                form.transport_subtotal
            ));
        } else {
            // 市内交通费行（住宿费行末行对齐）
            html.push_str(&format!(
                r##"<td class="lbl" colspan="2">市内交通费</td><td class="amt">{}</td><td class="amt">{:.2}</td><td class="amt">{:.2}</td>"##,
                form.city_transport_count, form.city_transport_actual_amount, form.city_transport_amount
            ));
        }

        // === 右侧：住宿费 ===
        if row_idx == 0 {
            html.push_str(&format!(
                r##"<td rowspan="{}" class="sec" style="width:4%; font-size:13px;">住宿费</td>"##,
                hotel_row_count
            ));
        }

        if row_idx < 4 {
            // 住宿费明细行
            let level_name = hotel_levels[row_idx];
            let detail = form.hotel_levels.iter().find(|h| h.level == level_name);
            if let Some(d) = detail {
                html.push_str(&format!(
                    r##"<td class="lbl">{}</td><td class="amt">{}</td><td class="amt">{}</td><td class="amt">{:.2}</td><td class="amt">{:.2}</td><td class="amt">{:.2}</td>"##,
                    level_name, d.persons, d.days, d.daily_rate, d.actual_amount, d.amount
                ));
            } else {
                html.push_str(&format!(
                    r##"<td class="lbl">{}</td><td></td><td></td><td></td><td></td><td></td>"##,
                    level_name
                ));
            }
        } else if row_idx == 4 {
            // 住宿费小计行
            html.push_str(&format!(
                r##"<td class="lbl">小 计</td><td></td><td></td><td></td><td class="amt">{:.2}</td><td class="amt">{:.2}</td>"##,
                hotel_actual_total, form.hotel_subtotal
            ));
        } else if row_idx == 5 {
            // 计发伙食补助费（上移至 transport section 末行右侧）
            html.push_str(&format!(
                r##"<td class="lbl" colspan="2">计发伙食补助费</td><td class="amt">{}</td><td class="amt">{}</td><td class="amt">{:.2}</td><td class="amt">{:.2}</td><td></td>"##,
                form.meal_subsidy.persons, form.meal_subsidy.days, form.meal_subsidy.daily_rate, form.meal_subsidy.amount
            ));
        } else {
            html.push_str(r##"<td></td><td></td><td></td><td></td><td></td><td></td>"##);
        }

        html.push_str("</tr>\n");
    }

    html
}

/// 金额转中文大写
fn amount_to_chinese(amount: f64) -> String {
    if amount.abs() < 0.01 {
        return "零元整".to_string();
    }
    let digits = ["零", "壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌", "玖"];
    let units = ["", "拾", "佰", "仟"];
    let big_units = ["", "万", "亿"];

    let amount_cents = (amount * 100.0).round() as i64;
    let yuan = amount_cents / 100;
    let jiao = ((amount_cents % 100) / 10) as usize;
    let fen = (amount_cents % 10) as usize;

    let mut result = String::new();

    if yuan == 0 {
        // 金额不足1元
    } else {
        let yuan_str = yuan.to_string();
        let chars: Vec<char> = yuan_str.chars().collect();
        let len = chars.len();
        let mut need_zero = false;

        for (i, &ch) in chars.iter().enumerate() {
            let d = ch.to_digit(10).unwrap() as usize;
            let pos = len - 1 - i;
            let unit_idx = pos % 4;
            let big_unit_idx = pos / 4;

            if d == 0 {
                need_zero = true;
            } else {
                if need_zero {
                    result.push('零');
                    need_zero = false;
                }
                result.push_str(digits[d]);
                result.push_str(units[unit_idx]);
            }

            if unit_idx == 0 && big_unit_idx > 0 {
                result.push_str(big_units[big_unit_idx]);
            }
        }
        result.push('元');
    }

    if jiao == 0 && fen == 0 {
        result.push('整');
    } else {
        if jiao > 0 {
            result.push_str(digits[jiao]);
            result.push('角');
        } else if !result.is_empty() {
            result.push('零');
        }
        if fen > 0 {
            result.push_str(digits[fen]);
            result.push('分');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::reimbursement::*;

    fn sample_form() -> ReimbursementForm {
        ReimbursementForm {
            name: String::new(),
            department: "聘用工程师".to_string(),
            destination: String::new(),
            travel_start: "2025-08-04".to_string(),
            travel_end: "2025-08-15".to_string(),
            travel_days: 12,
            companions: 0,
            transport_details: vec![
                TransportDetail { label: "车、船票".to_string(), count: 1, amount: 553.0 },
                TransportDetail { label: "飞机票".to_string(), count: 1, amount: 1090.0 },
                TransportDetail { label: "订（退、改签）票".to_string(), count: 1, amount: 110.5 },
            ],
            transport_subtotal: 1753.5,
            city_transport_count: 20,
            city_transport_amount: 956.65,
            city_transport_actual_amount: 956.65,
            hotel_levels: vec![
                HotelLevelDetail { level: "其他人员".to_string(), persons: 1, days: 11, daily_rate: 350.0, amount: 3850.0, actual_amount: 4222.63 },
            ],
            hotel_subtotal: 4222.63,
            meal_subsidy: MealSubsidyDetail { persons: 1, days: 12, daily_rate: 100.0, amount: 1200.0 },
            baggage_amount: 0.0,
            meal_reimbursement: 0.0,
            summaries: vec![],
            total_amount: 8132.78,
        }
    }

    #[test]
    fn test_html_generation() {
        let form = sample_form();
        let html = generate_reimbursement_html_string(&form);
        assert!(html.contains("差 旅 费 报 销 单"));
        assert!(html.contains("聘用工程师"));
        assert!(html.contains("553.00"));
        assert!(html.contains("1090.00"));
        assert!(html.contains("8132.78"));
        assert!(html.contains("城市间"));
        assert!(html.contains("住宿费"));
        assert!(html.contains("伙食补助"));
    }

    #[test]
    fn test_amount_to_chinese() {
        assert_eq!(amount_to_chinese(0.0), "零元整");
        assert_eq!(amount_to_chinese(100.0), "壹佰元整");
        assert_eq!(amount_to_chinese(8132.78), "捌仟壹佰叁拾贰元柒角捌分");
    }
}
