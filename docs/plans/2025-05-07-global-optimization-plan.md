# 发票报销系统全局优化方案

> **目标**: 从架构层面系统性解决OCR识别率低、信息提取不完整、匹配不准确等问题，构建可扩展、可维护、高精度的智能处理系统

**版本**: 1.0  
**日期**: 2026-05-07  
**状态**: 规划中

---

## 问题根因分析

### 1. 测试数据回顾

| 指标 | 当前值 | 目标值 | 差距 |
|------|--------|--------|------|
| OCR识别成功率 | 64% (9/14) | >95% | -31% |
| 发票信息完整度 | seller_name: 0%<br>category: 100% Other | >90% | -90% |
| 匹配成功率 | 67% (6/9) | >90% | -23% |

### 2. 架构层面问题

#### 2.1 OCR结果处理缺陷

**当前实现**:
```rust
let all_text: String = texts
    .iter()
    .map(|t| t.text.as_str())
    .collect::<Vec<_>>()
    .join(" ");
```

**问题**:
- OCR输出是**结构化数据**（文本块 + 坐标 + 置信度），却被降维成纯字符串
- 丢失了空间布局信息（上下左右关系）
- 丢失了置信度信息（用于判断提取可靠性）
- 丢失了语义块边界（不同字段混在一起）

**影响**:
- 关键词匹配容易误判
- 无法区分多个相同关键词（如多个"名称"）
- 无法利用位置关系提取字段（如"名称:"后紧跟的内容）

#### 2.2 发票解析器设计缺陷

**当前实现**: 单一正则匹配

```rust
fn extract_seller_name(text: &str) -> String {
    let re = Regex::new(r"(?:销售方|收款单位|开票方)[：:]\s*(.+?)(?:\s|$)").unwrap();
    if let Some(caps) = re.captures(text) {
        return caps[1].trim().to_string();
    }
    String::new()
}
```

**问题**:
- 只能识别固定前缀，无法适应发票格式多样性
- 正则匹配贪婪/非贪婪控制不精准
- 无fallback机制，一次失败即返回空
- 未考虑OCR识别误差（错字、漏字、多余空格）

**真实案例**（从测试日志）:
```
OCR输出: "名称：四川景澜酒店管理有限公司"
当前解析: 返回空字符串（因为关键词是"名称"而非"销售方"）
```

#### 2.3 分类逻辑缺陷

**当前实现**: 仅依赖seller_name + item_name

```rust
pub fn classify_invoice(seller_name: &str, item_name: &str) -> InvoiceCategory {
    let combined = format!("{} {}", seller_name, item_name);
    // ...
}
```

**问题**:
- seller_name提取失败 → 分类必然失败
- 未利用OCR全文中的丰富信息（如"*住宿服务*"、"*运输服务*"等税务分类代码）
- 未考虑发票类型识别（增值税发票、行程单、机票等格式差异）

#### 2.4 匹配算法单一

**当前实现**: 纯金额匹配

```rust
let diff = (invoice.amount - payment.amount).abs();
if diff <= self.tolerance {
    return Some(MatchResult { /* ... */ });
}
```

**问题**:
- 仅依赖金额，忽略了其他强特征（商户名称、时间、地点）
- 无法处理金额接近的多笔支付（如¥100.00 vs ¥99.80）
- 未利用发票分类信息（Hotel类发票应匹配酒店类支付）

---

## 优化方案架构

### 核心设计理念

1. **多策略提取**: 单字段支持多种提取方式，依次fallback
2. **结构化解析**: 保留OCR空间信息，基于布局理解发票结构
3. **多维度匹配**: 综合金额、商户、时间、分类多维度打分
4. **可配置规则**: 支持模板配置，无需修改代码即可适应新发票格式
5. **渐进式优化**: 从粗到精，先保证基础功能可用，再逐步提升精度

### 模块架构图

```
┌─────────────────────────────────────────────────────────────┐
│                        OCR引擎层                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                 │
│  │ PaddleOCR│  │ PDF渲染  │  │ 置信度   │                 │
│  │ v4/v5    │  │ pdfium   │  │ 过滤     │                 │
│  └──────────┘  └──────────┘  └──────────┘                 │
└───────────────────────┬─────────────────────────────────────┘
                        │ OcrStructuredOutput
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                    发票解析器层 (Parser)                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ 发票类型识别 │  │ 多策略提取器 │  │ 模板匹配引擎 │     │
│  │ (VAT/Itinerary)│  │ (Regex/ML/Rule)│  │ (配置驱动)   │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└───────────────────────┬─────────────────────────────────────┘
                        │ StructuredInvoice
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                    匹配引擎层 (Matcher)                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ 多维度评分   │  │ 组合匹配策略 │  │ 学习优化     │     │
│  │ (金额/名称/时间)│  │ (1:1, 1:N, N:N)│  │ (历史数据)   │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└───────────────────────┬─────────────────────────────────────┘
                        │ MatchResult
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                      应用层 (Tauri)                          │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase 1: OCR结果结构化处理（P0 - 核心基础）

### 1.1 定义OCR结构化输出模型

**目标**: 保留OCR输出的完整信息，包括文本、位置、置信度

**实现**:

```rust
// src-tauri/src/ocr/structured_output.rs

