# 票据城市 & 日期自动提取 — 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 从火车票/机票 OCR 文本中提取出发/到达城市，前端导出页一键填充目的地和出差日期

**架构：** 在 `parse_invoice_text()` 内部新增城市提取逻辑（利用函数内已有的 OCR 文本和已判定的 category），不修改 pipeline。城市名经归一化去站名/机场后缀。前端 ExportView 新增按钮触发提取。

**技术栈：** Rust (serde, regex, chrono) + Vue 3 (Composition API, Pinia)

---

### 任务 1：Invoice 模型扩展

**文件：**
- 修改：`src-tauri/src/models/invoice.rs:24-37`
- 修改：`src/types/invoice.ts:23-33`

- [ ] **步骤 1：Rust 端加字段**

在 `Invoice` 结构体末尾（`hotel_detail` 之后、`}` 之前）新增两个字段：

```rust
pub struct Invoice {
    pub id: String,
    pub invoice_number: String,
    pub amount: f64,
    pub seller_name: String,
    pub item_name: String,
    pub date: NaiveDate,
    pub category: InvoiceCategory,
    pub source: InvoiceSource,
    pub itineraries: Vec<Itinerary>,
    pub itinerary_file: Option<String>,
    pub remarks: String,
    pub hotel_detail: Option<HotelDetail>,
    // NEW: 票据出发/到达城市（仅 Train/Flight 类发票有值）
    pub departure_city: Option<String>,
    pub arrival_city: Option<String>,
}
```

- [ ] **步骤 2：TypeScript 端同步**

在 `src/types/invoice.ts` 的 `Invoice` 接口末尾新增：

```typescript
export interface Invoice {
  id: string
  invoice_number: string
  amount: number
  seller_name: string
  item_name: string
  date: string
  category: InvoiceCategory
  source: InvoiceSource
  itineraries: Itinerary[]
  // NEW
  departureCity?: string
  arrivalCity?: string
}
```

- [ ] **步骤 3：编译验证**

运行：`cd src-tauri && cargo check 2>&1`
预期：编译通过（可能有 2 处 `Invoice` 构造缺少新字段的报错，在任务 3 修复）

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/models/invoice.rs src/types/invoice.ts
git commit -m "feat(invoice): add departure_city and arrival_city fields to Invoice model"
```

---

### 任务 2：城市提取解析器

**文件：**
- 修改：`src-tauri/src/parser/invoice_parser.rs` — 新增 `extract_ticket_cities()` 和 `station_to_city()`

- [ ] **步骤 1：编写单元测试**

在 `src-tauri/src/parser/invoice_parser.rs` 的 `#[cfg(test)] mod tests` 块末尾（约 816 行后）新增：

