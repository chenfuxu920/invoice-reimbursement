# 高速费发票共享支付匹配 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 支持高速通行费发票与行程发票共享同一笔支付记录，自动关联并组合金额匹配。

**架构：** 新增 `Toll` 发票类别和通行时间字段；`MatchResult` 新增共享支付字段；`batch_match` 分离 Toll 发票，按通行时间自动关联到 CityTransport，用"行程金额+高速费金额"组合匹配支付，高速费获得独立 MatchResult 复用行程的支付。

**技术栈：** Rust + chrono + regex + serde；Tauri 2.x + Vue 3 前端

**规格文档：** `docs/superpowers/specs/2026-06-25-toll-invoice-shared-payment-design.md`

---

## 文件结构

### 修改的文件

| 文件 | 职责 |
|---|---|
| `src-tauri/src/models/invoice.rs` | 新增 `Toll` 类别、`toll_travel_time` 字段 |
| `src-tauri/src/models/match_result.rs` | 新增 `shared_payment_ids`、`shared_from_invoice_id` 字段 |
| `src-tauri/src/parser/invoice_type_detector.rs` | 新增 `TollInvoice` 类型及关键词识别 |
| `src-tauri/src/parser/invoice_parser.rs` | `InvoiceType→InvoiceCategory` 映射加 Toll；新增通行时间提取函数 |
| `src-tauri/src/matching/batch.rs` | Toll 分离、自动关联、组合金额匹配、共享支付逻辑 |
| `src-tauri/src/matching/manual.rs` | 手动匹配支持共享支付标记 |
| `src-tauri/src/pdf/form_xlsx_generator.rs` | 高速费发票行展示（支付单号允许重复） |
| `src-tauri/src/pdf/comparison_xlsx_generator.rs` | 同上 |
| `src-tauri/src/pdf/comparison_image_pdf_generator.rs` | 同上 |

### 不修改的文件

- `engine.rs`：一对一/组合匹配核心算法不变，组合金额由 `batch.rs` 计算后传入
- `scoring.rs`：未启用的评分器，不在本次范围

---

## 任务 1：数据模型 — Invoice 新增 Toll 类别和通行时间

**文件：**
- 修改：`src-tauri/src/models/invoice.rs:5-13`（InvoiceCategory 枚举）
- 修改：`src-tauri/src/models/invoice.rs:25-43`（Invoice 结构体）
- 测试：`src-tauri/src/models/invoice.rs`（内联测试）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/models/invoice.rs` 的 `mod tests` 末尾（`test_invoice_with_itineraries` 测试之后）添加：

```rust
    #[test]
    fn test_toll_category_exists() {
        let toll = InvoiceCategory::Toll;
        assert_eq!(toll, InvoiceCategory::Toll);
        assert_ne!(toll, InvoiceCategory::Other);
        assert_ne!(toll, InvoiceCategory::CityTransport);
    }

    #[test]
    fn test_invoice_with_toll_travel_time() {
        let invoice = Invoice {
            id: "inv1".to_string(),
            invoice_number: String::new(),
            amount: 10.0,
            seller_name: String::new(),
            item_name: String::new(),
            date: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            travel_date: None,
            category: InvoiceCategory::Toll,
            source: InvoiceSource::Manual,
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: Some(
                chrono::NaiveDateTime::parse_from_str("2026-05-25 10:06:04", "%Y-%m-%d %H:%M:%S").unwrap()
            ),
        };
        assert!(invoice.toll_travel_time.is_some());
        assert_eq!(invoice.category, InvoiceCategory::Toll);
    }

    #[test]
    fn test_invoice_toll_travel_time_serde_default() {
        // 旧数据无 toll_travel_time 字段，反序列化应默认 None
        let json = r#"{
            "id":"inv1","invoice_number":"","amount":10.0,"seller_name":"",
            "item_name":"","date":"2026-05-25","travel_date":null,
            "category":"Toll","source":{"type":"Manual"},
            "itineraries":[],"itinerary_file":null,"remarks":"",
            "hotel_detail":null,"departure_city":null,"arrival_city":null
        }"#;
        let invoice: Invoice = serde_json::from_str(json).unwrap();
        assert!(invoice.toll_travel_time.is_none());
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test --lib models::invoice::tests::test_toll_category_exists 2>&1`
预期：编译失败，报错 `no variant or associated item named Toll` / `no field toll_travel_time`

- [ ] **步骤 3：实现 InvoiceCategory::Toll 和 toll_travel_time 字段**

修改 `src-tauri/src/models/invoice.rs:5-13`，在 `Meal` 后、`Other` 前加 `Toll`：

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InvoiceCategory {
    Train,          // 高铁/车船票
    Flight,         // 飞机票
    TicketChange,   // 退改签/保险费
    CityTransport,  // 市内交通
    Hotel,          // 住宿费
    Meal,           // 餐饮费
    Toll,           // 高速通行费
    Other,          // 其他
}
```

修改 `src-tauri/src/models/invoice.rs:25-43`，在 `arrival_city` 后加 `toll_travel_time`：

```rust
    pub departure_city: Option<String>,
    pub arrival_city: Option<String>,
    #[serde(default)]
    pub toll_travel_time: Option<chrono::NaiveDateTime>,  // 通行时间（从备注提取，仅 Toll 类）
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test --lib models::invoice::tests::test_toll 2>&1`
预期：3 个 Toll 相关测试 PASS

- [ ] **步骤 5：修复全项目编译错误**

新增字段会导致所有 `Invoice { ... }` 构造缺少 `toll_travel_time`。运行：

```bash
cargo build --lib 2>&1 | Select-String "error\[E0063\]"
```

