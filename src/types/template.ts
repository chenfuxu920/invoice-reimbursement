/// 模板来源
export type TemplateSource = 'Builtin' | 'User'

/// 字段提取策略类型
export interface FieldStrategy {
  type: 'regex' | 'section_field'
  pattern: string | null
  section_keyword: string | null
  field_keyword: string | null
  confidence: number
}

/// 字段定义
export interface FieldDefinition {
  name: string
  required: boolean
  strategies: FieldStrategy[]
}

/// 发票模板
export interface InvoiceTemplate {
  template_id: string
  name: string
  enabled: boolean
  priority: number
  keywords: string[]
  category: string | null
  category_keywords: Record<string, string[]> | null
  fields: FieldDefinition[]
}

/// 模板元信息（列表用）
export interface TemplateMeta {
  template_id: string
  name: string
  enabled: boolean
  priority: number
  source: TemplateSource
}

/// 字段类型（标注模式用）
export type FieldType = 'Amount' | 'Date' | 'InvoiceNumber' | 'SellerName' | 'ItemName'

/// 单个字段测试结果
export interface FieldTestResult {
  name: string
  success: boolean
  value: string | null
  error: string | null
}

/// 模板测试结果
export interface TestResult {
  matched: boolean
  matched_keyword: string | null
  fields: FieldTestResult[]
  category: string | null
}

/// 标准字段名列表
export const STANDARD_FIELDS = ['amount', 'seller_name', 'date', 'invoice_number', 'item_name'] as const

/// 字段类型与字段名的映射（标注模式用）
export const FIELD_TYPE_MAP: Record<FieldType, string> = {
  Amount: 'amount',
  Date: 'date',
  InvoiceNumber: 'invoice_number',
  SellerName: 'seller_name',
  ItemName: 'item_name',
}

/// 字段类型中文标签
export const FIELD_TYPE_LABELS: Record<FieldType, string> = {
  Amount: '金额',
  Date: '日期',
  InvoiceNumber: '发票号',
  SellerName: '销售方',
  ItemName: '商品名',
}

/// 发票分类选项
export const CATEGORY_OPTIONS = [
  { value: 'Train', label: '高铁/车船票' },
  { value: 'Flight', label: '飞机票' },
  { value: 'TicketChange', label: '退改签/保险费' },
  { value: 'CityTransport', label: '市内交通' },
  { value: 'Hotel', label: '住宿费' },
  { value: 'Meal', label: '餐饮费' },
  { value: 'Other', label: '其他' },
]