/// OCR识别的结构化文本块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrTextBlock {
    pub text: String,
    pub confidence: f64,
    pub bbox: BoundingBox,          // 边界框
    pub line_index: usize,          // 所在行号
    pub block_type: TextBlockType,  // 文本块类型
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextBlockType {
    Title,      // 标题（如"增值税电子发票"）
    KeyValue,   // 键值对（如"金额：¥100.00"）
    Table,      // 表格
    Other,      // 其他
}

/// OCR页面结构化输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrStructuredOutput {
    pub blocks: Vec<OcrTextBlock>,
    pub layout: PageLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageLayout {
    pub width: f64,
    pub height: f64,
    pub text_regions: Vec<TextRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRegion {
    pub region_type: RegionType,
    pub bbox: BoundingBox,
    pub block_indices: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegionType {
    Header,     // 发票头部
    Body,       // 发票主体
    Table,      // 明细表格
    Footer,     // 发票尾部
}
```

### 1.2 OCR结果后处理

**目标**: 对原始OCR输出进行清洗、过滤、聚类

**实现**:

```rust
impl OcrEngine {
    /// 将原始OCR结果转换为结构化输出
    pub fn process_to_structured(
        &mut self,
        file_path: &str,
    ) -> Result<OcrStructuredOutput, String> {
        let raw_response = self.recognize_pdf_first_page(file_path)?;
        
        // Step 1: 过滤低置信度文本
        let filtered: Vec<OcrTextItem> = raw_response.texts
            .into_iter()
            .filter(|t| t.confidence > 0.6)  // 置信度阈值
            .collect();
        
        // Step 2: 解析边界框
        let mut blocks: Vec<OcrTextBlock> = Vec::new();
        for (idx, item) in filtered.iter().enumerate() {
            let bbox = parse_box_coords(&item.box_coords)?;
            blocks.push(OcrTextBlock {
                text: item.text.clone(),
                confidence: item.confidence,
                bbox,
                line_index: idx,
                block_type: infer_block_type(&item.text),
            });
        }
        
        // Step 3: 聚类文本区域（基于空间位置）
        let regions = cluster_text_regions(&blocks);
        
        Ok(OcrStructuredOutput {
            blocks,
            layout: PageLayout {
                width: 0.0,  // TODO: 从PDF页面获取
                height: 0.0,
                text_regions: regions,
            },
        })
    }
}

/// 基于空间位置聚类文本区域
fn cluster_text_regions(blocks: &[OcrTextBlock]) -> Vec<TextRegion> {
    // 使用简单的启发式规则：
    // - 顶部（y < height*0.2）: Header
    // - 底部（y > height*0.8）: Footer
    // - 中间: Body
    vec![]  // TODO: 实现聚类算法
}

/// 推断文本块类型
fn infer_block_type(text: &str) -> TextBlockType {
    if text.contains("发票") || text.contains("凭证") {
        TextBlockType::Title
    } else if text.contains("：") || text.contains(":") {
        TextBlockType::KeyValue
    } else {
        TextBlockType::Other
    }
}
```

**预期收益**:
- ✅ 保留完整信息，不再丢失空间布局
- ✅ 为后续解析提供更丰富的输入
- ✅ 可基于置信度判断提取可靠性

---

## Phase 2: 发票解析器重构（P0 - 核心功能）

### 2.1 发票类型识别器

**目标**: 在解析字段前，先识别发票类型，应用对应的解析策略

**实现**:

```rust
// src-tauri/src/parser/invoice_type_detector.rs

#[derive(Debug, Clone)]
pub enum InvoiceType {
    VatElectronicInvoice,     // 增值税电子发票
    VatSpecialInvoice,        // 增值税专用发票
    RideHailingInvoice,       // 网约车发票
    RideHailingItinerary,     // 网约车行程单
    FlightInvoice,            // 机票发票
    TrainInvoice,             // 火车票
    HotelStatement,           // 酒店结账单
    TransitCardStatement,     // 公交卡行程单（天府通等）
    Other,                    // 其他
}

pub struct InvoiceTypeDetector;

impl InvoiceTypeDetector {
    /// 根据OCR内容识别发票类型
    pub fn detect(ocr_output: &OcrStructuredOutput) -> InvoiceType {
        let all_text = ocr_output.blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        
        // 优先级从高到低匹配
        if Self::is_ride_hailing_itinerary(&all_text) {
            return InvoiceType::RideHailingItinerary;
        }
        
        if Self::is_transit_card_statement(&all_text) {
            return InvoiceType::TransitCardStatement;
        }
        
        if Self::is_flight_invoice(&all_text) {
            return InvoiceType::FlightInvoice;
        }
        
        if Self::is_vat_electronic_invoice(&all_text) {
            return InvoiceType::VatElectronicInvoice;
        }
        
        if Self::is_hotel_statement(&all_text) {
            return InvoiceType::HotelStatement;
        }
        
        InvoiceType::Other
    }
    
    fn is_ride_hailing_itinerary(text: &str) -> bool {
        text.contains("行程报销单") || 
        text.contains("行程单") && (text.contains("滴滴") || text.contains("高德"))
    }
    
    fn is_transit_card_statement(text: &str) -> bool {
        text.contains("天府通") || text.contains("电子行程单")
    }
    
    fn is_flight_invoice(text: &str) -> bool {
        text.contains("机票") || text.contains("航班") || text.contains("航空")
    }
    
    fn is_vat_electronic_invoice(text: &str) -> bool {
        text.contains("增值税") && text.contains("电子发票")
    }
    
    fn is_hotel_statement(text: &str) -> bool {
        text.contains("结账单") && text.contains("酒店")
    }
}
```

### 2.2 多策略字段提取器

**目标**: 每个字段支持多种提取方式，依次尝试直到成功

**设计**:

```rust
// src-tauri/src/parser/field_extractors.rs

/// 字段提取策略接口
pub trait FieldExtractor: Send + Sync {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField>;
}

#[derive(Debug, Clone)]
pub struct ExtractedField {
    pub value: String,
    pub confidence: f64,
    pub strategy: String,  // 使用的策略名称
    pub source_position: Option<BoundingBox>,
}

/// 销售方名称提取器
pub struct SellerNameExtractor {
    strategies: Vec<Box<dyn FieldExtractor>>,
}

impl SellerNameExtractor {
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Box::new(RegexStrategy::new(
                    "销售方名称",
                    &[
                        r"销售方[：:]\s*名称[：:]\s*(.+?)(?=\s|$)",
                        r"名称[：:]\s*(.+?)(?=\s+统一社会信用代码|$)",
                        r"收款单位[：:]\s*(.+?)(?=\s|$)",
                    ],
                )),
                Box::new(KeyValueProximityStrategy::new(
                    "销售方信息",  // 在OCR中寻找"销售方信息"区块
                    "名称",        // 然后找"名称"字段
                )),
                Box::new(ContextualStrategy::new(
                    "增值税发票销售方",  // 针对增值税发票的特殊逻辑
                )),
            ],
        }
    }
    
    pub fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        for strategy in &self.strategies {
            if let Some(field) = strategy.extract(ocr) {
                return Some(field);
            }
        }
        None
    }
}