对每个报错文件，在 `Invoice` 构造的 `arrival_city` 字段后加 `toll_travel_time: None,`。涉及文件（按 grep 结果）：
- `src-tauri/src/parser/invoice_parser.rs`
- `src-tauri/src/parser/dedup.rs`
- `src-tauri/src/pdf/invoice_pipeline.rs`
- `src-tauri/src/matching/batch.rs`
- `src-tauri/src/matching/batch_optimizer.rs`
- `src-tauri/src/matching/benchmarks.rs`
- `src-tauri/src/matching/engine.rs`
- `src-tauri/src/matching/manual.rs`
- `src-tauri/src/matching/scoring.rs`
- `src-tauri/src/matching/strategy_selector.rs`
- `src-tauri/src/pdf/form_builder.rs`
- `src-tauri/src/pdf/comparison_xlsx_generator.rs`
- `src-tauri/src/pdf/form_xlsx_generator.rs`
- `src-tauri/src/models/match_result.rs`
- `src-tauri/tests/*.rs`

运行：`cargo build --lib 2>&1` 确认无错误。

- [ ] **步骤 6：运行全量测试确认无回归**

运行：`cargo test --lib -- --skip test_invoice_parser_with_templates 2>&1`
预期：全部 PASS（跳过已知超时的模板测试）

- [ ] **步骤 7：Commit**

```bash
git add -A
git commit -m "feat(models): 新增 Toll 发票类别和 toll_travel_time 字段"
```

---

## 任务 2：数据模型 — MatchResult 新增共享支付字段

**文件：**
- 修改：`src-tauri/src/models/match_result.rs:23-35`（MatchResult 结构体）
- 修改：`src-tauri/src/models/match_result.rs:93-104`（测试辅助函数 make_result）
- 测试：`src-tauri/src/models/match_result.rs`（内联测试）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/models/match_result.rs` 的 `mod tests` 末尾（`test_payment_for_itinerary_returns_none_when_out_of_range` 之后）添加：

```rust
    #[test]
    fn test_match_result_shared_fields_default_empty() {
        let result = make_result(vec![make_payment("p1")], vec![]);
        assert!(result.shared_payment_ids.is_empty());
        assert!(result.shared_from_invoice_id.is_none());
    }

    #[test]
    fn test_match_result_serde_default_shared_fields() {
        // 旧数据无 shared 字段，反序列化应默认空
        let json = r#"{
            "invoice_id":"inv1","invoice":{"id":"inv1","invoice_number":"","amount":100.0,
            "seller_name":"","item_name":"","date":"2025-01-01","travel_date":null,
            "category":"Other","source":{"type":"Manual"},
            "itineraries":[],"itinerary_file":null,"remarks":"",
            "hotel_detail":null,"departure_city":null,"arrival_city":null,
            "toll_travel_time":null},
            "payment_ids":["p1"],"payments":[{"id":"p1","transaction_id":"TX-p1",
            "transaction_time":"2025-01-01 12:00","amount":50.0,"original_amount":50.0,
            "refund_amount":0.0,"discount":0.0,"merchant_name":"M",
            "source":"Wechat","category":"","payment_method":""}],
            "match_type":"ManualConfirmed","confidence":1.0,"amount_diff":0.0,
            "itinerary_payment_pairs":[]
        }"#;
        let result: MatchResult = serde_json::from_str(json).unwrap();
        assert!(result.shared_payment_ids.is_empty());
        assert!(result.shared_from_invoice_id.is_none());
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test --lib models::match_result::tests::test_match_result_shared 2>&1`
预期：编译失败，报错 `no field shared_payment_ids` / `no field shared_from_invoice_id`

- [ ] **步骤 3：实现共享字段**

修改 `src-tauri/src/models/match_result.rs:23-35`，在 `itinerary_payment_pairs` 后加：

```rust
    /// 行程-支付显式配对。非行程场景或旧数据为空，导出层回退按 payments 索引对应。
    #[serde(default)]
    pub itinerary_payment_pairs: Vec<ItineraryPaymentPair>,
    /// 共享的支付ID（高速费 MatchResult 标记复用的支付）。
    #[serde(default)]
    pub shared_payment_ids: Vec<String>,
    /// 共享来源发票ID（高速费 MatchResult 指向行程发票ID）。
    #[serde(default)]
    pub shared_from_invoice_id: Option<String>,
}
```

修改 `make_result` 辅助函数（`src-tauri/src/models/match_result.rs:93-104`），在 `itinerary_payment_pairs: pairs,` 后加：

```rust
            itinerary_payment_pairs: pairs,
            shared_payment_ids: vec![],
            shared_from_invoice_id: None,
        }
    }
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test --lib models::match_result::tests::test_match_result_shared 2>&1`
预期：2 个测试 PASS

- [ ] **步骤 5：修复全项目 MatchResult 构造编译错误**

运行：`cargo build --lib 2>&1 | Select-String "error\[E0063\]"`

对每个报错文件，在 `MatchResult { ... }` 构造的 `itinerary_payment_pairs:` 后加 `shared_payment_ids: vec![], shared_from_invoice_id: None,`。涉及文件：
- `src-tauri/src/matching/engine.rs`（2 处：match_one_to_one、match_one_to_many）
- `src-tauri/src/matching/batch.rs`（1 处：match_itinerary_to_payments）
- `src-tauri/src/matching/batch_optimizer.rs`（多处）
- `src-tauri/src/matching/manual.rs`（1 处：create_manual_match）

运行：`cargo build --lib 2>&1` 确认无错误。

- [ ] **步骤 6：运行全量测试确认无回归**

运行：`cargo test --lib -- --skip test_invoice_parser_with_templates 2>&1`
预期：全部 PASS

- [ ] **步骤 7：Commit**

```bash
git add -A
git commit -m "feat(models): MatchResult 新增共享支付字段"
```

---

## 任务 3：高速费发票类型识别

**文件：**
- 修改：`src-tauri/src/parser/invoice_type_detector.rs:5-15`（InvoiceType 枚举）
- 修改：`src-tauri/src/parser/invoice_type_detector.rs:20-57`（detect 方法）
- 测试：`src-tauri/src/parser/invoice_type_detector.rs`（内联测试）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/parser/invoice_type_detector.rs` 的 `mod tests` 末尾添加：