```rust
#[test]
fn test_extract_ticket_cities_train() {
    let text = "出发站：北京南站\n到达站：上海虹桥站\n票价：553.00";
    let (dep, arr) = extract_ticket_cities(text, &InvoiceCategory::Train);
    assert_eq!(dep.as_deref(), Some("北京"));
    assert_eq!(arr.as_deref(), Some("上海"));
}

#[test]
fn test_extract_ticket_cities_flight() {
    let text = "自：北京首都国际机场\n至：上海浦东国际机场\n航班号：CA1234";
    let (dep, arr) = extract_ticket_cities(text, &InvoiceCategory::Flight);
    assert_eq!(dep.as_deref(), Some("北京"));
    assert_eq!(arr.as_deref(), Some("上海"));
}

#[test]
fn test_extract_ticket_cities_no_keyword() {
    let text = "这是普通的住宿发票";
    let (dep, arr) = extract_ticket_cities(text, &InvoiceCategory::Hotel);
    assert!(dep.is_none());
    assert!(arr.is_none());
}

#[test]
fn test_station_to_city_suffix_strip() {
    assert_eq!(station_to_city("上海虹桥站"), "上海");
    assert_eq!(station_to_city("广州南站"), "广州");
    assert_eq!(station_to_city("成都双流国际机场"), "成都");
}

#[test]
fn test_station_to_city_mapping() {
    assert_eq!(station_to_city("虹桥"), "上海");
    assert_eq!(station_to_city("宝安"), "深圳");
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：`cd src-tauri && cargo test test_extract_ticket_cities_train -- --nocapture 2>&1`
预期：FAIL，报错 `cannot find function extract_ticket_cities`

- [ ] **步骤 3：实现 `extract_ticket_cities()` 和 `station_to_city()`**

在 `invoice_parser.rs` 的 `split_into_regions` 之后（约 92 行）、`parse_invoice_text` 之前插入：

```rust
/// 从 OCR 文本中提取出发/到达城市（仅 Train/Flight 类发票）
fn extract_ticket_cities(text: &str, category: &InvoiceCategory) -> (Option<String>, Option<String>) {
    if *category != InvoiceCategory::Train && *category != InvoiceCategory::Flight {
        return (None, None);
    }

    let departure = if *category == InvoiceCategory::Train {
        // 火车票：出发站/发站
        let re = Regex::new(r"(?:出发站|发站)[：:]\s*(\S{2,6}(?:站)?)").unwrap();
        re.captures(text).map(|c| station_to_city(c.get(1).unwrap().as_str()))
    } else {
        // 机票：自/FROM
        let re = Regex::new(r"(?:自|FROM)[：:]\s*(\S{2,10})").unwrap();
        re.captures(text).map(|c| station_to_city(c.get(1).unwrap().as_str()))
    };

    let arrival = if *category == InvoiceCategory::Train {
        let re = Regex::new(r"(?:到达站|到站)[：:]\s*(\S{2,6}(?:站)?)").unwrap();
        re.captures(text).map(|c| station_to_city(c.get(1).unwrap().as_str()))
    } else {
        let re = Regex::new(r"(?:至|TO)[：:]\s*(\S{2,10})").unwrap();
        re.captures(text).map(|c| station_to_city(c.get(1).unwrap().as_str()))
    };

    (departure, arrival)
}