/// 正则表达式提取策略
pub struct RegexStrategy {
    name: String,
    patterns: Vec<Regex>,
}

impl RegexStrategy {
    pub fn new(name: &str, patterns: &[&str]) -> Self {
        Self {
            name: name.to_string(),
            patterns: patterns.iter()
                .filter_map(|p| Regex::new(p).ok())
                .collect(),
        }
    }
}

impl FieldExtractor for RegexStrategy {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        let text = ocr.blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        
        for pattern in &self.patterns {
            if let Some(caps) = pattern.captures(&text) {
                if let Some(value) = caps.get(1) {
                    return Some(ExtractedField {
                        value: value.as_str().trim().to_string(),
                        confidence: 0.9,  // 正则匹配默认置信度
                        strategy: format!("regex:{}", self.name),
                        source_position: None,
                    });
                }
            }
        }
        None
    }
}

/// 基于空间邻近的键值对提取策略
pub struct KeyValueProximityStrategy {
    section_keyword: String,
    field_keyword: String,
}

impl KeyValueProximityStrategy {
    pub fn new(section_keyword: &str, field_keyword: &str) -> Self {
        Self {
            section_keyword: section_keyword.to_string(),
            field_keyword: field_keyword.to_string(),
        }
    }
}

impl FieldExtractor for KeyValueProximityStrategy {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        // 1. 找到包含section_keyword的文本块
        let section_block = ocr.blocks.iter()
            .find(|b| b.text.contains(&self.section_keyword))?;
        
        // 2. 找到该section内（空间邻近）包含field_keyword的文本块
        let field_block = ocr.blocks.iter()
            .filter(|b| {
                // 在section_block下方且垂直距离较近
                b.bbox.y > section_block.bbox.y &&
                (b.bbox.y - section_block.bbox.y) < 200.0  // TODO: 根据页面高度动态调整
            })
            .find(|b| b.text.contains(&self.field_keyword))?;
        
        // 3. 提取field_keyword后的内容
        let text = &field_block.text;
        if let Some(pos) = text.find(&self.field_keyword) {
            let value_start = pos + self.field_keyword.len();
            let value = text[value_start..].trim_start_matches(|c| c == '：' || c == ':')
                .split_whitespace()
                .next()?
                .to_string();
            
            return Some(ExtractedField {
                value,
                confidence: field_block.confidence,
                strategy: format!("proximity:{}:{}", self.section_keyword, self.field_keyword),
                source_position: Some(field_block.bbox.clone()),
            });
        }
        
        None
    }
}