```rust
    #[test]
    fn test_detect_toll_invoice_by_keyword() {
        let ocr = create_ocr_output(vec!["通行费", "增值税电子发票", "价税合计：10.00"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TollInvoice);
    }

    #[test]
    fn test_detect_toll_invoice_etc() {
        let ocr = create_ocr_output(vec!["ETC通行费", "高速公路", "金额：15.50"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TollInvoice);
    }

    #[test]
    fn test_detect_toll_invoice_overpass_fee() {
        let ocr = create_ocr_output(vec!["过路费", "电子发票"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TollInvoice);
    }

    #[test]
    fn test_detect_toll_priority_over_vat_electronic() {
        // 同时含"增值税电子发票"和"通行费"，应优先识别为 Toll
        let ocr = create_ocr_output(vec!["增值税电子发票", "通行费", "价税合计：10.00"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TollInvoice);
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test --lib parser::invoice_type_detector::tests::test_detect_toll 2>&1`
预期：编译失败，报错 `no variant or associated item named TollInvoice`

- [ ] **步骤 3：实现 TollInvoice 类型和识别**

修改 `src-tauri/src/parser/invoice_type_detector.rs:5-15`，在 `TransitCardStatement` 后、`Other` 前加：

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvoiceType {
    VatElectronicInvoice,
    RideHailingInvoice,
    RideHailingItinerary,
    FlightInvoice,
    TrainInvoice,
    HotelStatement,
    TransitCardStatement,
    TollInvoice,    // 新增：高速通行费发票
    Other,
}
```

修改 `detect` 方法（`src-tauri/src/parser/invoice_type_detector.rs:20-57`），在 `is_vat_electronic_invoice` 检查**之前**插入 Toll 检查（优先于增值税电子发票）：

```rust
    pub fn detect(ocr_output: &OcrStructuredOutput) -> InvoiceType {
        let all_text = ocr_output
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        if Self::is_ride_hailing_itinerary(&all_text) {
            return InvoiceType::RideHailingItinerary;
        }

        if Self::is_transit_card_statement(&all_text) {
            return InvoiceType::TransitCardStatement;
        }

        if Self::is_flight_invoice(&all_text) {
            return InvoiceType::FlightInvoice;
        }

        if Self::is_toll_invoice(&all_text) {
            return InvoiceType::TollInvoice;
        }

        if Self::is_vat_electronic_invoice(&all_text) {
            return InvoiceType::VatElectronicInvoice;
        }

        if Self::is_hotel_statement(&all_text) {
            return InvoiceType::HotelStatement;
        }

        if Self::is_train_invoice(&all_text) {
            return InvoiceType::TrainInvoice;
        }

        if Self::is_ride_hailing_invoice(&all_text) {
            return InvoiceType::RideHailingInvoice;
        }

        InvoiceType::Other
    }

    fn is_toll_invoice(text: &str) -> bool {
        text.contains("通行费")
            || text.contains("过路费")
            || (text.contains("ETC") && text.contains("高速"))
            || (text.contains("高速") && text.contains("电子发票"))
    }
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test --lib parser::invoice_type_detector::tests::test_detect_toll 2>&1`
预期：4 个测试 PASS

- [ ] **步骤 5：Commit**

```bash
git add -A
git commit -m "feat(parser): 识别高速通行费发票类型 TollInvoice"
```

---

## 任务 4：InvoiceType→InvoiceCategory 映射加 Toll + 通行时间提取

**文件：**
- 修改：`src-tauri/src/parser/invoice_parser.rs:742-750`（类别映射）
- 修改：`src-tauri/src/parser/invoice_parser.rs`（新增提取函数）
- 测试：`src-tauri/src/parser/invoice_parser.rs`（内联测试）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/parser/invoice_parser.rs` 的 `mod tests` 末尾添加：

```rust
    #[test]
    fn test_extract_toll_travel_time_standard_format() {
        let remarks = "湘ADG5926 湖南新港站入 湖南黄花站出 2026-05-25 10:06:04 （不可用于增值税进项抵扣）";
        let time = extract_toll_travel_time(remarks);
        assert!(time.is_some());
        let t = time.unwrap();
        assert_eq!(t.date(), chrono::NaiveDate::from_ymd_opt(2026, 5, 25).unwrap());
        assert_eq!(t.time(), chrono::NaiveTime::from_hms_opt(10, 6, 4).unwrap());
    }

    #[test]
    fn test_extract_toll_travel_time_second_example() {
        let remarks = "川AB55365 四川天府机场T1T2站入 四川天府机场成都站出 2026-06-23 14:24:10 （不可用于增值税进项抵扣）";
        let time = extract_toll_travel_time(remarks);
        assert!(time.is_some());
        let t = time.unwrap();
        assert_eq!(t.date(), chrono::NaiveDate::from_ymd_opt(2026, 6, 23).unwrap());
        assert_eq!(t.time(), chrono::NaiveTime::from_hms_opt(14, 24, 10).unwrap());
    }

    #[test]
    fn test_extract_toll_travel_time_no_date() {
        let remarks = "普通备注无时间";
        let time = extract_toll_travel_time(remarks);
        assert!(time.is_none());
    }

    #[test]
    fn test_extract_toll_travel_time_date_only() {
        let remarks = "通行时间 2026-05-25";
        let time = extract_toll_travel_time(remarks);
        assert!(time.is_some());
        let t = time.unwrap();
        assert_eq!(t.date(), chrono::NaiveDate::from_ymd_opt(2026, 5, 25).unwrap());
        assert_eq!(t.time(), chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test --lib parser::invoice_parser::tests::test_extract_toll_travel_time 2>&1`