/// 站名/机场名归一化为城市名
fn station_to_city(raw: &str) -> String {
    let mut s = raw.trim().to_string();

    // 去除常见后缀（按序处理，长的先匹配）
    for suffix in &["国际机场", "机场", "东站", "西站", "南站", "北站", "站"] {
        if s.ends_with(suffix) {
            s = s[..s.len() - suffix.len()].to_string();
            break;
        }
    }

    // 去除机场三字码（如 PEK / SHA）
    let re_code = Regex::new(r#"\s*[A-Z]{3}$"#).unwrap();
    s = re_code.replace(&s, "").to_string();

    // 兜底映射表（已知片区/镇/区 → 城市）
    let mapping: std::collections::HashMap<&str, &str> = [
        ("虹桥", "上海"), ("宝安", "深圳"), ("江北", "重庆"),
        ("流亭", "青岛"), ("龙嘉", "长春"), ("太平", "哈尔滨"),
        ("遥墙", "济南"), ("周水子", "大连"), ("双流", "成都"),
        ("天河", "武汉"), ("黄花", "长沙"), ("咸阳", "西安"),
        ("滨海", "天津"), ("长水", "昆明"), ("萧山", "杭州"),
    ].iter().cloned().collect();

    if let Some(city) = mapping.get(s.as_str()) {
        return city.to_string();
    }

    // 如果已经是纯城市名（2-4 字），直接返回
    if s.chars().count() >= 2 && s.chars().count() <= 4 {
        return s;
    }

    raw.trim().to_string()
}
```

- [ ] **步骤 4：运行测试确认通过**

运行：`cd src-tauri && cargo test test_extract_ticket_cities -- --nocapture 2>&1`
预期：所有新增测试 PASS

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/parser/invoice_parser.rs
git commit -m "feat(parser): add extract_ticket_cities and station_to_city for train/flight tickets"
```

---

### 任务 3：流水线集成

**文件：**
- 修改：`src-tauri/src/parser/invoice_parser.rs:161-174` — 在 Invoice 构造处调用城市提取
- 修改：`src-tauri/src/pdf/invoice_pipeline.rs:186-199` — 虚拟发票构造处添加新字段

- [ ] **步骤 1：修改 `parse_invoice_text()` 的 Invoice 构造**

在 `parse_invoice_text()` 函数中（约 137 行 `let hotel_detail = if category ==` 之后），在构造 Invoice 之前，新增城市提取调用：

```rust
// 在 parse_invoice_text() 中，hotel_detail 之后，Ok(Invoice { ... }) 之前
// NEW: 提取票据出发/到达城市
let (departure_city, arrival_city) = extract_ticket_cities(&all_text, &category);
```

然后在 `Ok(Invoice { ... })` 构造中新增两个字段：

```rust
Ok(Invoice {
    id: Uuid::new_v4().to_string(),
    invoice_number,
    amount,
    seller_name,
    item_name,
    date,
    category,
    source,
    itineraries: vec![],
    itinerary_file: None,
    remarks: regions.remarks.clone(),
    hotel_detail,
    // NEW
    departure_city,
    arrival_city,
})
```

- [ ] **步骤 2：修改 pipeline 中虚拟发票构造**

在 `src-tauri/src/pdf/invoice_pipeline.rs:186-199` 的 `pair_invoices_with_itineraries()` 中，虚拟发票的构造也需要加新字段（None）：

```rust
invoices.push(Invoice {
    id,
    invoice_number: String::new(),
    amount: doc.total_amount,
    seller_name: "市内交通".to_string(),
    item_name: "市内交通".to_string(),
    date: chrono::NaiveDate::default(),
    category: InvoiceCategory::CityTransport,
    source: InvoiceSource::Pdf(doc.file_name.clone()),
    itineraries: doc.itineraries,
    itinerary_file: Some(doc.file_name.clone()),
    remarks: String::new(),
    hotel_detail: None,
    // NEW
    departure_city: None,
    arrival_city: None,
});
```

- [ ] **步骤 3：编译验证 & 运行现有测试**

```bash
cd src-tauri && cargo test 2>&1
```
预期：所有已有测试 PASS（新增字段 None 不影响现有逻辑）

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/parser/invoice_parser.rs src-tauri/src/pdf/invoice_pipeline.rs
git commit -m "feat(pipeline): integrate ticket city extraction into parse flow"
```

---

### 任务 4：前端提取按钮

**文件：**
- 修改：`src/views/ExportView.vue`

- [ ] **步骤 1：新增按钮和提取函数**

在 `ExportView.vue` 的 `<template>` 中，`<ReimbursementForm>` 上方添加按钮：

```vue
<!-- 从票据提取按钮 -->
<div class="mb-4">
  <button
    @click="extractTripFromTickets"
    class="px-4 py-2 rounded bg-green-500 text-white hover:bg-green-600 transition-colors text-sm"
  >
    🎫 从票据提取
  </button>
</div>
```

在 `<script setup>` 中新增函数：

```typescript
function extractTripFromTickets() {
  // 过滤 Train/Flight 类且有到达城市的发票
  const tickets = matchStore.matches
    .filter(m => {
      const inv = m.invoice
      return (inv.category === 'Train' || inv.category === 'Flight') && inv.arrivalCity
    })
    .map(m => m.invoice)

  if (tickets.length === 0) {
    alert('未找到可提取的火车票或机票')
    return
  }

  // 按日期排序
  tickets.sort((a, b) => a.date.localeCompare(b.date))

  // 目的地 = 最早一张票的到达城市
  formInfo.destination = tickets[0].arrivalCity!

  // 日期范围 = min/max
  formInfo.travelStart = tickets[0].date
  formInfo.travelEnd = tickets[tickets.length - 1].date
}
```

- [ ] **步骤 2：手动验证**

```bash
cd src && npm run dev 2>&1
```
预期：编译通过
- 手动测试：无票据时点击 → alert 提示
- 手动测试：有火车票时点击 → 表单填充

- [ ] **步骤 3：Commit**

```bash
git add src/views/ExportView.vue
git commit -m "feat(ui): add extract trip info from tickets button in ExportView"
```

---

### 任务 5：双向验证

- [ ] **步骤 1：Rust 单元测试**

运行：`cd src-tauri && cargo test test_extract_ticket_cities -- --nocapture 2>&1`
预期：5 个新增测试全部 PASS

- [ ] **步骤 2：全量回归测试**

运行：`cd src-tauri && cargo test 2>&1`
预期：所有已有测试 PASS（共 ~20+ 个测试）

- [ ] **步骤 3：前端编译验证**

运行：`cd src && npm run build 2>&1`
预期：无类型错误，构建成功

- [ ] **步骤 4：Commit（如需修复）**

若有修复，提交：
```bash
git commit -am "fix: address verification issues"
```