/// 基于发票类型上下文的提取策略
pub struct ContextualStrategy {
    invoice_type: String,
}

impl ContextualStrategy {
    pub fn new(invoice_type: &str) -> Self {
        Self {
            invoice_type: invoice_type.to_string(),
        }
    }
}

impl FieldExtractor for ContextualStrategy {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        // TODO: 根据发票类型实现特定逻辑
        // 例如：增值税发票的销售方在"销售方信息"区块
        // 行程单的提供商在标题中
        None
    }
}
```

### 2.3 发票解析器重构

**目标**: 组合多个提取器，提供统一的解析接口

**实现**:

```rust
// src-tauri/src/parser/invoice_parser.rs

pub struct InvoiceParser {
    type_detector: InvoiceTypeDetector,
    seller_name_extractor: SellerNameExtractor,
    item_name_extractor: ItemNameExtractor,
    amount_extractor: AmountExtractor,
    date_extractor: DateExtractor,
    invoice_number_extractor: InvoiceNumberExtractor,
}

impl InvoiceParser {
    pub fn new() -> Self {
        Self {
            type_detector: InvoiceTypeDetector,
            seller_name_extractor: SellerNameExtractor::new(),
            item_name_extractor: ItemNameExtractor::new(),
            amount_extractor: AmountExtractor::new(),
            date_extractor: DateExtractor::new(),
            invoice_number_extractor: InvoiceNumberExtractor::new(),
        }
    }
    
    /// 解析OCR输出为结构化发票
    pub fn parse(
        &self,
        ocr_output: &OcrStructuredOutput,
        source: InvoiceSource,
    ) -> Result<Invoice, ParseError> {
        // Step 1: 识别发票类型
        let invoice_type = self.type_detector.detect(ocr_output);
        
        // Step 2: 提取各字段（带置信度）
        let amount_field = self.amount_extractor.extract(ocr_output)
            .ok_or(ParseError::MissingField("amount"))?;
        
        let seller_field = self.seller_name_extractor.extract(ocr_output);
        let item_field = self.item_name_extractor.extract(ocr_output);
        let date_field = self.date_extractor.extract(ocr_output);
        let number_field = self.invoice_number_extractor.extract(ocr_output);
        
        // Step 3: 分类（基于全文内容）
        let category = self.classify_from_full_text(ocr_output, &seller_field, &item_field);
        
        // Step 4: 构建发票对象
        Ok(Invoice {
            id: Uuid::new_v4().to_string(),
            invoice_number: number_field.map(|f| f.value).unwrap_or_default(),
            amount: amount_field.value.parse()
                .map_err(|_| ParseError::InvalidField("amount"))?,
            seller_name: seller_field.map(|f| f.value).unwrap_or_default(),
            item_name: item_field.map(|f| f.value).unwrap_or_default(),
            date: date_field
                .and_then(|f| parse_date(&f.value))
                .unwrap_or_default(),
            category,
            source,
            itineraries: vec![],  // TODO: 从行程单提取
        })
    }
    
    /// 基于OCR全文内容进行分类（不依赖单一字段）
    fn classify_from_full_text(
        &self,
        ocr: &OcrStructuredOutput,
        seller: &Option<ExtractedField>,
        item: &Option<ExtractedField>,
    ) -> InvoiceCategory {
        let all_text = ocr.blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        
        // 优先级1: 税务分类代码（最可靠）
        if all_text.contains("*住宿服务*") {
            return InvoiceCategory::Hotel;
        }
        if all_text.contains("*运输服务*") || all_text.contains("*客运服务*") {
            return InvoiceCategory::CityTransport;
        }
        if all_text.contains("*航空运输服务*") {
            return InvoiceCategory::Flight;
        }
        
        // 优先级2: 关键词组合
        if contains_any(&all_text, &["酒店", "宾馆", "住宿", "招待所", "民宿"]) {
            return InvoiceCategory::Hotel;
        }
        if contains_any(&all_text, &["滴滴", "网约车", "高德", "t3", "曹操", "出租"]) {
            return InvoiceCategory::CityTransport;
        }
        if contains_any(&all_text, &["航空", "机票", "机场", "航班"]) {
            return InvoiceCategory::Flight;
        }
        if contains_any(&all_text, &["铁路", "高铁", "火车", "客运站"]) {
            return InvoiceCategory::Train;
        }
        if contains_any(&all_text, &["餐饮", "饭店", "食品", "餐厅", "饭馆"]) {
            return InvoiceCategory::Meal;
        }
        if contains_any(&all_text, &["退票", "改签", "保险"]) {
            return InvoiceCategory::TicketChange;
        }
        
        InvoiceCategory::Other
    }
}
```

**预期收益**:
- ✅ 提取成功率从 <10% 提升到 >80%
- ✅ 支持多种发票格式，无需硬编码
- ✅ 提供置信度信息，便于后续人工确认

---

## Phase 3: 匹配引擎增强（P1 - 重要功能）

### 3.1 多维度评分机制

**目标**: 综合金额、商户名称、时间、分类等多个维度进行匹配评分

**实现**:

```rust
// src-tauri/src/matching/scoring.rs