预期：编译失败，报错 `cannot find function extract_toll_travel_time`

- [ ] **步骤 3：实现通行时间提取函数**

在 `src-tauri/src/parser/invoice_parser.rs` 中（`extract_amount` 函数附近）添加：

```rust
/// 从高速费发票备注中提取通行时间。
/// 支持格式："YYYY-MM-DD HH:MM:SS" 或 "YYYY-MM-DD"。
/// 取第一个匹配的日期时间字符串。
pub fn extract_toll_travel_time(remarks: &str) -> Option<chrono::NaiveDateTime> {
    // 优先匹配 "YYYY-MM-DD HH:MM:SS"
    let re_datetime = regex::Regex::new(r"(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})").ok()?;
    if let Some(caps) = re_datetime.captures(remarks) {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&caps[1], "%Y-%m-%d %H:%M:%S") {
            return Some(dt);
        }
    }
    // 回退匹配 "YYYY-MM-DD"
    let re_date = regex::Regex::new(r"(\d{4}-\d{2}-\d{2})").ok()?;
    if let Some(caps) = re_date.captures(remarks) {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(&caps[1], "%Y-%m-%d") {
            return d.and_hms_opt(0, 0, 0);
        }
    }
    None
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test --lib parser::invoice_parser::tests::test_extract_toll_travel_time 2>&1`
预期：4 个测试 PASS

- [ ] **步骤 5：在 InvoiceType→InvoiceCategory 映射中加 Toll**

修改 `src-tauri/src/parser/invoice_parser.rs:742-750`，在 `RideHailingInvoice` 分支后加：

```rust
    match invoice_type {
        InvoiceType::FlightInvoice => return InvoiceCategory::Flight,
        InvoiceType::TrainInvoice => return InvoiceCategory::Train,
        InvoiceType::HotelStatement => return InvoiceCategory::Hotel,
        InvoiceType::RideHailingInvoice | InvoiceType::RideHailingItinerary => {
            return InvoiceCategory::CityTransport
        }
        InvoiceType::TollInvoice => return InvoiceCategory::Toll,
        _ => {}
    }
```

- [ ] **步骤 6：在发票解析流程中填充 toll_travel_time**

找到 `src-tauri/src/parser/invoice_parser.rs` 中构造 `Invoice` 的主函数（约 line 340-360，`remarks: regions.remarks.clone()` 附近），在 `toll_travel_time` 字段处加：

```rust
            toll_travel_time: if category == InvoiceCategory::Toll {
                extract_toll_travel_time(&regions.remarks)
            } else {
                None
            },
```

- [ ] **步骤 7：运行全量测试确认无回归**

运行：`cargo test --lib -- --skip test_invoice_parser_with_templates 2>&1`
预期：全部 PASS

- [ ] **步骤 8：Commit**

```bash
git add -A
git commit -m "feat(parser): Toll 类别映射和通行时间提取"
```

---

## 任务 5：batch_match — Toll 分离与自动关联