#[derive(Debug, Clone)]
pub struct MatchScore {
    pub total: f64,
    pub amount_score: f64,
    pub merchant_score: f64,
    pub time_score: f64,
    pub category_score: f64,
    pub breakdown: ScoreBreakdown,
}

#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    pub amount_diff: f64,
    pub merchant_similarity: f64,
    pub time_diff_hours: f64,
    pub category_match: bool,
}

pub struct MultiDimensionalScorer {
    weights: ScoringWeights,
}

#[derive(Debug, Clone)]
pub struct ScoringWeights {
    pub amount: f64,      // 金额权重
    pub merchant: f64,    // 商户名称权重
    pub time: f64,        // 时间权重
    pub category: f64,    // 分类权重
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            amount: 0.4,
            merchant: 0.3,
            time: 0.2,
            category: 0.1,
        }
    }
}

impl MultiDimensionalScorer {
    pub fn new(weights: ScoringWeights) -> Self {
        Self { weights }
    }
    
    /// 计算发票与支付记录的匹配得分
    pub fn score(
        &self,
        invoice: &Invoice,
        payment: &PaymentRecord,
    ) -> MatchScore {
        let amount_score = self.score_amount(invoice.amount, payment.amount);
        let merchant_score = self.score_merchant(&invoice.seller_name, &payment.merchant_name);
        let time_score = self.score_time(&invoice.date, &payment.transaction_time);
        let category_score = self.score_category(&invoice.category, &payment.category);
        
        let total = amount_score * self.weights.amount
            + merchant_score * self.weights.merchant
            + time_score * self.weights.time
            + category_score * self.weights.category;
        
        MatchScore {
            total,
            amount_score,
            merchant_score,
            time_score,
            category_score,
            breakdown: ScoreBreakdown {
                amount_diff: (invoice.amount - payment.amount).abs(),
                merchant_similarity: merchant_score,
                time_diff_hours: 0.0,  // TODO: 计算时间差
                category_match: category_score > 0.5,
            },
        }
    }
    
    /// 金额匹配评分（差值越小得分越高）
    fn score_amount(&self, invoice_amount: f64, payment_amount: f64) -> f64 {
        let diff = (invoice_amount - payment_amount).abs();
        let tolerance = 5.0;  // 容差5元
        
        if diff == 0.0 {
            1.0  // 完美匹配
        } else if diff <= tolerance {
            1.0 - (diff / tolerance) * 0.5  // 0.5-1.0之间
        } else {
            0.0  // 不匹配
        }
    }
    
    /// 商户名称相似度评分
    fn score_merchant(&self, invoice_seller: &str, payment_merchant: &str) -> f64 {
        if invoice_seller.is_empty() || payment_merchant.is_empty() {
            return 0.0;  // 无法判断
        }
        
        // 策略1: 完全匹配
        if invoice_seller == payment_merchant {
            return 1.0;
        }
        
        // 策略2: 包含关系
        if invoice_seller.contains(payment_merchant) || payment_merchant.contains(invoice_seller) {
            return 0.9;
        }
        
        // 策略3: 编辑距离相似度
        let similarity = self.levenshtein_similarity(invoice_seller, payment_merchant);
        if similarity > 0.7 {
            return similarity;
        }
        
        // 策略4: 关键词匹配（酒店连锁、滴滴等）
        self.keyword_matching_score(invoice_seller, payment_merchant)
    }
    
    /// 时间匹配评分
    fn score_time(&self, invoice_date: &NaiveDate, payment_time: &str) -> f64 {
        // 解析支付时间
        let payment_date = parse_datetime(payment_time)
            .map(|dt| dt.date())
            .unwrap_or_default();
        
        let days_diff = (*invoice_date - payment_date).num_days().abs();
        
        if days_diff == 0 {
            1.0
        } else if days_diff <= 1 {
            0.9
        } else if days_diff <= 3 {
            0.7
        } else if days_diff <= 7 {
            0.5
        } else {
            0.0
        }
    }
    
    /// 分类匹配评分
    fn score_category(&self, invoice_category: &InvoiceCategory, payment_category: &str) -> f64 {
        // 构建分类映射
        let category_keywords = match invoice_category {
            InvoiceCategory::Hotel => &["酒店", "住宿", "宾馆"],
            InvoiceCategory::CityTransport => &["滴滴", "高德", "交通", "出租"],
            InvoiceCategory::Flight => &["航空", "机票", "航班"],
            InvoiceCategory::Train => &["铁路", "高铁", "火车"],
            InvoiceCategory::Meal => &["餐饮", "饭店", "食品"],
            _ => return 0.5,  // 默认中等得分
        };
        
        if category_keywords.iter().any(|k| payment_category.contains(k)) {
            1.0
        } else {
            0.0
        }
    }
    
    /// Levenshtein相似度
    fn levenshtein_similarity(&self, s1: &str, s2: &str) -> f64 {
        let len1 = s1.chars().count();
        let len2 = s2.chars().count();
        
        if len1 == 0 || len2 == 0 {
            return 0.0;
        }
        
        let distance = levenshtein_distance(s1, s2);
        1.0 - (distance as f64 / (len1.max(len2) as f64))
    }
    
    /// 关键词匹配评分（处理连锁店、平台名称等）
    fn keyword_matching_score(&self, seller: &str, merchant: &str) -> f64 {
        // 酒店连锁：如"如家酒店" vs "如家"
        let hotel_brands = ["如家", "汉庭", "锦江", "7天", "华住", "希尔顿", "万豪"];
        for brand in &hotel_brands {
            if seller.contains(brand) && merchant.contains(brand) {
                return 0.85;
            }
        }
        
        // 打车平台：如"滴滴出行" vs "滴滴"
        let ride_brands = ["滴滴", "高德", "T3", "曹操"];
        for brand in &ride_brands {
            if seller.contains(brand) && merchant.contains(brand) {
                return 0.85;
            }
        }
        
        0.0
    }
}
```

### 3.2 智能匹配策略选择器

**目标**: 根据发票类型和场景自动选择匹配策略

**实现**:

```rust
// src-tauri/src/matching/strategy_selector.rs

pub enum MatchingStrategy {
    StrictAmountOnly,        // 仅金额匹配（高精度）
    AmountWithMerchant,      // 金额+商户名称
    AmountWithTime,          // 金额+时间
    MultiDimensional,        // 多维度综合评分
    OneToMany,               // 一对多匹配（打车行程）
    FuzzyMatching,           // 模糊匹配（低置信度场景）
}

pub struct StrategySelector;

impl StrategySelector {
    pub fn select(
        invoice: &Invoice,
        payment_count: usize,
    ) -> MatchingStrategy {
        // 优先级1: 打车行程单 → 一对多匹配
        if invoice.category == InvoiceCategory::CityTransport 
            && !invoice.itineraries.is_empty() {
            return MatchingStrategy::OneToMany;
        }
        
        // 优先级2: 有销售方名称 → 金额+商户名称
        if !invoice.seller_name.is_empty() {
            return MatchingStrategy::AmountWithMerchant;
        }
        
        // 优先级3: 支付记录较少 → 多维度综合
        if payment_count < 50 {
            return MatchingStrategy::MultiDimensional;
        }
        
        // 默认: 多维度综合
        MatchingStrategy::MultiDimensional
    }
}
```

### 3.3 批量匹配优化

**目标**: 使用优先队列和剪枝优化匹配性能

**实现**:

```rust
// src-tauri/src/matching/batch_optimizer.rs

use std::collections::BinaryHeap;

pub struct BatchMatchOptimizer {
    scorer: MultiDimensionalScorer,
    strategy_selector: StrategySelector,
}

impl BatchMatchOptimizer {
    /// 批量匹配（带优先队列优化）
    pub fn batch_match(
        &self,
        invoices: &[Invoice],
        payments: &[PaymentRecord],
    ) -> BatchMatchResult {
        let mut matched = Vec::new();
        let mut unmatched_invoices = Vec::new();
        let mut used_payments: HashSet<String> = HashSet::new();
        
        // Step 1: 为每张发票生成候选支付列表
        for invoice in invoices {
            let strategy = self.strategy_selector.select(invoice, payments.len());
            
            // Step 2: 计算所有候选的得分
            let candidates: Vec<(f64, &PaymentRecord)> = payments
                .iter()
                .filter(|p| !used_payments.contains(&p.id))
                .filter_map(|p| {
                    let score = self.scorer.score(invoice, p);
                    if score.total > 0.5 {  // 阈值过滤
                        Some((score.total, p))
                    } else {
                        None
                    }
                })
                .collect();
            
            // Step 3: 选择最佳匹配
            if let Some((best_score, best_payment)) = candidates.iter()
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap()) {
                
                if *best_score > 0.7 {  // 高置信度阈值
                    matched.push(self.create_match_result(
                        invoice,
                        vec![(*best_payment).clone()],
                        *best_score,
                    ));
                    used_payments.insert(best_payment.id.clone());
                } else {
                    unmatched_invoices.push(invoice.clone());
                }
            } else {
                unmatched_invoices.push(invoice.clone());
            }
        }
        
        let unmatched_payments = payments
            .iter()
            .filter(|p| !used_payments.contains(&p.id))
            .cloned()
            .collect();
        