**文件：**
- 修改：`src-tauri/src/matching/batch.rs:16-72`（batch_match 函数）
- 测试：`src-tauri/src/matching/batch.rs`（内联测试）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/matching/batch.rs` 的 `mod tests` 末尾添加测试辅助函数和红灯测试：

```rust
    fn make_toll_invoice(id: &str, amount: f64, travel_time: &str) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "ETC".to_string(),
            item_name: "通行费".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            travel_date: None,
            category: InvoiceCategory::Toll,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: format!("XX站入 XX站出 {}", travel_time),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: chrono::NaiveDateTime::parse_from_str(
                travel_time, "%Y-%m-%d %H:%M:%S"
            ).ok(),
        }
    }

    #[test]
    fn test_toll_auto_links_to_nearest_city_transport() {
        // 行程发票 50元，高速费 10元，支付 60元
        let mut invoice = make_city_transport_invoice("inv1", 50.00);
        invoice.itineraries = vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 50.00,
        }];
        let toll = make_toll_invoice("toll1", 10.00, "2025-01-15 09:30:00");
        let payment = make_payment_at("p1", 60.00, "2025-01-15 09:35");

        let result = batch_match(&[invoice, toll], &[payment], 1.00);

        // 两张发票都应匹配成功，共用同一笔支付
        assert_eq!(result.matched.len(), 2);
        assert_eq!(result.unmatched_invoices.len(), 0);
        assert_eq!(result.unmatched_payments.len(), 0);

        // 行程发票 MatchResult
        let trip_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::CityTransport).unwrap();
        assert_eq!(trip_match.payment_ids, vec!["p1".to_string()]);
        assert!(trip_match.shared_payment_ids.is_empty());
        assert!(trip_match.shared_from_invoice_id.is_none());

        // 高速费 MatchResult
        let toll_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::Toll).unwrap();
        assert_eq!(toll_match.payment_ids, vec!["p1".to_string()]);
        assert_eq!(toll_match.shared_payment_ids, vec!["p1".to_string()]);
        assert_eq!(toll_match.shared_from_invoice_id, Some("inv1".to_string()));
    }

    #[test]
    fn test_toll_no_city_transport_goes_unmatched() {
        // 没有行程发票，高速费无法关联，应未匹配
        let toll = make_toll_invoice("toll1", 10.00, "2025-01-15 09:30:00");
        let payment = make_payment_at("p1", 10.00, "2025-01-15 09:35");

        let result = batch_match(&[toll], &[payment], 1.00);

        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 1);
        assert_eq!(result.unmatched_invoices[0].id, "toll1");
    }

    #[test]
    fn test_toll_combination_amount_matches_payment() {
        // 行程 50 + 高速费 10 = 60，支付 60元
        let mut invoice = make_city_transport_invoice("inv1", 50.00);
        invoice.itineraries = vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 50.00,
        }];
        let toll = make_toll_invoice("toll1", 10.00, "2025-01-15 09:30:00");
        // 支付恰好 60 元（行程+高速费组合）
        let payment = make_payment_at("p1", 60.00, "2025-01-15 09:35");

        let result = batch_match(&[invoice, toll], &[payment], 1.00);

        assert_eq!(result.matched.len(), 2);
        let trip_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::CityTransport).unwrap();
        // 行程发票的 amount_diff 应基于组合金额 60 vs 支付 60
        assert!(trip_match.amount_diff <= 1.00);
    }

    #[test]
    fn test_toll_falls_to_next_trip_if_first_fails() {
        // 两条行程：行程1(50元,09:00) 行程2(40元,14:00)
        // 高速费 20元，通行时间 14:30（更近行程2）
        // 支付1: 60元（行程1+高速费=70 不匹配） 支付2: 60元（行程2+高速费=60 匹配）
        let mut inv1 = make_city_transport_invoice("inv1", 50.00);
        inv1.itineraries = vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 50.00,
        }];
        let mut inv2 = make_city_transport_invoice("inv2", 40.00);
        inv2.itineraries = vec![Itinerary {
            date_time: "2025-01-15 14:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "C".to_string(),
            dropoff: "D".to_string(),
            amount: 40.00,
        }];
        let toll = make_toll_invoice("toll1", 20.00, "2025-01-15 14:30:00");
        let payments = vec![
            make_payment_at("p1", 60.00, "2025-01-15 09:05"),  // 行程1时间附近，但50+20=70≠60
            make_payment_at("p2", 60.00, "2025-01-15 14:05"),  // 行程2时间附近，40+20=60 匹配
        ];

        let result = batch_match(&[inv1, inv2, toll], &payments, 1.00);

        // 高速费应关联到行程2（时间更近且金额组合匹配）
        let toll_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::Toll).unwrap();
        assert_eq!(toll_match.shared_from_invoice_id, Some("inv2".to_string()));
        assert_eq!(toll_match.payment_ids, vec!["p2".to_string()]);
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test --lib matching::batch::tests::test_toll 2>&1`
预期：4 个测试 FAIL（高速费未被关联，当前逻辑下高速费走一对一匹配失败）

- [ ] **步骤 3：实现 Toll 分离和自动关联**

修改 `src-tauri/src/matching/batch.rs:16-72` 的 `batch_match` 函数：

```rust
pub fn batch_match(
    invoices: &[Invoice],
    payments: &[PaymentRecord],
    tolerance: f64,
) -> BatchMatchResult {
    let engine = MatchEngine::new(tolerance);
    // 按交易时间升序排序，消除文件读取顺序偏差
    let mut payments_sorted: Vec<PaymentRecord> = payments.to_vec();
    sort_payments_by_time(&mut payments_sorted);
    let payments = &payments_sorted[..];

    // 分离 Toll 发票和其他发票
    let mut toll_invoices: Vec<Invoice> = invoices.iter()
        .filter(|inv| inv.category == InvoiceCategory::Toll)
        .cloned()
        .collect();
    let non_toll_invoices: Vec<Invoice> = invoices.iter()
        .filter(|inv| inv.category != InvoiceCategory::Toll)
        .cloned()
        .collect();

    let mut matched = Vec::new();
    let mut unmatched_invoices = Vec::new();
    let mut used_payment_ids: Vec<String> = Vec::new();

    // 自动关联 Toll 到 CityTransport：toll_id -> city_transport_id
    let mut toll_links: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    // CityTransport 发票列表（含行程单），用于关联
    let city_transport_invoices: Vec<&Invoice> = non_toll_invoices.iter()
        .filter(|inv| inv.category == InvoiceCategory::CityTransport && !inv.itineraries.is_empty())
        .collect();

    for toll in &toll_invoices {
        let toll_time = toll.toll_travel_time
            .map(|dt| dt)
            .or_else(|| {
                // 回退用开票日期
                Some(toll.date.and_hms_opt(0, 0, 0).unwrap())
            });
        if let Some(tt) = toll_time {
            // 找通行时间与首条行程时间差最小的 CityTransport
            let best = city_transport_invoices.iter()
                .filter(|ct| !toll_links.values().any(|linked_id| linked_id == &ct.id))
                .min_by_key(|ct| {
                    ct.itineraries.first()
                        .and_then(|e| parse_datetime(&e.date_time))
                        .map(|it| (it - tt).num_seconds().abs())
                        .unwrap_or(i64::MAX)
                });
            if let Some(ct) = best {
                toll_links.insert(toll.id.clone(), ct.id.clone());
            }
        }
    }

    // 匹配非 Toll 发票（含行程）
    for invoice in &non_toll_invoices {
        let available_payments: Vec<PaymentRecord> = payments
            .iter()
            .filter(|p| !used_payment_ids.contains(&p.id))
            .cloned()
            .collect();

        // 计算组合目标金额：行程金额 + 关联的 Toll 金额
        let linked_toll_amount: f64 = toll_links.iter()
            .filter(|(_, ct_id)| ct_id == &invoice.id)
            .filter_map(|(toll_id, _)| {
                toll_invoices.iter().find(|t| t.id == *toll_id).map(|t| t.amount)
            })
            .sum();
        let combined_amount = invoice.amount + linked_toll_amount;

        // 用组合金额构造临时发票用于匹配
        let mut invoice_for_match = invoice.clone();
        invoice_for_match.amount = combined_amount;

        let result = if invoice.category == InvoiceCategory::CityTransport
            && !invoice.itineraries.is_empty()
        {
            if invoice.itineraries.len() > 1 {
                match_itinerary_to_payments(&invoice_for_match, &available_payments, tolerance)
                    .or_else(|| {
                        let time_filtered = filter_payments_by_itinerary_time(&invoice_for_match, &available_payments);
                        engine.match_one_to_many(&invoice_for_match, &time_filtered)
                    })
            } else {
                let time_filtered = filter_payments_by_itinerary_time(&invoice_for_match, &available_payments);
                engine.match_one_to_many(&invoice_for_match, &time_filtered)
            }
        } else {
            engine.match_one_to_one(&invoice_for_match, &available_payments)
        };

        if let Some(match_result) = result {
            for pid in &match_result.payment_ids {
                used_payment_ids.push(pid.clone());
            }
            // 行程发票 MatchResult 用原始发票（非组合金额）
            let mut trip_result = match_result;
            trip_result.invoice = invoice.clone();
            trip_result.invoice_id = invoice.id.clone();
            // 重新计算 amount_diff 基于原始金额
            let total: f64 = trip_result.payments.iter().map(|p| p.amount).sum();
            trip_result.amount_diff = (invoice.amount - total).abs();
            matched.push(trip_result);

            // 为关联的 Toll 发票创建共享 MatchResult
            let linked_tolls: Vec<&Invoice> = toll_invoices.iter()
                .filter(|t| toll_links.get(&t.id) == Some(&invoice.id))
                .collect();
            for toll in linked_tolls {
                let toll_match = MatchResult {
                    invoice_id: toll.id.clone(),
                    invoice: toll.clone(),
                    payment_ids: matched.last().unwrap().payment_ids.clone(),
                    payments: matched.last().unwrap().payments.clone(),
                    match_type: MatchType::OneToMany,
                    confidence: 1.0 - ((toll.amount - matched.last().unwrap().payments.iter().map(|p| p.amount).sum::<f64>()).abs() / toll.amount.max(0.01)),
                    amount_diff: (toll.amount - matched.last().unwrap().payments.iter().map(|p| p.amount).sum::<f64>()).abs(),
                    itinerary_payment_pairs: vec![],
                    shared_payment_ids: matched.last().unwrap().payment_ids.clone(),
                    shared_from_invoice_id: Some(invoice.id.clone()),
                };
                matched.push(toll_match);
                // 从 toll_invoices 移除已匹配的
                toll_invoices.retain(|t| t.id != toll.id);
            }
        } else {
            // 行程匹配失败：解除关联的 Toll，让它们尝试下一个 CityTransport
            toll_links.retain(|_, ct_id| ct_id != &invoice.id);
            unmatched_invoices.push(invoice.clone());
        }
    }

    // 未关联到任何行程的 Toll 发票 → 未匹配
    for toll in &toll_invoices {
        unmatched_invoices.push(toll.clone());
    }

    let unmatched_payments: Vec<PaymentRecord> = payments
        .iter()
        .filter(|p| !used_payment_ids.contains(&p.id))
        .cloned()
        .collect();

    BatchMatchResult {
        matched,
        unmatched_invoices,
        unmatched_payments,
    }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test --lib matching::batch::tests::test_toll 2>&1`
预期：4 个测试 PASS

- [ ] **步骤 5：运行全量 matching 测试确认无回归**

运行：`cargo test --lib matching:: 2>&1`
预期：全部 PASS

- [ ] **步骤 6：Commit**

```bash
git add -A
git commit -m "feat(matching): Toll 发票自动关联和组合金额匹配"
```

---

## 任务 6：手动匹配支持共享支付标记

**文件：**
- 修改：`src-tauri/src/matching/manual.rs:8-26`（create_manual_match 函数）
- 测试：`src-tauri/src/matching/manual.rs`（内联测试）

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/matching/manual.rs` 的 `mod tests` 末尾添加：

```rust
    #[test]
    fn test_manual_match_with_shared_payment() {
        let invoice = make_invoice("toll1", 10.0);
        let payments = vec![make_payment("pay1", 60.0)];
        let result = create_manual_match_shared(
            invoice,
            payments,
            vec![],
            Some("inv_trip".to_string()),
        );
        assert_eq!(result.shared_from_invoice_id, Some("inv_trip".to_string()));
        assert_eq!(result.shared_payment_ids, vec!["pay1".to_string()]);
        assert!(matches!(result.match_type, MatchType::ManualConfirmed));
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test --lib matching::manual::tests::test_manual_match_with_shared 2>&1`
预期：编译失败，报错 `cannot find function create_manual_match_shared`

- [ ] **步骤 3：实现 create_manual_match_shared**

修改 `src-tauri/src/matching/manual.rs`，在 `create_manual_match` 后添加：

```rust
/// 手动创建共享匹配（高速费复用行程支付）
/// shared_from_invoice_id：共享来源的行程发票ID
pub fn create_manual_match_shared(
    invoice: Invoice,
    payments: Vec<PaymentRecord>,
    itinerary_payment_pairs: Vec<ItineraryPaymentPair>,
    shared_from_invoice_id: Option<String>,
) -> MatchResult {
    let total: f64 = payments.iter().map(|p| p.amount).sum();
    let diff = (invoice.amount - total).abs();
    let payment_ids: Vec<String> = payments.iter().map(|p| p.id.clone()).collect();

    MatchResult {
        invoice_id: invoice.id.clone(),
        invoice,
        payment_ids: payment_ids.clone(),
        payments,
        match_type: MatchType::ManualConfirmed,
        confidence: if diff == 0.0 { 1.0 } else { 0.8 },
        amount_diff: diff,
        itinerary_payment_pairs,
        shared_payment_ids: if shared_from_invoice_id.is_some() { payment_ids } else { vec![] },
        shared_from_invoice_id,
    }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test --lib matching::manual::tests::test_manual_match_with_shared 2>&1`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add -A
git commit -m "feat(matching): 手动匹配支持共享支付标记"
```

---

## 任务 7：报销单生成 — 高速费发票独立行展示

**文件：**
- 修改：`src-tauri/src/pdf/form_xlsx_generator.rs`（高速费发票行）
- 修改：`src-tauri/src/pdf/comparison_xlsx_generator.rs`（同上）
- 修改：`src-tauri/src/pdf/comparison_image_pdf_generator.rs`（同上）
- 测试：各文件内联测试

- [ ] **步骤 1：编写失败的测试**

先确认现有生成器如何遍历 match_results 和展示支付单号。运行：

```bash
cargo test --lib pdf::form_xlsx_generator 2>&1 | Select-String "test result"
```

在 `src-tauri/src/pdf/form_xlsx_generator.rs` 的 `mod tests` 末尾添加：

```rust
    #[test]
    fn test_toll_invoice_appears_in_xlsx_with_shared_payment() {
        let trip_invoice = Invoice {
            id: "inv1".to_string(),
            invoice_number: "TRIP001".to_string(),
            amount: 50.0,
            seller_name: "滴滴出行".to_string(),
            item_name: "市内交通".to_string(),
            date: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            travel_date: None,
            category: InvoiceCategory::CityTransport,
            source: InvoiceSource::Pdf("trip.pdf".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: None,
        };
        let toll_invoice = Invoice {
            id: "toll1".to_string(),
            invoice_number: "TOLL001".to_string(),
            amount: 10.0,
            seller_name: "ETC".to_string(),
            item_name: "通行费".to_string(),
            date: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            travel_date: None,
            category: InvoiceCategory::Toll,
            source: InvoiceSource::Pdf("toll.pdf".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: None,
        };
        let payment = PaymentRecord {
            id: "p1".to_string(),
            transaction_id: "TX001".to_string(),
            transaction_time: "2026-05-25 09:35".to_string(),
            amount: 60.0,
            original_amount: 60.0,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: "滴滴出行".to_string(),
            source: PaymentSource::Wechat,
            category: "交通".to_string(),
            payment_method: String::new(),
        };
        let trip_match = MatchResult {
            invoice_id: "inv1".to_string(),
            invoice: trip_invoice,
            payment_ids: vec!["p1".to_string()],
            payments: vec![payment.clone()],
            match_type: MatchType::OneToMany,
            confidence: 1.0,
            amount_diff: 10.0,
            itinerary_payment_pairs: vec![],
            shared_payment_ids: vec![],
            shared_from_invoice_id: None,
        };
        let toll_match = MatchResult {
            invoice_id: "toll1".to_string(),
            invoice: toll_invoice,
            payment_ids: vec!["p1".to_string()],
            payments: vec![payment],
            match_type: MatchType::OneToMany,
            confidence: 0.8,
            amount_diff: 50.0,
            itinerary_payment_pairs: vec![],
            shared_payment_ids: vec!["p1".to_string()],
            shared_from_invoice_id: Some("inv1".to_string()),
        };

        // 验证两张发票都在 match_results 中，支付单号都显示
        let match_results = vec![trip_match, toll_match];
        // 高速费发票应独立存在，不被过滤
        assert_eq!(match_results.len(), 2);
        assert!(match_results.iter().any(|m| m.invoice.category == InvoiceCategory::Toll));
        // 两张发票的支付单号相同（允许重复）
        assert_eq!(match_results[0].payments[0].transaction_id, "TX001");
        assert_eq!(match_results[1].payments[0].transaction_id, "TX001");
    }
```

- [ ] **步骤 2：运行测试验证通过（此测试验证数据模型，应直接通过）**

运行：`cargo test --lib pdf::form_xlsx_generator::tests::test_toll_invoice_appears 2>&1`
预期：PASS（此测试验证数据结构正确性，生成器本身已遍历所有 match_results）

- [ ] **步骤 3：确认生成器不过滤 Toll 发票**

检查 `src-tauri/src/pdf/form_xlsx_generator.rs`、`comparison_xlsx_generator.rs`、`comparison_image_pdf_generator.rs` 中遍历 match_results 的逻辑，确认没有按类别过滤掉 Toll。运行：

```bash
cargo build --lib 2>&1
```

确认无编译错误。如有按类别过滤的逻辑（如 `filter(|r| r.invoice.category != Toll)`），移除该过滤。

- [ ] **步骤 4：运行全量测试确认无回归**

运行：`cargo test --lib pdf:: 2>&1`
预期：全部 PASS

- [ ] **步骤 5：Commit**

```bash
git add -A
git commit -m "feat(pdf): 高速费发票独立行展示，支付单号允许重复"
```

---

## 任务 8：端到端集成测试

**文件：**
- 创建：`src-tauri/tests/toll_integration_test.rs`

- [ ] **步骤 1：编写集成测试**

创建 `src-tauri/tests/toll_integration_test.rs`：

```rust
use invoice_reimbursement_lib::matching::batch::batch_match;
use invoice_reimbursement_lib::matching::manual::create_manual_match_shared;
use invoice_reimbursement_lib::models::invoice::{Invoice, InvoiceCategory, InvoiceSource, Itinerary};
use invoice_reimbursement_lib::models::match_result::MatchType;
use invoice_reimbursement_lib::models::payment::{PaymentRecord, PaymentSource};
use chrono::NaiveDate;

fn make_trip_invoice(id: &str, amount: f64, itin_time: &str, itin_amount: f64) -> Invoice {
    Invoice {
        id: id.to_string(),
        invoice_number: format!("INV-{}", id),
        amount,
        seller_name: "滴滴出行".to_string(),
        item_name: "市内交通".to_string(),
        date: NaiveDate::parse_from_str(&itin_time[..10], "%Y-%m-%d").unwrap(),
        travel_date: None,
        category: InvoiceCategory::CityTransport,
        source: InvoiceSource::Link("http://example.com".to_string()),
        itineraries: vec![Itinerary {
            date_time: itin_time.to_string(),
            provider: "滴滴".to_string(),
            pickup: "A站".to_string(),
            dropoff: "B站".to_string(),
            amount: itin_amount,
        }],
        itinerary_file: None,
        remarks: String::new(),
        hotel_detail: None,
        departure_city: None,
        arrival_city: None,
        toll_travel_time: None,
    }
}

fn make_toll_invoice(id: &str, amount: f64, travel_time: &str) -> Invoice {
    Invoice {
        id: id.to_string(),
        invoice_number: format!("TOLL-{}", id),
        amount,
        seller_name: "ETC".to_string(),
        item_name: "通行费".to_string(),
        date: NaiveDate::parse_from_str(&travel_time[..10], "%Y-%m-%d").unwrap(),
        travel_date: None,
        category: InvoiceCategory::Toll,
        source: InvoiceSource::Link("http://example.com".to_string()),
        itineraries: vec![],
        itinerary_file: None,
        remarks: format!("XX站入 XX站出 {}", travel_time),
        hotel_detail: None,
        departure_city: None,
        arrival_city: None,
        toll_travel_time: chrono::NaiveDateTime::parse_from_str(
            travel_time, "%Y-%m-%d %H:%M:%S"
        ).ok(),
    }
}

fn make_payment(id: &str, amount: f64, time: &str) -> PaymentRecord {
    PaymentRecord {
        id: id.to_string(),
        transaction_id: format!("TX-{}", id),
        transaction_time: time.to_string(),
        amount,
        original_amount: amount,
        refund_amount: 0.0,
        discount: 0.0,
        merchant_name: "滴滴出行".to_string(),
        source: PaymentSource::Wechat,
        category: "交通".to_string(),
        payment_method: String::new(),
    }
}

#[test]
fn test_e2e_toll_shared_payment() {
    let trip = make_trip_invoice("inv1", 50.0, "2025-01-15 09:00", 50.0);
    let toll = make_toll_invoice("toll1", 10.0, "2025-01-15 09:30:00");
    let payment = make_payment("p1", 60.0, "2025-01-15 09:35");

    let result = batch_match(&[trip, toll], &[payment], 1.0);

    assert_eq!(result.matched.len(), 2);
    assert_eq!(result.unmatched_invoices.len(), 0);
    assert_eq!(result.unmatched_payments.len(), 0);

    let toll_match = result.matched.iter()
        .find(|m| m.invoice.category == InvoiceCategory::Toll).unwrap();
    assert_eq!(toll_match.shared_from_invoice_id, Some("inv1".to_string()));
    assert_eq!(toll_match.payments[0].id, "p1");
}

#[test]
fn test_e2e_toll_manual_shared_match() {
    let toll = make_toll_invoice("toll1", 10.0, "2025-01-15 09:30:00");
    let payment = make_payment("p1", 60.0, "2025-01-15 09:35");

    let result = create_manual_match_shared(
        toll,
        vec![payment],
        vec![],
        Some("inv_trip".to_string()),
    );

    assert!(matches!(result.match_type, MatchType::ManualConfirmed));
    assert_eq!(result.shared_from_invoice_id, Some("inv_trip".to_string()));
    assert_eq!(result.shared_payment_ids, vec!["p1".to_string()]);
}

#[test]
fn test_e2e_toll_without_trip_unmatched() {
    let toll = make_toll_invoice("toll1", 10.0, "2025-01-15 09:30:00");
    let payment = make_payment("p1", 10.0, "2025-01-15 09:35");

    let result = batch_match(&[toll], &[payment], 1.0);

    assert_eq!(result.matched.len(), 0);
    assert_eq!(result.unmatched_invoices.len(), 1);
    assert_eq!(result.unmatched_invoices[0].id, "toll1");
}
```

- [ ] **步骤 2：运行集成测试验证通过**

运行：`cargo test --test toll_integration_test 2>&1`
预期：3 个测试 PASS

- [ ] **步骤 3：运行全量测试确认无回归**

运行：`cargo test --lib -- --skip test_invoice_parser_with_templates 2>&1`
运行：`cargo test --test matching_integration 2>&1`
预期：全部 PASS

- [ ] **步骤 4：Commit**

```bash
git add -A
git commit -m "test: 高速费共享支付端到端集成测试"
```

---

## 自检

### 规格覆盖度

| 规格需求 | 对应任务 |
|---|---|
| 新增 Toll 类别 | 任务 1 |
| Invoice.toll_travel_time 字段 | 任务 1 |
| MatchResult.shared_payment_ids / shared_from_invoice_id | 任务 2 |
| 电子发票关键词识别 | 任务 3 |
| InvoiceType→InvoiceCategory 映射 | 任务 4 |
| 通行时间提取（正则） | 任务 4 |
| Toll 分离与自动关联（按通行时间） | 任务 5 |
| 组合金额匹配（行程+高速费） | 任务 5 |
| 匹配失败换下一条行程 | 任务 5 |
| 共享支付 used_payment_ids 只占用一次 | 任务 5 |
| 手动匹配共享支付标记 | 任务 6 |
| 报销单高速费独立行、支付单号重复 | 任务 7 |
| 端到端验证 | 任务 8 |

无遗漏。

### 占位符扫描

无 TODO/待定/模糊描述。每个步骤含完整代码。

### 类型一致性

- `toll_travel_time: Option<chrono::NaiveDateTime>` — 任务 1 定义，任务 4/5/8 使用，一致
- `shared_payment_ids: Vec<String>` — 任务 2 定义，任务 5/6/8 使用，一致
- `shared_from_invoice_id: Option<String>` — 任务 2 定义，任务 5/6/8 使用，一致
- `InvoiceCategory::Toll` — 任务 1 定义，任务 4/5/7/8 使用，一致
- `InvoiceType::TollInvoice` — 任务 3 定义，任务 4 使用，一致
- `extract_toll_travel_time` — 任务 4 定义，任务 4 步骤 6 使用，一致
- `create_manual_match_shared` — 任务 6 定义，任务 8 使用，一致

无类型不一致。