        BatchMatchResult {
            matched,
            unmatched_invoices,
            unmatched_payments,
        }
    }
}
```

**预期收益**:
- ✅ 匹配准确率从 67% 提升到 >90%
- ✅ 支持模糊匹配场景
- ✅ 提供匹配原因说明（金额、商户、时间等）
- ✅ 性能优化（优先队列+阈值剪枝）

---

## Phase 4: 模板配置系统（P2 - 扩展性）

### 4.1 模板定义

**目标**: 支持JSON配置发票模板，无需修改代码

**实现**:

```json
// config/invoice_templates/vat_electronic.json
{
  "template_id": "vat_electronic_invoice",
  "name": "增值税电子发票",
  "keywords": ["增值税", "电子发票", "价税合计"],
  "fields": [
    {
      "name": "amount",
      "required": true,
      "strategies": [
        {
          "type": "regex",
          "pattern": "价税合计[：:￥¥]*\\s*([\\d,]+\\.?\\d*)",
          "confidence": 0.9
        },
        {
          "type": "regex",
          "pattern": "合计金额[：:￥¥]*\\s*([\\d,]+\\.?\\d*)",
          "confidence": 0.8
        }
      ]
    },
    {
      "name": "seller_name",
      "required": true,
      "strategies": [
        {
          "type": "section_field",
          "section_keyword": "销售方信息",
          "field_keyword": "名称",
          "confidence": 0.95
        },
        {
          "type": "regex",
          "pattern": "名称[：:](.+?)(?=\\s+统一社会信用代码|$)",
          "confidence": 0.85
        }
      ]
    },
    {
      "name": "invoice_number",
      "required": true,
      "strategies": [
        {
          "type": "regex",
          "pattern": "发票号码[：:]\\s*(\\d+)",
          "confidence": 0.95
        }
      ]
    }
  ]
}
```

### 4.2 模板管理器

```rust
// src-tauri/src/parser/template_manager.rs

pub struct TemplateManager {
    templates: HashMap<String, InvoiceTemplate>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InvoiceTemplate {
    pub template_id: String,
    pub name: String,
    pub keywords: Vec<String>,
    pub fields: Vec<FieldDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub required: bool,
    pub strategies: Vec<FieldStrategy>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldStrategy {
    #[serde(rename = "type")]
    pub strategy_type: String,
    pub pattern: Option<String>,
    pub section_keyword: Option<String>,
    pub field_keyword: Option<String>,
    pub confidence: f64,
}

impl TemplateManager {
    pub fn from_config_dir(dir: &str) -> Result<Self, String> {
        // 加载所有JSON模板文件
        let mut templates = HashMap::new();
        
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                let template: InvoiceTemplate = serde_json::from_str(&content)
                    .map_err(|e| format!("Failed to parse template {:?}: {}", path, e))?;
                templates.insert(template.template_id.clone(), template);
            }
        }
        
        Ok(Self { templates })
    }
    
    /// 根据OCR内容匹配合适的模板
    pub fn match_template(&self, ocr: &OcrStructuredOutput) -> Option<&InvoiceTemplate> {
        let all_text = ocr.blocks.iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        
        self.templates.values()
            .find(|t| {
                t.keywords.iter().all(|k| all_text.contains(k))
            })
    }
}
```

---

## Phase 5: 测试与验证（P0 - 必须完成）

### 5.1 单元测试扩展

**目标**: 为每个提取策略编写单元测试

**测试用例**:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_seller_name_extraction_from_vat_invoice() {
        let ocr_blocks = vec![
            OcrTextBlock {
                text: "销售方信息".to_string(),
                confidence: 0.99,
                bbox: BoundingBox { x: 0.0, y: 100.0, width: 100.0, height: 20.0 },
                line_index: 0,
                block_type: TextBlockType::Title,
            },
            OcrTextBlock {
                text: "名称：四川景澜酒店管理有限公司".to_string(),
                confidence: 0.95,
                bbox: BoundingBox { x: 0.0, y: 120.0, width: 200.0, height: 20.0 },
                line_index: 1,
                block_type: TextBlockType::KeyValue,
            },
        ];
        
        let ocr_output = OcrStructuredOutput {
            blocks: ocr_blocks,
            layout: PageLayout::default(),
        };
        
        let extractor = SellerNameExtractor::new();
        let result = extractor.extract(&ocr_output);
        
        assert!(result.is_some());
        let field = result.unwrap();
        assert_eq!(field.value, "四川景澜酒店管理有限公司");
        assert!(field.confidence > 0.8);
    }
    
    #[test]
    fn test_amount_extraction_multiple_strategies() {
        // 测试多种金额格式的提取
        let test_cases = vec![
            ("价税合计：¥1045.24", 1045.24),
            ("合计金额：100.00元", 100.0),
            ("￥523.57", 523.57),
            ("总金额1,234.56", 1234.56),
        ];
        
        for (text, expected) in test_cases {
            let blocks = vec![OcrTextBlock {
                text: text.to_string(),
                confidence: 0.99,
                bbox: BoundingBox::default(),
                line_index: 0,
                block_type: TextBlockType::KeyValue,
            }];
            
            let ocr = OcrStructuredOutput {
                blocks,
                layout: PageLayout::default(),
            };
            
            let extractor = AmountExtractor::new();
            let result = extractor.extract(&ocr);
            
            assert!(result.is_some());
            let value: f64 = result.unwrap().value.parse().unwrap();
            assert!((value - expected).abs() < 0.01);
        }
    }
}
```

### 5.2 集成测试

**目标**: 使用真实发票数据验证完整流程

```rust
#[test]
fn test_real_invoice_parsing() {
    let test_files = vec![
        "data/发票与行程单/dzfp_26512000001728418261.pdf",
        "data/发票与行程单/滴滴电子发票A.pdf",
        "data/发票与行程单/【飞猪】成都-长沙.pdf",
    ];
    
    let mut engine = OcrEngine::new("models").unwrap();
    let parser = InvoiceParser::new();
    
    for file_path in &test_files {
        let ocr_output = engine.process_to_structured(file_path).unwrap();
        let result = parser.parse(&ocr_output, InvoiceSource::Pdf(file_path.to_string()));
        
        assert!(result.is_ok());
        let invoice = result.unwrap();
        
        // 验证必填字段
        assert!(invoice.amount > 0.0);
        assert!(!invoice.invoice_number.is_empty());
        
        // 验证分类
        assert!(!matches!(invoice.category, InvoiceCategory::Other));
    }
}
```

### 5.3 性能基准测试

```rust
#[test]
fn test_matching_performance() {
    // 生成测试数据
    let invoices = generate_test_invoices(100);
    let payments = generate_test_payments(1000);
    
    let optimizer = BatchMatchOptimizer::new(ScoringWeights::default());
    
    let start = std::time::Instant::now();
    let result = optimizer.batch_match(&invoices, &payments);
    let duration = start.elapsed();
    
    println!("Matching time: {:?}", duration);
    println!("Matched: {}", result.matched.len());
    println!("Unmatched invoices: {}", result.unmatched_invoices.len());
    
    assert!(duration.as_millis() < 1000);  // < 1秒
    assert!(result.matched.len() > 80);    // > 80%匹配率
}
```

---

## 实施计划

### 优先级排序

| Phase | 模块 | 优先级 | 预计工作量 | 预期收益 |
|-------|------|--------|-----------|---------|
| Phase 1 | OCR结构化处理 | P0 | 3天 | 保留完整信息，为后续优化奠基 |
| Phase 2 | 发票解析器重构 | P0 | 5天 | 提取成功率从10%提升到80%+ |
| Phase 3 | 匹配引擎增强 | P1 | 4天 | 匹配准确率从67%提升到90%+ |
| Phase 5 | 测试与验证 | P0 | 2天 | 确保质量和稳定性 |
| Phase 4 | 模板配置系统 | P2 | 3天 | 提升可扩展性，降低维护成本 |

### 实施步骤

**Week 1: Phase 1 + Phase 5.1**
- Day 1-2: 实现OCR结构化输出模型
- Day 3: 实现OCR结果后处理
- Day 4-5: 编写单元测试

**Week 2: Phase 2 + Phase 5.2**
- Day 1-2: 实现发票类型识别和多策略提取器
- Day 3-4: 重构发票解析器
- Day 5: 集成测试和调优

**Week 3: Phase 3 + Phase 5.3**
- Day 1-2: 实现多维度评分机制
- Day 3: 实现智能策略选择和批量优化
- Day 4: 性能测试和调优
- Day 5: 端到端测试验证

**Week 4: Phase 4 + 文档**
- Day 1-3: 实现模板配置系统
- Day 4: 编写用户文档和开发者文档
- Day 5: 代码审查和优化

---

## 风险与缓解措施

### 风险1: OCR识别质量不稳定

**缓解措施**:
- 多次OCR取平均结果
- 使用不同OCR引擎（PaddleOCR v4/v5）交叉验证
- 低置信度区域标记为人工确认

### 风险2: 发票格式变化导致解析失败

**缓解措施**:
- 模板配置系统快速适配新格式
- Fallback策略确保基础功能可用
- 用户反馈机制收集新发票样本

### 风险3: 匹配算法误判

**缓解措施**:
- 多维度评分降低误判率
- 置信度阈值设置保守值
- 提供"人工确认"入口

---

## 成功指标

### 短期目标（1个月）

- ✅ OCR识别成功率 > 90%
- ✅ 发票信息完整度 > 85%
- ✅ 匹配准确率 > 90%
- ✅ 单张发票处理时间 < 5秒

### 长期目标（3个月）

- ✅ 支持发票模板数量 > 10种
- ✅ 自动化匹配覆盖率 > 95%
- ✅ 用户满意度 > 90%

---

## 附录

### A. 参考文档

- [PaddleOCR文档](https://github.com/PaddlePaddle/PaddleOCR)
- [Rust正则表达式库](https://docs.rs/regex/latest/regex/)
- [编辑距离算法](https://en.wikipedia.org/wiki/Levenshtein_distance)

### B. 测试数据集

- 增值税电子发票: 3张
- 滴滴发票/行程单: 4张
- 机票报销凭证: 3张
- 酒店结账单: 1张
- 天府通行程单: 1张
- 其他: 2张

### C. 性能基准

- OCR识别: < 3秒/张
- 发票解析: < 100ms/张
- 匹配100对: < 1秒
- PDF生成: < 5秒
